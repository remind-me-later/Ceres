//! The console's commands, with SameBoy's names: a command can be shortened
//! to any prefix at least as long as its shortest form (`c`, `bt`, `x`), and
//! takes modifiers after a slash (`x/32`, `w/rw`, `print/b`).

use {
    super::{
        Breakpoint, Condition, Debugger, Mode, Range, State, Stop, Watchpoint, call_length,
        expr::{Expr, Value},
        info, mapped_bank,
        symbols::{Bank, Symbols},
    },
    ceres_core::{AudioCallback, Gb, MemoryAccess, disassemble},
};

struct Command {
    name: &'static str,
    /// The shortest prefix that names it.
    min: usize,
    args: &'static str,
    help: &'static str,
}

/// The commands, in the order prefixes are tried (as in SameBoy).
const COMMANDS: &[Command] = &[
    Command {
        name: "continue",
        min: 1,
        args: "",
        help: "Runs until the next stop.",
    },
    Command {
        name: "interrupt",
        min: 1,
        args: "",
        help: "Stops the game (Ctrl-C does too).",
    },
    Command {
        name: "reset",
        min: 3,
        args: "",
        help: "Resets the Game Boy.",
    },
    Command {
        name: "next",
        min: 1,
        args: "",
        help: "Runs the next instruction, stepping over calls.",
    },
    Command {
        name: "step",
        min: 1,
        args: "",
        help: "Runs the next instruction, into calls.",
    },
    Command {
        name: "finish",
        min: 1,
        args: "",
        help: "Runs until the current function returns.",
    },
    Command {
        name: "registers",
        min: 1,
        args: "",
        help: "Shows the CPU's registers.",
    },
    Command {
        name: "backtrace",
        min: 2,
        args: "",
        help: "Shows the calls (and interrupts) that led here.",
    },
    Command {
        name: "bt",
        min: 2,
        args: "",
        help: "Same as backtrace.",
    },
    Command {
        name: "print",
        min: 1,
        args: "/[x|d|b|a] <expression>",
        help: "Evaluates an expression (`=` assigns a register or memory). Shows it in hex, \
               (d)ecimal, (b)inary or as an (a)ddress with its label.",
    },
    Command {
        name: "eval",
        min: 2,
        args: "/[x|d|b|a] <expression>",
        help: "Same as print.",
    },
    Command {
        name: "examine",
        min: 2,
        args: "/[count] <expression>",
        help: "Shows memory from an address (16 bytes by default).",
    },
    Command {
        name: "x",
        min: 1,
        args: "/[count] <expression>",
        help: "Same as examine.",
    },
    Command {
        name: "disassemble",
        min: 1,
        args: "/[count] [expression]",
        help: "Disassembles from an address, PC by default (5 instructions by default). \
               A ROM address with a bank reads that bank, mapped or not.",
    },
    Command {
        name: "breakpoint",
        min: 1,
        args: "<expression> [to <end>] [if <condition>]",
        help: "Stops before the instruction at an address, or at any in a range (end \
               included), if the condition holds. On a label, only in the label's bank.",
    },
    Command {
        name: "delete",
        min: 2,
        args: "[id]",
        help: "Deletes a breakpoint, or all of them.",
    },
    Command {
        name: "watch",
        min: 1,
        args: "/[r|w|rw] <expression> [to <end>] [if <condition>]",
        help: "Stops after the CPU reads or writes (by default) an address or a range, if \
               the condition holds; `old` and `new` are the byte's value before and after.",
    },
    Command {
        name: "unwatch",
        min: 3,
        args: "[id]",
        help: "Deletes a watchpoint, or all of them.",
    },
    Command {
        name: "softbreak",
        min: 2,
        args: "on|off",
        help: "Stops at `ld b, b`, the breakpoint test ROMs use.",
    },
    Command {
        name: "list",
        min: 1,
        args: "",
        help: "Lists the breakpoints and watchpoints.",
    },
    Command {
        name: "ticks",
        min: 2,
        args: "[keep]",
        help: "Shows the time since the last `ticks` (`keep`: without starting over).",
    },
    Command {
        name: "cartridge",
        min: 2,
        args: "",
        help: "Shows the banks mapped.",
    },
    Command {
        name: "mbc",
        min: 3,
        args: "",
        help: "Same as cartridge.",
    },
    Command {
        name: "lcd",
        min: 3,
        args: "",
        help: "Shows the display's state: mode, line, dot and registers.",
    },
    Command {
        name: "dma",
        min: 3,
        args: "",
        help: "Shows the OAM DMA and HDMA transfers.",
    },
    Command {
        name: "timer",
        min: 3,
        args: "",
        help: "Shows the timer: DIV's counter, TIMA, TMA, TAC and their countdowns.",
    },
    Command {
        name: "ints",
        min: 4,
        args: "",
        help: "Shows IF, IE, the interrupts pending, IME and HALT.",
    },
    Command {
        name: "save",
        min: 2,
        args: "<file>",
        help: "Saves the machine as a BESS state (SameBoy reads it too).",
    },
    Command {
        name: "load",
        min: 2,
        args: "<file>",
        help: "Loads a BESS state: a starting point, not an exact rewind.",
    },
    Command {
        name: "help",
        min: 1,
        args: "[command]",
        help: "Lists the commands, or explains one.",
    },
];

