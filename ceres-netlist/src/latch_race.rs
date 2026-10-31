//! The race of the core's request against the chip's decodings, in
//! picoseconds.
//!
//! It opens the latches of the address pins within a phase
//! (`hdl/rtl/external_bus.vhd`, `hdl/rtl/dmg_cpu_b.vhd`, `hdl/rtl/sm83.vhd`).
//!
//! The core's outputs reach the chip some 14 to 32 ns after the edge that
//! changes them: the request before the address's high byte when a register
//! drives it, the low byte last. The chip's decodings follow: of VRAM's
//! addresses (0.3 ns), and of FE00 to FFFF and the boot ROM's, which gate
//! the request (10 ns). So the request can be high for a few nanoseconds
//! at the start of an access that has none in the end (FE00 to FFFF, the
//! boot ROM), or the decoding see an address outside VRAM at the start of a
//! VRAM access. The latches' enable passes no pulse shorter than its
//! delays: what reaches it opens the latches for a moment, and they keep
//! the address they saw last.
//!
//! The settled phases of `crate::bus` cannot see this; this follows each
//! signal through the phase as a level at its start and the times at which
//! it toggles, with VHDL's delays: a transport delay shifts them, an
//! inertial one also drops the pulses shorter than itself.
//!
//! @fact cartridge-bus-edges

use crate::sm83::{CTL_MREQ_ARRIVES, HighByte, LOW_BYTE_ARRIVES, MREQ_GATE};

/// The decoding of VRAM's addresses (8000 to 9FFF).
const VRAM_DECODE: u32 = 300;
/// The decodings of FE00 to FFFF and of the boot ROM's addresses.
const SLOW_DECODE: u32 = 10_000;
/// TEXO: the request outside VRAM rises after `NON_VRAM_RISE` and falls
/// after `NON_VRAM_FALL`.
const NON_VRAM_RISE: u32 = 2_400;
const NON_VRAM_FALL: u32 = 1_350;
/// The latches' enable.
const LATCH_ENABLE: u32 = 2_700;

/// The most toggles a signal has in a phase.
const MAX_TOGGLES: usize = 8;

/// A signal in a phase: its level as the phase starts, and the times
/// (picoseconds after the edge) at which it toggles, in order.
#[derive(Clone, Copy)]
struct Wave {
    start: bool,
    toggles: [u32; MAX_TOGGLES],
    len: usize,
}

/// No toggle: later than any.
const NEVER: u32 = u32::MAX;

/// How two signals combine.
#[derive(Clone, Copy)]
enum Gate {
    AndNot,
    Or,
}

impl Gate {
    const fn level(self, a: bool, b: bool) -> bool {
        match self {
            Self::AndNot => a & !b,
            Self::Or => a | b,
        }
    }
}

impl Wave {
    /// A signal that is `before` as the phase starts and `after` from `at`.
    const fn step(before: bool, after: bool, at: u32) -> Self {
        let mut wave = Self {
            start: before,
            toggles: [0; MAX_TOGGLES],
            len: 0,
        };
        if before != after {
            wave.toggles[0] = at;
            wave.len = 1;
        }
        wave
    }

    /// The level at `time`.
    fn at(&self, time: u32) -> bool {
        let mut level = self.start;
        for n in 0..self.len {
            if self.toggles[n] <= time {
                level = !level;
            }
        }
        level
    }

    /// The level as the phase ends.
    const fn end(&self) -> bool {
        self.start ^ (self.len % 2 == 1)
    }

    /// A toggle at `time`, after the others.
    const fn push(&mut self, time: u32) {
        self.toggles[self.len] = time;
        self.len += 1;
    }

    /// The signal `delay` later (a transport delay).
    fn delayed(&self, delay: u32) -> Self {
        let mut wave = *self;
        for n in 0..self.len {
            wave.toggles[n] = self.toggles[n] + delay;
        }
        wave
    }

    /// The signal through an inertial delay of `rise` for rises and `fall`
    /// for falls: a change replaces the one still pending, so a level that
    /// lasts less than the delay of the change it schedules is dropped.
    fn inertial(&self, rise: u32, fall: u32) -> Self {
        let mut wave = Self::step(self.start, self.start, 0);
        let mut level = self.start;
        let mut pending = NEVER;
        let mut input = self.start;
        for n in 0..self.len {
            let time = self.toggles[n];
            input = !input;
            // What was due by now has happened; what was not, is dropped.
            if pending <= time {
                wave.push(pending);
                level = !level;
            }
            pending = if input == level {
                NEVER
            } else if input {
                time + rise
            } else {
                time + fall
            };
        }
        if pending != NEVER {
            wave.push(pending);
        }
        wave
    }

    /// Its first toggle after `time`, or `NEVER`.
    fn next_after(&self, time: u32) -> u32 {
        let mut next = NEVER;
        for n in 0..self.len {
            if (self.toggles[n] > time) & (self.toggles[n] < next) {
                next = self.toggles[n];
            }
        }
        next
    }

