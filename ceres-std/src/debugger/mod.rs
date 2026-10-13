//! A debugger for the running game, in the terminal, after SameBoy's.
//!
//! Break into it with Ctrl-C, set breakpoints and watchpoints, step, and
//! look at the machine. Addresses are written as symbol files write them,
//! `BB:AAAA` (bank and address, in hex).
//!
//! The emulator thread runs frames through `Debugger::run_frame`, which
//! checks every instruction for a reason to stop; the console thread
//! (`console`) reads commands and runs them with `Debugger::execute`.

mod commands;
mod console;
pub mod expr;
mod info;
pub mod symbols;

use {
    ceres_core::{AudioCallback, Gb, MemoryAccess, WatchHit},
    core::sync::atomic::{AtomicBool, Ordering},
    expr::{Expr, Watched},
    std::{
        path::Path,
        sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
        time::Duration,
    },
    symbols::{Bank, Symbols},
};

pub use console::spawn;

/// The bank mapped where `addr` is, as symbol files number them.
#[must_use]
#[inline]
pub fn mapped_bank<A: AudioCallback>(gb: &Gb<A>, addr: u16) -> Bank {
    if gb.boot_rom_at(addr) {
        return Bank::Boot;
    }
    Bank::Number(match addr {
        0x4000..=0x7FFF => gb.rom_banks().1,
        0x8000..=0x9FFF => u32::from(gb.vram_bank()),
        0xA000..=0xBFFF => gb.ram_bank(),
        0xD000..=0xDFFF => u32::from(gb.wram_bank()),
        _ => 0,
    })
}

/// Where stop messages go.
type Printer = Box<dyn FnMut(String) + Send>;

/// The debugger, shared by the console and the emulator thread.
pub struct Debugger<A: AudioCallback> {
    gb: Arc<Mutex<Gb<A>>>,
    state: Mutex<State>,
    /// Wakes the emulator thread when the game resumes.
    resume: Condvar,
    /// Stop at the next instruction (Ctrl-C).
    interrupt: AtomicBool,
    /// Checking every instruction: off once the console is closed.
    active: AtomicBool,
    printer: Mutex<Option<Printer>>,
}

/// What the game is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Running,
    Stopped,
    /// Stop after one instruction (a halted CPU: once it wakes up).
    Step,
    /// Stop back after a call: at `ret` with SP back at `sp`.
    Next {
        ret: u16,
        sp: u16,
    },
    /// Stop once a return takes SP above `sp`.
    Finish {
        sp: u16,
    },
}

/// Why the game stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Stop {
    Breakpoint(u32),
    Watchpoint(u32, WatchHit),
    Interrupted,
    Stepped,
    /// The `ld b, b` breakpoint test ROMs use.
    Softbreak,
    /// A breakpoint or watchpoint condition that can't be evaluated.
    BadCondition(u32, String),
}

/// Addresses `start..=end`, in `bank` (any if none).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Range {
    bank: Option<Bank>,
    start: u16,
    end: u16,
}

impl Range {
    fn contains<A: AudioCallback>(&self, gb: &Gb<A>, addr: u16) -> bool {
        (self.start..=self.end).contains(&addr)
            && self
                .bank
                .is_none_or(|bank| Symbols::is_mapped(addr, bank, mapped_bank(gb, addr)))
    }
}

#[derive(Clone, Debug)]
struct Condition {
    text: String,
    expr: Expr,
}

#[derive(Clone, Debug)]
struct Breakpoint {
    id: u32,
    range: Range,
    condition: Option<Condition>,
}

#[derive(Clone, Debug)]
struct Watchpoint {
    id: u32,
    range: Range,
    read: bool,
    write: bool,
    condition: Option<Condition>,
}

/// A call the CPU made (or an interrupt it took), for `backtrace`.
#[derive(Clone, Copy, Debug)]
struct Frame {
    /// Where the call is.
    site: u16,
    site_bank: Bank,
    /// Where it went.
    target: u16,
    target_bank: Bank,
    /// SP once the return address is pushed.
    sp: u16,
    interrupt: bool,
}

/// The instruction about to run, to see what it did after.
#[derive(Clone, Copy, Debug, Default)]
struct Before {
    pc: u16,
    sp: u16,
    opcode: u8,
}

impl Before {
    fn of<A: AudioCallback>(gb: &Gb<A>) -> Self {
        Self {
            pc: gb.cpu_pc(),
            sp: gb.cpu_sp(),
            opcode: gb.peek_mem(gb.cpu_pc()),
        }
    }
}

/// How long a call is, if `opcode` is one (CALL, conditional or not, or
/// RST).
const fn call_length(opcode: u8) -> Option<u16> {
    match opcode {
        0xCD | 0xC4 | 0xCC | 0xD4 | 0xDC => Some(3),
        _ if opcode & 0xC7 == 0xC7 => Some(1),
        _ => None,
    }
}

/// RET, conditional or not, and RETI.
const fn is_return(opcode: u8) -> bool {
    matches!(opcode, 0xC9 | 0xD9 | 0xC0 | 0xC8 | 0xD0 | 0xD8)
}