/// The names of the commands, for completion.
pub(super) fn names() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().map(|command| command.name)
}

fn find(word: &str) -> Option<&'static Command> {
    COMMANDS
        .iter()
        .find(|command| word.len() >= command.min && command.name.starts_with(word))
}

fn help(arg: &str) -> String {
    if arg.is_empty() {
        let intro = "Addresses are hex as in symbol files (BB:AAAA, a bank and an address), \
                     numbers decimal unless $hex or %binary.";
        let lines: Vec<String> = COMMANDS
            .iter()
            .map(|command| format!("  {:<12} {}", command.name, command.help))
            .collect();
        return format!("{intro}\n{}", lines.join("\n"));
    }
    find(arg).map_or_else(
        || format!("no command `{arg}`"),
        |command| format!("{} {}\n{}", command.name, command.args, command.help),
    )
}

impl<A: AudioCallback> Debugger<A> {
    /// Runs a console command and returns what it prints. An empty line
    /// repeats the last command.
    #[inline]
    pub fn execute(&self, line: &str) -> String {
        let mut line = line.trim().to_owned();
        if line.is_empty() {
            line.clone_from(&self.state().last_command);
            if line.is_empty() {
                return String::new();
            }
        } else {
            self.state().last_command.clone_from(&line);
        }

        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((&line, ""));
        let (word, modifiers) = word.split_once('/').unwrap_or((word, ""));
        let arg = rest.trim();
        let Some(command) = find(word) else {
            return format!("no command `{word}` (try `help`)");
        };

        // The machine first, then the debugger, as the emulator thread
        // locks them.
        let mut gb = self.gb();
        let mut state = self.state();
        let output = self.run(command, modifiers, arg, &mut gb, &mut state);
        drop(state);
        drop(gb);
        output
    }

    fn run(
        &self,
        command: &Command,
        modifiers: &str,
        arg: &str,
        gb: &mut Gb<A>,
        state: &mut State,
    ) -> String {
        match command.name {
            "continue" => self.go(state, Mode::Running),
            "interrupt" => {
                if state.mode == Mode::Stopped {
                    "the game is already stopped".to_owned()
                } else {
                    self.interrupt();
                    String::new()
                }
            }
            "reset" => {
                gb.soft_reset();
                state.calls.clear();
                "reset".to_owned()
            }
            "next" => {
                let pc = gb.cpu_pc();
                let mode = call_length(gb.peek_mem(pc)).map_or(Mode::Step, |len| Mode::Next {
                    ret: pc.wrapping_add(len),
                    sp: gb.cpu_sp(),
                });
                self.go(state, mode)
            }
            "step" => self.go(state, Mode::Step),
            "finish" => {
                let sp = gb.cpu_sp();
                self.go(state, Mode::Finish { sp })
            }
            "registers" => info::registers(gb),
            "backtrace" | "bt" => backtrace(gb, state),
            "print" | "eval" => print(gb, state, modifiers, arg),
            "examine" | "x" => examine(gb, state, modifiers, arg),
            "disassemble" => disassemble_command(gb, state, modifiers, arg),
            "breakpoint" => add_breakpoint(gb, state, arg),
            "delete" => delete(state, arg, true, gb),
            "watch" => add_watchpoint(gb, state, modifiers, arg),
            "unwatch" => delete(state, arg, false, gb),
            "softbreak" => match arg {
                "on" => {
                    state.softbreak = true;
                    "stopping at `ld b, b`".to_owned()
                }
                "off" => {
                    state.softbreak = false;
                    "not stopping at `ld b, b`".to_owned()
                }
                _ => "softbreak on or off?".to_owned(),
            },
            "list" => list(state),
            "ticks" => {
                let now = gb.clock_units();
                let units = now - state.ticks_mark;
                if arg != "keep" {
                    state.ticks_mark = now;
                }
                format!("{} dots ({units} half dots)", units / 2)
            }
            "cartridge" | "mbc" => info::cartridge(gb),
            "lcd" => info::lcd(gb),
            "dma" => info::dma(gb),
            "timer" => info::timer(gb),
            "ints" => info::interrupts(gb),
            "save" if !arg.is_empty() => info::save(gb, arg),
            "load" if !arg.is_empty() => match info::load(gb, arg) {
                Ok(loaded) => {
                    state.calls.clear();
                    state.sync_watchpoints(gb);
                    loaded
                }
                Err(e) => e,
            },
            "save" | "load" => format!("{} which file?", command.name),
            _ => help(arg),
        }
    }