    /// Two signals through a gate.
    fn gate(&self, other: &Self, gate: Gate) -> Self {
        let mut level = gate.level(self.start, other.start);
        let mut wave = Self::step(level, level, 0);
        // Each toggle of either input, in order (none is at the edge).
        let mut last = 0;
        for _ in 0..self.len + other.len {
            let next = min(self.next_after(last), other.next_after(last));
            if next != NEVER {
                let now = gate.level(self.at(next), other.at(next));
                if now != level {
                    wave.push(next);
                    level = now;
                }
                last = next;
            }
        }
        wave
    }

    /// When it last fell (0 if it did not).
    fn last_fall(&self) -> u32 {
        let mut fall = 0;
        let mut level = self.start;
        for n in 0..self.len {
            level = !level;
            if !level {
                fall = self.toggles[n];
            }
        }
        fall
    }
}

const fn min(a: u32, b: u32) -> u32 {
    if a < b { a } else { b }
}

/// The core's address and request in a phase, and the chip's state that
/// decodes them.
#[derive(Clone, Copy, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the requests' and pins' levels"
)]
pub struct LatchRace {
    /// The address and the decoder's request (`ctl_mreq`) as the phase
    /// before ended, and as this one ends, and what drives the new
    /// address's high byte.
    pub addr_before: u16,
    pub addr: u16,
    pub high_byte: HighByte,
    pub ctl_mreq_before: bool,
    pub ctl_mreq: bool,
    /// The boot ROM is mapped.
    pub boot_mapped: bool,
    /// The test mode T2 alone: the latches are open.
    pub t2_only: bool,
}

/// What an address's high byte decodes to.
// @hw die: SORE TEVY (VRAM), TUNA SYRO (FE00 to FFFF), TULO TERA TUTU (the
// boot ROM's)
#[derive(Clone, Copy)]
struct Decoded {
    vram: bool,
    fexx_ffxx: bool,
    boot: bool,
}

const fn decode(high: u8, boot_mapped: bool) -> Decoded {
    Decoded {
        vram: high >> 5 == 0b100,
        fexx_ffxx: high >> 1 == 0x7F,
        boot: boot_mapped & (high == 0),
    }
}

impl LatchRace {
    /// The address the latches of the address pins took in the phase, if
    /// they opened in it (or were open as it started) and are closed as it
    /// ends: they keep the address as it was when they closed.
    #[must_use]
    pub fn glitch(&self) -> Option<u16> {
        let [high_before, _] = self.addr_before.to_be_bytes();
        let [high_after, _] = self.addr.to_be_bytes();
        let arrives = self.high_byte.arrives();
        let (was, is) = (
            decode(high_before, self.boot_mapped),
            decode(high_after, self.boot_mapped),
        );
        // The chip's decodings of the high byte.
        // @hw die: SORE TEVY; TUNA SYRO, TULO TUTU (after dmg-sim's delays)
        let vram = Wave::step(was.vram, is.vram, arrives).inertial(VRAM_DECODE, VRAM_DECODE);
        let fexx_ffxx =
            Wave::step(was.fexx_ffxx, is.fexx_ffxx, arrives).inertial(SLOW_DECODE, SLOW_DECODE);
        let boot = Wave::step(was.boot, is.boot, arrives).inertial(SLOW_DECODE, SLOW_DECODE);
        // The chip's request: the decoder's, unless the address is FE00 to
        // FFFF or the boot ROM's.
        // @hw die: ZLYZ (oa211)
        let ctl_mreq = Wave::step(self.ctl_mreq_before, self.ctl_mreq, CTL_MREQ_ARRIVES);
        let internal = fexx_ffxx.gate(&boot, Gate::Or);
        let mreq = ctl_mreq.gate(&internal, Gate::AndNot).delayed(MREQ_GATE);
        // The request outside VRAM, then the latches' enable.
        // @hw die: TEXO; the latches' enable (`addr_latch`)
        let non_vram_mreq = mreq
            .gate(&vram, Gate::AndNot)
            .inertial(NON_VRAM_RISE, NON_VRAM_FALL);
        let t2_only = Wave::step(self.t2_only, self.t2_only, 0);
        let enable = non_vram_mreq
            .gate(&t2_only, Gate::Or)
            .inertial(LATCH_ENABLE, LATCH_ENABLE);
        let opened = enable.start | (enable.len > 0);
        if enable.end() | !opened {
            None
        } else {
            // The address as the latches closed.
            let closed = enable.last_fall();
            let high = if closed >= arrives {
                self.addr
            } else {
                self.addr_before
            };
            let low = if closed >= LOW_BYTE_ARRIVES {
                self.addr
            } else {
                self.addr_before
            };
            Some(high & 0xFF00 | low & 0x00FF)
        }
    }
}
