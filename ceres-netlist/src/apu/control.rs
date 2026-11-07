//! The APU's control (`hdl/rtl/apu_control.vhd`): NR50, NR51 and NR52's
//! power and test bits; the dividers of the chip's clock that give the
//! channels theirs; the frame sequencer's clocks of 512, 256 and 128 Hz.
//!
//! @hw die: dmg-schematics, category apu-control (sheet "APU control")

use super::{ApuInputs, Ff, fell, rose};

/// The control's flip-flops and latches.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct NetApuControl {
    /// NR52 bit 7: the APU's power, taken as the CPU's write ends. HADA
    pub power: Ff<bool>,
    /// NR52 bit 4 (with T1 alone), a latch; bit 5, and its copy at 2 MiHz.
    /// FERO; BOWY, BAZA
    pub test: bool,
    pub test_bit5: Ff<bool>,
    pub baza: Ff<bool>,
    /// NR50, NR51: latches, open while the CPU writes them.
    pub nr50: u8,
    pub nr51: u8,
    /// The dividers of the chip's clock. AJER ATEP CERY ATYK; CALO CEMO
    /// AVOK; JESO
    pub ajer: Ff<bool>,
    pub calo: Ff<bool>,
    pub atep: Ff<bool>,
    pub cemo: Ff<bool>,
    pub cery: Ff<bool>,
    pub atyk: Ff<bool>,
    pub avok: Ff<bool>,
    pub jeso: Ff<bool>,
    /// The frame sequencer: the divider's bit 10, and two halvings. BARA
    /// CARU BYLU
    pub bara: Ff<bool>,
    pub caru: Ff<bool>,
    pub bylu: Ff<bool>,
    pub wires: ControlWires,
}

/// The control's concurrent statements, each a delta behind its inputs;
/// its ports that copy a signal are wires too.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[expect(clippy::struct_excessive_bools, reason = "the wires' levels")]
pub struct ControlWires {
    pub write_nr50: bool,
    pub write_nr51: bool,
    pub write_nr52: bool,
    /// The APU's reset: no power, or reset2; and the port.
    pub apu_reset_i: bool,
    pub apu_reset: bool,
    pub test: bool,
    /// The channels' clocks.
    pub ajer: bool,
    pub ch1_1mhz: bool,
    pub ch2_1mhz: bool,
    pub ch3_2mhz: bool,
    pub ch4_1mhz_i: bool,
    pub ch4_1mhz: bool,
    pub khz512: bool,
    /// The frame sequencer's clocks (in test mode, all 512 KiHz).
    pub hz512: bool,
    pub hz256: bool,
    pub hz128_i: bool,
    pub hz128: bool,
    pub hz128_ch1: bool,
    /// Not `data_phase`: rises at the start of phase A.
    pub apu_phi: bool,
}

impl NetApuControl {
    /// A flip-flop's change is pending.
    #[must_use]
    pub const fn any_due(&self) -> bool {
        self.power.due
            | self.test_bit5.due
            | self.baza.due
            | self.ajer.due
            | self.calo.due
            | self.atep.due
            | self.cemo.due
            | self.cery.due
            | self.atyk.due
            | self.avok.due
            | self.jeso.due
            | self.bara.due
            | self.caru.due
            | self.bylu.due
    }

    /// The next nanosecond.
    pub const fn advance(&mut self) {
        self.power.advance();
        self.test_bit5.advance();
        self.baza.advance();
        self.ajer.advance();
        self.calo.advance();
        self.atep.advance();
        self.cemo.advance();
        self.cery.advance();
        self.atyk.advance();
        self.avok.advance();
        self.jeso.advance();
        self.bara.advance();
        self.caru.advance();
        self.bylu.advance();
    }

    /// What a read of NR50, NR51 or NR52 drives on the data bus, with the
    /// channels' `active` ports (bit `n - 1` for channel `n`); the bits
    /// nothing drives stay precharged, 1.
    // @hw die: JURE HOPE, DOLE COTO DURU EFUS FEWA FATE KAMU KOGE; ADAK, GUMU
    #[must_use]
    pub const fn read(&self, i: &ApuInputs, active: u8) -> Option<u8> {
        if !i.cpu_rd {
            None
        } else if i.addr == 0xFF24 {
            Some(self.nr50)
        } else if i.addr == 0xFF25 {
            Some(self.nr51)
        } else if i.addr == 0xFF26 {
            Some((self.power.q as u8) << 7 | 0x70 | active)
        } else {
            None
        }
    }