    /// Runs the stopped game in `mode`.
    fn go(&self, state: &mut State, mode: Mode) -> String {
        if state.mode == Mode::Stopped {
            self.resume(state, mode);
            String::new()
        } else {
            "the game is running: stop it first (Ctrl-C or `interrupt`)".to_owned()
        }
    }
}

/// The name of `addr` in the bank mapped there, if it's a label.
fn label<A: AudioCallback>(gb: &Gb<A>, symbols: Option<&Symbols>, addr: u16) -> Option<String> {
    let (name, offset) = symbols?.label_for(addr, mapped_bank(gb, addr))?;
    (offset == 0).then(|| name.to_owned())
}

/// `count` instructions from `addr`, in `bank` (the one mapped if none),
/// with labels; `mark` points at one.
fn disassembly<A: AudioCallback>(
    gb: &Gb<A>,
    symbols: Option<&Symbols>,
    bank: Option<Bank>,
    mut addr: u16,
    count: usize,
    mark: Option<u16>,
) -> String {
    let mut lines = Vec::with_capacity(count);
    for _ in 0..count {
        let bank_here = bank.unwrap_or_else(|| mapped_bank(gb, addr));
        let read = |offset: u16| {
            let at = addr.wrapping_add(offset);
            match bank_here {
                Bank::Number(number) if at < 0x8000 && mapped_bank(gb, at) != bank_here => {
                    gb.peek_rom(number, at)
                }
                _ => gb.peek_mem(at),
            }
        };
        let instruction = disassemble(addr, [read(0), read(1), read(2)]);
        let text = instruction.text(|target| label(gb, symbols, target));
        let place = info::describe(symbols, bank_here, addr);
        let arrow = if mark == Some(addr) { "->" } else { "  " };
        lines.push(format!("{arrow}{place:<32} {text}"));
        addr = addr.wrapping_add(u16::from(instruction.len));
    }
    lines.join("\n")
}

/// What the console prints when the game stops: why, where, and the next
/// instructions.
pub(super) fn stop_message<A: AudioCallback>(gb: &Gb<A>, state: &State, stop: Stop) -> String {
    let symbols = state.symbols.as_ref();
    let why = match stop {
        Stop::Breakpoint(id) => format!("Breakpoint {id}"),
        Stop::Watchpoint(id, hit) => {
            let place = info::describe_mapped(gb, symbols, hit.addr);
            if hit.access == MemoryAccess::Write {
                format!(
                    "Watchpoint {id}: ${:02X} written to {place} (was ${:02X})",
                    hit.value, hit.old
                )
            } else {
                format!("Watchpoint {id}: ${:02X} read from {place}", hit.value)
            }
        }
        Stop::Interrupted => "Interrupted".to_owned(),
        Stop::Stepped => String::new(),
        Stop::Softbreak => "ld b, b".to_owned(),
        Stop::BadCondition(id, e) => format!("Stopped: the condition of {id} failed: {e}"),
    };
    let pc = gb.cpu_pc();
    let code = disassembly(gb, symbols, None, pc, 5, Some(pc));
    if why.is_empty() {
        code
    } else {
        format!("{why}\n{code}")
    }
}

fn backtrace<A: AudioCallback>(gb: &Gb<A>, state: &State) -> String {
    let symbols = state.symbols.as_ref();
    let here = format!("  {}", info::describe_mapped(gb, symbols, gb.cpu_pc()));
    let calls = state.calls.iter().rev().map(|frame| {
        let kind = if frame.interrupt {
            "interrupt at"
        } else {
            "called from"
        };
        format!(
            "  {}  {kind} {}",
            info::describe(symbols, frame.target_bank, frame.target),
            info::describe(symbols, frame.site_bank, frame.site),
        )
    });
    core::iter::once(here)
        .chain(calls)
        .collect::<Vec<_>>()
        .join("\n")
}