const INTERRUPT_VECTORS: [u16; 5] = [0x40, 0x48, 0x50, 0x58, 0x60];

struct State {
    mode: Mode,
    breakpoints: Vec<Breakpoint>,
    watchpoints: Vec<Watchpoint>,
    next_id: u32,
    /// A bit per address with a breakpoint, to skip the list quickly.
    breakpoint_bits: Vec<u64>,
    softbreak: bool,
    calls: Vec<Frame>,
    before: Before,
    symbols: Option<Symbols>,
    /// `Gb::clock_units` at the last `ticks`.
    ticks_mark: u64,
    stop: Option<Stop>,
    /// The last command, which an empty line repeats.
    last_command: String,
}

impl State {
    fn new() -> Self {
        Self {
            mode: Mode::Running,
            breakpoints: Vec::new(),
            watchpoints: Vec::new(),
            next_id: 1,
            breakpoint_bits: vec![0; 0x1_0000 / 64],
            softbreak: false,
            calls: Vec::new(),
            before: Before::default(),
            symbols: None,
            ticks_mark: 0,
            stop: None,
            last_command: String::new(),
        }
    }

    const fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn rebuild_breakpoint_bits(&mut self) {
        self.breakpoint_bits.fill(0);
        for breakpoint in &self.breakpoints {
            for addr in breakpoint.range.start..=breakpoint.range.end {
                self.breakpoint_bits[usize::from(addr) / 64] |= 1 << (addr % 64);
            }
        }
    }

    /// Makes the emulator watch what the watchpoints watch.
    fn sync_watchpoints<A: AudioCallback>(&self, gb: &mut Gb<A>) {
        gb.clear_watchpoints();
        for watchpoint in &self.watchpoints {
            for addr in watchpoint.range.start..=watchpoint.range.end {
                if watchpoint.read {
                    gb.set_watchpoint(addr, MemoryAccess::Read, true);
                }
                if watchpoint.write {
                    gb.set_watchpoint(addr, MemoryAccess::Write, true);
                }
            }
        }
    }

    /// After every instruction: follows calls and returns, and says whether
    /// to stop.
    fn after_step<A: AudioCallback>(&mut self, gb: &mut Gb<A>, interrupt: &AtomicBool) -> bool {
        let before = self.before;
        let pc = gb.cpu_pc();
        let sp = gb.cpu_sp();
        self.track_calls(gb, before, pc, sp);
        self.before = Before::of(gb);

        // Taken every time, so an old one doesn't stop a later run.
        let watch_hit = gb.take_watch_hit();
        let softbreak = gb.take_ld_b_b_breakpoint() && self.softbreak;
        let halted = gb.cpu_is_halted();

        self.stop = if interrupt.swap(false, Ordering::Relaxed) {
            Some(Stop::Interrupted)
        } else if let Some(stop) = watch_hit.and_then(|hit| self.watch_stop(gb, hit)) {
            Some(stop)
        } else if let Some(stop) = self.breakpoint_stop(gb, pc, halted) {
            Some(stop)
        } else if softbreak {
            Some(Stop::Softbreak)
        } else {
            let done = match self.mode {
                Mode::Running | Mode::Stopped => false,
                Mode::Step => !halted,
                Mode::Next { ret, sp: frame_sp } => pc == ret && sp == frame_sp,
                Mode::Finish { sp: frame_sp } => is_return(before.opcode) && sp > frame_sp,
            };
            done.then_some(Stop::Stepped)
        };
        self.stop.is_some()
    }

    fn track_calls<A: AudioCallback>(&mut self, gb: &Gb<A>, before: Before, pc: u16, sp: u16) {
        if sp == before.sp.wrapping_sub(2) {
            // Where it will return to.
            let ret = u16::from_le_bytes([gb.peek_mem(sp), gb.peek_mem(sp.wrapping_add(1))]);
            let call =
                call_length(before.opcode).is_some_and(|len| ret == before.pc.wrapping_add(len));
            let interrupt = ret == before.pc && INTERRUPT_VECTORS.contains(&pc);
            if call || interrupt {
                self.calls.push(Frame {
                    site: before.pc,
                    site_bank: mapped_bank(gb, before.pc),
                    target: pc,
                    target_bank: mapped_bank(gb, pc),
                    sp,
                    interrupt,
                });
            }
        }
        // Calls whose return address is above the stack have returned (or
        // the stack was moved past them).
        while self.calls.last().is_some_and(|frame| frame.sp < sp) {
            self.calls.pop();
        }
    }

    fn watch_stop<A: AudioCallback>(&self, gb: &mut Gb<A>, hit: WatchHit) -> Option<Stop> {
        let write = hit.access == MemoryAccess::Write;
        let watched = Watched {
            old: hit.old,
            new: hit.value,
        };
        for watchpoint in &self.watchpoints {
            let wanted = if write {
                watchpoint.write
            } else {
                watchpoint.read
            };
            if !wanted || !watchpoint.range.contains(gb, hit.addr) {
                continue;
            }
            let Some(condition) = &watchpoint.condition else {
                return Some(Stop::Watchpoint(watchpoint.id, hit));
            };
            match condition.expr.eval(gb, Some(watched)) {
                Ok(value) if value.value != 0 => return Some(Stop::Watchpoint(watchpoint.id, hit)),
                Ok(_) => {}
                Err(e) => return Some(Stop::BadCondition(watchpoint.id, e)),
            }
        }
        None
    }

