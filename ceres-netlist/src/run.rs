//! A ROM on `NetChip` from the power-on, as the model's testbench runs it.
//!
//! The board's pins (`ChipInputs::board`), its memories, and the test's
//! end as the testbench sees it (`hdl/board/gameboy_tb.vhd`). Nothing comes
//! from a trace.
//!
//! This is the transcriptions' harness, not a part of them: Aeneas does not
//! take it.

use crate::{
    Phase,
    bus::NetBoard,
    chip::{ChipInputs, ChipMemories, NetChip},
    render::vram_power_on,
};

/// A half period of the DMG's crystal, in picoseconds.
///
/// It is the time between two edges. The Game Boy's is 119.2 ns (4 MiHz);
/// the model's and dmg-sim's is rounded to 119 ns, and so is this one, to
/// keep their time.
pub const HALF_PERIOD_PS: u64 = 119_000;

/// How a test ROM reports its result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Test {
    /// Mooneye's: LD B,B (opcode 40), with the Fibonacci numbers 3, 5, 8,
    /// 13, 21 and 34 in B, C, D, E, H and L if it passed.
    Mooneye,
    /// gbmicrotest's: 01 (passed) or FF (failed) in FF82, with the result
    /// in FF80 and the expected value in FF81.
    Gbmicrotest,
    /// Blargg's: "Passed" or "Failed" at the end of what it sends on the
    /// serial port.
    Blargg,
}

/// A memory access of the CPU, as the model's testbench traces it
/// (`BUS_TRACE`): seen in phase H, the M-cycle's address and the byte on
/// the core's data bus, read or written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CpuAccess {
    /// The M-cycle, counted from the CPU's first access.
    pub mcycle: u64,
    pub write: bool,
    pub addr: u16,
    pub value: u8,
}

/// How a run ended.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Pass(String),
    Fail(String),
    /// The time ran out first.
    Timeout,
}

/// A ROM running on the chip.
pub struct Run {
    pub chip: NetChip,
    pub memories: ChipMemories,
    /// The crystal's edges since the power-on.
    pub edge: u64,
    /// What the chip sent on the serial port, the byte coming in, its
    /// bits so far, and SCK at the last edge.
    pub serial: Vec<u8>,
    pub byte_in: u8,
    pub bits_in: u8,
    pub sck_before: bool,
    /// The CPU has read the entry point (0100): the boot ROM is done.
    pub booted: bool,
    /// The edge at which a result showed, which counts once it holds for
    /// 2 µs (the testbench's wait).
    pub shown_at: Option<u64>,
    /// The M-cycles since the CPU's first access (counted at each A, as
    /// the phases run on through HALT), once it came.
    pub mcycle: Option<u64>,
}

/// The edges in `ps` picoseconds.
const fn edges_in(ps: u64) -> u64 {
    ps.div_ceil(HALF_PERIOD_PS)
}

impl Run {
    /// The chip at power-on with the cartridge's `rom` and the boot ROM
    /// `boot_rom`, VRAM with the board's arbitrary bytes.
    #[must_use]
    pub fn new(rom: Vec<u8>, boot_rom: Vec<u8>) -> Self {
        Self::with_vram(rom, boot_rom, vram_power_on())
    }

    /// The chip at power-on, as `new`, with VRAM holding `vram` (the
    /// model's board can zero it: `VRAM_ZERO`).
    #[must_use]
    pub fn with_vram(rom: Vec<u8>, boot_rom: Vec<u8>, vram: Box<[u8; 0x2000]>) -> Self {
        let memories = ChipMemories {
            vram,
            board: NetBoard::power_on(rom),
            boot_rom,
        };
        let chip = NetChip::power_on(ChipInputs::board(0), &memories.vram);
        Self {
            chip,
            memories,
            edge: 0,
            serial: Vec::new(),
            byte_in: 0,
            bits_in: 0,
            sck_before: true,
            booted: false,
            shown_at: None,
            mcycle: None,
        }
    }