fn print<A: AudioCallback>(gb: &mut Gb<A>, state: &State, modifiers: &str, arg: &str) -> String {
    let symbols = state.symbols.as_ref();
    let value = match Expr::parse(arg, symbols).and_then(|expr| expr.eval(gb, None)) {
        Ok(value) => value,
        Err(e) => return e,
    };
    let number = value.value;
    match modifiers {
        "d" => number.to_string(),
        "b" => format!("%{number:b}"),
        "a" => {
            let bank = value.bank.unwrap_or_else(|| mapped_bank(gb, number));
            info::describe(symbols, bank, number)
        }
        "" | "x" => value.bank.map_or_else(
            || format!("${number:X} ({number})"),
            |bank| info::describe(symbols, bank, number),
        ),
        other => format!("print: unknown format `{other}` (x, d, b or a)"),
    }
}

/// The count in `x/32`-style modifiers.
fn count(modifiers: &str, default: usize) -> Result<usize, String> {
    if modifiers.is_empty() {
        return Ok(default);
    }
    modifiers
        .strip_prefix('$')
        .map_or_else(|| modifiers.parse(), |hex| usize::from_str_radix(hex, 16))
        .map_err(|_err| format!("bad count `{modifiers}`"))
}

fn examine<A: AudioCallback>(gb: &mut Gb<A>, state: &State, modifiers: &str, arg: &str) -> String {
    let symbols = state.symbols.as_ref();
    let count = match count(modifiers, 16) {
        Ok(count) => count,
        Err(e) => return e,
    };
    let start = match Expr::parse(arg, symbols).and_then(|expr| expr.eval(gb, None)) {
        Ok(value) => value,
        Err(e) => return e,
    };
    let mut lines = Vec::new();
    for row in (0..count).step_by(16) {
        let addr = start
            .value
            .wrapping_add(u16::try_from(row).unwrap_or(u16::MAX));
        let bank = start.bank.unwrap_or_else(|| mapped_bank(gb, addr));
        let mut bytes = Vec::with_capacity(16);
        for column in 0..16.min(count - row) {
            let at = Value {
                bank: start.bank,
                value: addr.wrapping_add(u16::try_from(column).unwrap_or_default()),
            };
            match Expr::Byte(Box::new(Expr::Address(at))).eval(gb, None) {
                Ok(byte) => bytes.push(format!("{:02X}", byte.value)),
                Err(e) => return e,
            }
        }
        lines.push(format!("{bank}:{addr:04X}: {}", bytes.join(" ")));
    }
    lines.join("\n")
}

fn disassemble_command<A: AudioCallback>(
    gb: &mut Gb<A>,
    state: &State,
    modifiers: &str,
    arg: &str,
) -> String {
    let symbols = state.symbols.as_ref();
    let count = match count(modifiers, 5) {
        Ok(count) => count,
        Err(e) => return e,
    };
    let start = if arg.is_empty() {
        Value {
            bank: None,
            value: gb.cpu_pc(),
        }
    } else {
        match Expr::parse(arg, symbols).and_then(|expr| expr.eval(gb, None)) {
            Ok(value) => value,
            Err(e) => return e,
        }
    };
    disassembly(
        gb,
        symbols,
        start.bank,
        start.value,
        count,
        Some(gb.cpu_pc()),
    )
}

/// `<expression> [to <end>] [if <condition>]`.
fn parse_range<A: AudioCallback>(
    gb: &mut Gb<A>,
    symbols: Option<&Symbols>,
    arg: &str,
) -> Result<(Range, Option<Condition>), String> {
    let (place, condition) = match arg.split_once(" if ") {
        Some((place, condition)) => (place, Some(condition.trim())),
        None => (arg, None),
    };
    let (start, end) = match place.split_once(" to ") {
        Some((start, end)) => (start, Some(end)),
        None => (place, None),
    };
    if start.trim().is_empty() {
        return Err("where?".to_owned());
    }
    let start = Expr::parse(start, symbols)?.eval(gb, None)?;
    let end = match end {
        Some(end) => Expr::parse(end, symbols)?.eval(gb, None)?.value,
        None => start.value,
    };
    if end < start.value {
        return Err("the range ends before it starts".to_owned());
    }
    let condition = condition
        .map(|text| {
            Expr::parse(text, symbols).map(|expr| Condition {
                text: text.to_owned(),
                expr,
            })
        })
        .transpose()?;
    Ok((
        Range {
            bank: start.bank,
            start: start.value,
            end,
        },
        condition,
    ))
}