    fn breakpoint_stop<A: AudioCallback>(
        &self,
        gb: &mut Gb<A>,
        pc: u16,
        halted: bool,
    ) -> Option<Stop> {
        // A halted CPU sits after HALT for many steps: a breakpoint there
        // stops once, when it wakes up.
        if halted || self.breakpoint_bits[usize::from(pc) / 64] & (1 << (pc % 64)) == 0 {
            return None;
        }
        for breakpoint in &self.breakpoints {
            if !breakpoint.range.contains(gb, pc) {
                continue;
            }
            let Some(condition) = &breakpoint.condition else {
                return Some(Stop::Breakpoint(breakpoint.id));
            };
            match condition.expr.eval(gb, None) {
                Ok(value) if value.value != 0 => return Some(Stop::Breakpoint(breakpoint.id)),
                Ok(_) => {}
                Err(e) => return Some(Stop::BadCondition(breakpoint.id, e)),
            }
        }
        None
    }
}

impl<A: AudioCallback> Debugger<A> {
    /// A debugger for `gb`, which is running.
    #[must_use]
    #[inline]
    pub fn new(gb: Arc<Mutex<Gb<A>>>) -> Arc<Self> {
        Arc::new(Self {
            gb,
            state: Mutex::new(State::new()),
            resume: Condvar::new(),
            interrupt: AtomicBool::new(false),
            active: AtomicBool::new(true),
            printer: Mutex::new(None),
        })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn gb(&self) -> MutexGuard<'_, Gb<A>> {
        self.gb.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Where stop messages go (the console's prompt; tests collect them).
    #[inline]
    pub fn set_printer<F: FnMut(String) + Send + 'static>(&self, printer: F) {
        *self.printer.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(printer));
    }

    fn print(&self, message: String) {
        if let Some(printer) = self
            .printer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            printer(message);
        }
    }

    /// Reads the symbol file next to the ROM at `rom_path` (`game.sym` for
    /// `game.gb`), if there is one; says how many symbols it has.
    #[inline]
    pub fn load_symbols(&self, rom_path: Option<&Path>) -> Option<usize> {
        let symbols = rom_path.and_then(Symbols::for_rom);
        let count = symbols.as_ref().map(Symbols::len);
        let mut state = self.state();
        state.symbols = symbols;
        state.calls.clear();
        count
    }

    /// Whether the game is stopped in the debugger.
    #[must_use]
    #[inline]
    pub fn is_stopped(&self) -> bool {
        self.state().mode == Mode::Stopped
    }

    /// Stops the running game at its next instruction.
    #[inline]
    pub fn interrupt(&self) {
        self.interrupt.store(true, Ordering::Relaxed);
    }

    /// Waits while the game is stopped, or until `exiting`. Returns whether
    /// it waited.
    #[inline]
    pub fn wait_while_stopped(&self, exiting: &AtomicBool) -> bool {
        let mut state = self.state();
        let mut waited = false;
        while state.mode == Mode::Stopped && !exiting.load(Ordering::Relaxed) {
            waited = true;
            state = self
                .resume
                .wait_timeout(state, Duration::from_millis(100))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        drop(state);
        waited
    }

    /// Runs `gb` (locked by the caller) for the rest of the frame, or until
    /// a reason to stop, which it prints. Returns whether the frame ended.
    #[inline]
    pub fn run_frame(&self, gb: &mut Gb<A>) -> bool {
        if !self.active.load(Ordering::Relaxed) {
            gb.run_frame();
            return true;
        }
        let mut state = self.state();
        if state.mode == Mode::Stopped {
            return false;
        }
        state.before = Before::of(gb);
        let ended = gb.run_frame_until(|gb| state.after_step(gb, &self.interrupt));
        if let Some(stop) = state.stop.take() {
            state.mode = Mode::Stopped;
            let message = commands::stop_message(gb, &state, stop);
            drop(state);
            self.print(message);
        }
        ended
    }

    /// Lets the game run on: from `mode`, with nothing to check if `detach`.
    fn resume(&self, state: &mut State, mode: Mode) {
        state.mode = mode;
        self.resume.notify_all();
    }

    /// The console is gone: the game runs on with nothing to stop it.
    fn detach(&self) {
        let mut gb = self.gb();
        let mut state = self.state();
        state.breakpoints.clear();
        state.watchpoints.clear();
        state.rebuild_breakpoint_bits();
        state.sync_watchpoints(&mut gb);
        state.softbreak = false;
        drop(gb);
        self.active.store(false, Ordering::Relaxed);
        self.resume(&mut state, Mode::Running);
        drop(state);
    }
}