    /// The crystal's next edge, and the phase it starts; the CPU's access,
    /// if the phase is an H that has one.
    pub fn step(&mut self) -> Option<CpuAccess> {
        self.edge += 1;
        let pins = ChipInputs::board(self.edge);
        let events = self.chip.edge(pins, &mut self.memories);
        let phase = self.chip.clocks.phase();
        if events.core.rd & (events.core.addr == 0x0100) {
            self.booted = true;
        }
        // The other end of the link: SCK is the chip's while its internal
        // clock drives it, else the pull-up's; a byte comes in with the
        // most significant bit first, on SCK's rises.
        let sck = if self.chip.bus.internal_clock {
            events.bus.sck
        } else {
            pins.sck
        };
        if sck & !self.sck_before {
            self.byte_in = self.byte_in << 1 | u8::from(events.bus.sout);
            self.bits_in += 1;
            if self.bits_in == 8 {
                self.serial.push(self.byte_in);
                self.bits_in = 0;
            }
        }
        self.sck_before = sck;

        // The CPU's accesses, as the testbench's BUS_TRACE sees them.
        if matches!(phase, Phase::A) {
            self.mcycle = self.mcycle.map(|m| m + 1);
        }
        let access = events.clocks.data_phase & (events.core.rd | events.core.wr);
        (matches!(phase, Phase::H) & access).then(|| {
            let mcycle = *self.mcycle.get_or_insert(0);
            CpuAccess {
                mcycle,
                write: events.core.wr,
                addr: events.core.addr,
                value: events.core.db,
            }
        })
    }

    /// The result as `test` reads it, if it shows.
    fn result(&mut self, test: Test) -> Option<Outcome> {
        let hold = edges_in(2_000_000);
        match test {
            Test::Mooneye => {
                let seq = &self.chip.core.seq;
                if self.shown_at.is_none() & self.booted & (seq.opcode == 0x40) {
                    self.shown_at = Some(self.edge);
                }
                let at = self.shown_at?;
                (self.edge >= at + hold).then(|| {
                    let r = &self.chip.core.regs;
                    let regs = [r.b.q, r.c.q, r.d.q, r.e.q, r.h.q, r.l.q];
                    if regs == [3, 5, 8, 13, 21, 34] {
                        Outcome::Pass(String::new())
                    } else {
                        Outcome::Fail(format!(
                            "B={:02X} C={:02X} D={:02X} E={:02X} H={:02X} L={:02X}",
                            regs[0], regs[1], regs[2], regs[3], regs[4], regs[5]
                        ))
                    }
                })
            }
            Test::Gbmicrotest => {
                // FF80 to FF82: HRAM's first bytes.
                let ram = &self.chip.bus.high_ram;
                let done = matches!(ram[2], 0x01 | 0xFF);
                match (done, self.shown_at) {
                    (false, _) => {
                        self.shown_at = None;
                        None
                    }
                    (true, None) => {
                        self.shown_at = Some(self.edge);
                        None
                    }
                    (true, Some(at)) => (self.edge >= at + hold).then(|| {
                        let detail = format!("FF80={:02X} FF81={:02X}", ram[0], ram[1]);
                        if ram[2] == 0x01 {
                            Outcome::Pass(detail)
                        } else {
                            Outcome::Fail(detail)
                        }
                    }),
                }
            }
            Test::Blargg => {
                let sent = String::from_utf8_lossy(&self.serial).into_owned();
                if sent.ends_with("Passed") {
                    Some(Outcome::Pass(format!("serial: {sent}")))
                } else if sent.ends_with("Failed") {
                    Some(Outcome::Fail(format!("serial: {sent}")))
                } else {
                    None
                }
            }
        }
    }

    /// Runs until `test`'s result shows, for at most `secs` seconds after
    /// the reset ends (`cpu_sys_reset` falls, after the power-on wait), as
    /// the testbench's time limit.
    pub fn until(&mut self, test: Test, secs: f64) -> Outcome {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a time limit in picoseconds"
        )]
        let run_for = edges_in((secs * 1e12) as u64);
        // A reset that never ends still stops: a second at most.
        let mut limit = edges_in(1_000_000_000_000) + run_for;
        let mut in_reset = self.chip.clocks.cpu_sys_reset;
        while self.edge < limit {
            self.step();
            if in_reset & !self.chip.clocks.cpu_sys_reset {
                limit = self.edge + run_for;
            }
            in_reset = self.chip.clocks.cpu_sys_reset;
            if let Some(outcome) = self.result(test) {
                return outcome;
            }
        }
        Outcome::Timeout
    }
}