fn add_breakpoint<A: AudioCallback>(gb: &mut Gb<A>, state: &mut State, arg: &str) -> String {
    let (range, condition) = match parse_range(gb, state.symbols.as_ref(), arg) {
        Ok(parsed) => parsed,
        Err(e) => return format!("breakpoint: {e}"),
    };
    let id = state.next_id();
    let breakpoint = Breakpoint {
        id,
        range,
        condition,
    };
    let text = format!(
        "Breakpoint {}",
        describe_point(state, id, &breakpoint.range, breakpoint.condition.as_ref())
    );
    state.breakpoints.push(breakpoint);
    state.rebuild_breakpoint_bits();
    text
}

fn add_watchpoint<A: AudioCallback>(
    gb: &mut Gb<A>,
    state: &mut State,
    modifiers: &str,
    arg: &str,
) -> String {
    let (read, write) = match modifiers {
        "" | "w" => (false, true),
        "r" => (true, false),
        "rw" | "wr" => (true, true),
        other => return format!("watch: unknown access `{other}` (r, w or rw)"),
    };
    let (range, condition) = match parse_range(gb, state.symbols.as_ref(), arg) {
        Ok(parsed) => parsed,
        Err(e) => return format!("watch: {e}"),
    };
    let id = state.next_id();
    let watchpoint = Watchpoint {
        id,
        range,
        read,
        write,
        condition,
    };
    let text = format!(
        "Watchpoint {} ({})",
        describe_point(state, id, &watchpoint.range, watchpoint.condition.as_ref()),
        access(&watchpoint)
    );
    state.watchpoints.push(watchpoint);
    state.sync_watchpoints(gb);
    text
}

const fn access(watchpoint: &Watchpoint) -> &'static str {
    match (watchpoint.read, watchpoint.write) {
        (true, true) => "reads and writes",
        (true, false) => "reads",
        _ => "writes",
    }
}

/// `1: 39:4579 SplashScreen [to ...] [if ...]`.
fn describe_point(state: &State, id: u32, range: &Range, condition: Option<&Condition>) -> String {
    let symbols = state.symbols.as_ref();
    let place = |addr: u16| {
        range.bank.map_or_else(
            || format!("{addr:04X} (any bank)"),
            |bank| info::describe(symbols, bank, addr),
        )
    };
    let mut text = format!("{id}: {}", place(range.start));
    if range.end != range.start {
        text = format!("{text} to {}", place(range.end));
    }
    if let Some(condition) = condition {
        text = format!("{text} if {}", condition.text);
    }
    text
}

fn delete<A: AudioCallback>(
    state: &mut State,
    arg: &str,
    breakpoints: bool,
    gb: &mut Gb<A>,
) -> String {
    let kind = if breakpoints {
        "breakpoint"
    } else {
        "watchpoint"
    };
    let result = if arg.is_empty() {
        let count = if breakpoints {
            std::mem::take(&mut state.breakpoints).len()
        } else {
            std::mem::take(&mut state.watchpoints).len()
        };
        format!("deleted {count} {kind}s")
    } else {
        let Ok(id) = arg.parse::<u32>() else {
            return format!("which {kind}? (a number)");
        };
        let before = state.breakpoints.len() + state.watchpoints.len();
        if breakpoints {
            state.breakpoints.retain(|point| point.id != id);
        } else {
            state.watchpoints.retain(|point| point.id != id);
        }
        if state.breakpoints.len() + state.watchpoints.len() == before {
            return format!("no {kind} {id}");
        }
        format!("deleted {kind} {id}")
    };
    state.rebuild_breakpoint_bits();
    state.sync_watchpoints(gb);
    result
}

fn list(state: &State) -> String {
    if state.breakpoints.is_empty() && state.watchpoints.is_empty() {
        return "no breakpoints or watchpoints".to_owned();
    }
    let breakpoints = state.breakpoints.iter().map(|breakpoint| {
        let point = describe_point(
            state,
            breakpoint.id,
            &breakpoint.range,
            breakpoint.condition.as_ref(),
        );
        format!("Breakpoint {point}")
    });
    let watchpoints = state.watchpoints.iter().map(|watchpoint| {
        let point = describe_point(
            state,
            watchpoint.id,
            &watchpoint.range,
            watchpoint.condition.as_ref(),
        );
        format!("Watchpoint {point} ({})", access(watchpoint))
    });
    breakpoints
        .chain(watchpoints)
        .collect::<Vec<_>>()
        .join("\n")
}