    /// The wires, from the signals as they are.
    // @hw die: BOSU BOWE, BUPO BONO, ETUC HAWU BOPY; APUV BATA DYFA AZEG BUFO
    // CYBO ARYF BAVU AMUK; BEZE BULE GALE CELY (mux)
    #[must_use]
    pub const fn next_wires(&self, i: &ApuInputs) -> ControlWires {
        let w = &self.wires;
        let test = self.test;
        ControlWires {
            write_nr50: i.cpu_wr & (i.addr == 0xFF24),
            write_nr51: i.cpu_wr & (i.addr == 0xFF25),
            write_nr52: i.cpu_wr & (i.addr == 0xFF26),
            apu_reset_i: !self.power.q | i.reset2,
            apu_reset: w.apu_reset_i,
            test,
            ajer: self.ajer.q,
            ch1_1mhz: self.calo.q,
            ch2_1mhz: self.cemo.q,
            ch3_2mhz: self.cery.q,
            ch4_1mhz_i: !self.avok.q,
            ch4_1mhz: w.ch4_1mhz_i,
            khz512: self.jeso.q,
            hz512: if test { self.jeso.q } else { !self.bara.q },
            hz256: if test { self.jeso.q } else { self.caru.q },
            hz128_i: if test { self.jeso.q } else { self.bylu.q },
            hz128: w.hz128_i,
            hz128_ch1: if test { self.baza.q } else { w.hz128_i },
            apu_phi: !i.data_phase,
        }
    }

    /// One delta cycle into `next`: the processes on the signals as they
    /// are (`self`), with their edges against the delta before (`prev`).
    pub const fn delta(&self, prev: &Self, i: &ApuInputs, pi: &ApuInputs, next: &mut Self) {
        let (w, pw) = (&self.wires, &prev.wires);
        next.wires = self.next_wires(i);
        let reset = w.apu_reset_i;

        // NR52 bit 7, as the CPU's write ends.
        // @hw die: HADA (dffr), HAWU GUFO HAPO; JYRO KUBY KEBA KEPY
        if i.reset2 {
            next.power.set(false);
        } else if fell(pw.write_nr52, w.write_nr52) {
            next.power.after_tco(i.data & 0x80 != 0);
        } else {
            // The flip-flop holds.
        }

        // Test mode: NR52 bit 4, writable with T1 alone, and bit 5.
        // @hw die: FERO (drlatch_ee), EFOP; BOWY (dffr), BAZA (dffr)
        if reset {
            next.test = false;
        } else if w.write_nr52 {
            next.test = (i.data & 0x10 != 0) & i.t1_only;
        } else {
            // The latch holds.
        }
        if reset {
            next.test_bit5.set(false);
            next.baza.set(false);
        } else {
            if fell(pw.write_nr52, w.write_nr52) {
                next.test_bit5.after_tco(i.data & 0x20 != 0);
            }
            if fell(prev.ajer.q, self.ajer.q) {
                next.baza.after_tco(self.test_bit5.q);
            }
        }

        // NR50 and NR51.
        // @hw die: APEG..BEDU, ANEV..BEPU (drlatch_ee)
        if reset {
            next.nr50 = 0;
            next.nr51 = 0;
        } else {
            if w.write_nr50 {
                next.nr50 = i.data;
            }
            if w.write_nr51 {
                next.nr51 = i.data;
            }
        }

        // The dividers.
        // @hw die: AJER CALO ATEP CEMO CERY ATYK AVOK JESO (dffr)
        if reset {
            next.ajer.set(false);
            next.atep.set(false);
            next.cery.set(false);
            next.atyk.set(false);
        } else if rose(pi.clk, i.clk) {
            next.ajer.after_tco(!self.ajer.q);
            next.atep.after_tco(!self.atep.q);
            next.cery.after_tco(!self.cery.q);
            next.atyk.after_tco(!self.atyk.q);
        } else {
            // The flip-flops hold.
        }
        if reset {
            next.calo.set(false);
            next.cemo.set(false);
            next.avok.set(false);
        } else {
            if fell(prev.ajer.q, self.ajer.q) {
                next.calo.after_tco(!self.calo.q);
            }
            if fell(prev.atep.q, self.atep.q) {
                next.cemo.after_tco(!self.cemo.q);
            }
            if fell(prev.atyk.q, self.atyk.q) {
                next.avok.after_tco(!self.avok.q);
            }
        }
        if reset {
            next.jeso.set(false);
        } else if rose(pw.ch4_1mhz_i, w.ch4_1mhz_i) {
            next.jeso.after_tco(!self.jeso.q);
        } else {
            // The flip-flop holds.
        }

        // The frame sequencer: the divider's bit 10, inverted, at 2 MiHz's
        // fall; 256 and 128 Hz halve it.
        // @hw die: BARA CARU BYLU (dffr), COKE BURE CULO APEF
        if reset {
            next.bara.set(false);
        } else if fell(prev.ajer.q, self.ajer.q) {
            next.bara.after_tco(!i.div10);
        } else {
            // The flip-flop holds.
        }
        if reset {
            next.caru.set(false);
            next.bylu.set(false);
        } else {
            if rose(prev.bara.q, self.bara.q) {
                next.caru.after_tco(!self.caru.q);
            }
            if fell(prev.caru.q, self.caru.q) {
                next.bylu.after_tco(!self.bylu.q);
            }
        }
    }
}
