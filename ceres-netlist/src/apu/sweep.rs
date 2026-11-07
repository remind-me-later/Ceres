//! Channel 1's frequency and sweep (`hdl/rtl/apu_ch1.vhd`). The frequency
//! is flip-flops that the CPU sets and clears directly while it writes NR13
//! and NR14, and that take the sweep's sum when it updates. The sweep's
//! period counts the falls of 128 Hz; at its end a step loads the shifter
//! with the frequency, which shifts right once a microsecond, NR10's shift
//! times; then the sum's registers take the frequency and the shifted
//! frequency (inverted to subtract), and a step writes their sum back. A
//! sum above 2047 stops the channel.
//!
//! @hw die: dmg-schematics, category apu-ch1 (sheets "NR10 to NR14" and
//! "Sweep")

use super::{ApuInputs, ControlWires, Ff, NetSquare, SquareWires, fell, rose};

/// Channel 1's frequency and sweep.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct NetSweep {
    /// NR10: a latch, open while the CPU writes it.
    pub sweep: u8,
    /// The frequency: bits 0 to 7 (NR13) and 8 to 10 (NR14). HYKA..AXAN
    pub acc_low: Ff<u8>,
    pub acc_high: Ff<u8>,
    /// The sweep's period and its step. CUPO CYPU CAXY; BEXA
    pub sweep_count: Ff<u8>,
    pub sweep_step: Ff<bool>,
    /// The shifter: shifting, its count, its end, the shifted frequency.
    /// ~FEMU (latch); COPA CAJA BYRA; BYTE
    pub shifting: bool,
    pub shifts: Ff<u8>,
    pub shift_done: Ff<bool>,
    pub shifted: Ff<u16>,
    /// The sum's registers.
    pub augend: Ff<u16>,
    pub addend: Ff<u16>,
    pub wires: SweepWires,
}

/// The sweep's concurrent statements, each a delta behind its inputs.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[expect(clippy::struct_excessive_bools, reason = "the wires' levels")]
pub struct SweepWires {
    pub write_nr10: bool,
    pub no_sweep: bool,
    pub load_sweep: bool,
    pub load_shift: bool,
    pub shift_clock: bool,
    pub negate: bool,
    pub sum_reset: bool,
    pub sum: u16,
    pub overflow: bool,
    pub update: bool,
}

impl NetSweep {
    /// The frequency.
    #[must_use]
    pub const fn acc(&self) -> u16 {
        u16::from_le_bytes([self.acc_low.q, self.acc_high.q])
    }

    /// A flip-flop's change is pending.
    #[must_use]
    pub const fn any_due(&self) -> bool {
        self.acc_low.due
            | self.acc_high.due
            | self.sweep_count.due
            | self.sweep_step.due
            | self.shifts.due
            | self.shift_done.due
            | self.shifted.due
            | self.augend.due
            | self.addend.due
    }

    /// The next nanosecond.
    pub const fn advance(&mut self) {
        self.acc_low.advance();
        self.acc_high.advance();
        self.sweep_count.advance();
        self.sweep_step.advance();
        self.shifts.advance();
        self.shift_done.advance();
        self.shifted.advance();
        self.augend.advance();
        self.addend.advance();
    }

    /// What a read of NR10 drives on the data bus; its bit 7 stays
    /// precharged, 1.
    // @hw die: ATYN ASOP
    #[must_use]
    pub const fn read(&self, i: &ApuInputs) -> Option<u8> {
        if i.cpu_rd & (i.addr == 0xFF10) {
            Some(0x80 | self.sweep)
        } else {
            None
        }
    }

    /// The wires, from the signals as they are, with channel 1's square
    /// part (`square`) and its clock.
    // @hw die: KALA (ld_shift); EGYP DODY EGOR (shift_clk); ARYL; GUXA..CORU
    // (full_add); BYLE ATYS BONE; ATUV BUSO BOJE
    #[must_use]
    pub const fn next_wires(
        &self,
        i: &ApuInputs,
        cw: &ControlWires,
        square: &NetSquare,
    ) -> SweepWires {
        let w = &self.wires;
        let negate = self.sweep & 8 != 0;
        SweepWires {
            write_nr10: i.cpu_wr & (i.addr == 0xFF10),
            no_sweep: self.sweep & 7 == 0,
            load_sweep: self.sweep_step.q | square.restart.q,
            load_shift: self.sweep_step.q | square.restart.q,
            shift_clock: w.no_sweep | (self.shifting & !cw.ch1_1mhz),
            negate,
            sum_reset: cw.apu_reset | square.restart.q,
            sum: (self.augend.q + self.addend.q + w.negate as u16) & 0xFFF,
            overflow: (w.sum & 0x800 != 0) & !w.negate,
            update: self.sweep_step.q & !w.overflow & !w.no_sweep,
        }
    }

    /// One delta cycle into `next`: the processes on the signals as they
    /// are (`self`), with their edges against the delta before (`prev`);
    /// `square` is channel 1's square part, and `sw` its wires.
    #[expect(clippy::too_many_arguments, reason = "the sweep's inputs")]
    pub const fn delta(
        &self,
        prev: &Self,
        i: &ApuInputs,
        cw: &ControlWires,
        pcw: &ControlWires,
        square: &NetSquare,
        sw: &SquareWires,
        next: &mut Self,
    ) {
        let (w, pw) = (&self.wires, &prev.wires);
        next.wires = self.next_wires(i, cw, square);
        let reset = cw.apu_reset;

        // NR10.
        // @hw die: BANY ARAX ANAZ AVAF ADEK BANA BOTU (drlatch_ee)
        if reset {
            next.sweep = 0;
        } else if w.write_nr10 {
            next.sweep = i.data & 0x7F;
        } else {
            // The latch holds.
        }

        // The frequency: set and cleared while the CPU writes NR13 and
        // NR14, else the sum at the update's rise.
        // @hw die: HYKA..AXAN (dffsr)
        let update_rose = rose(pw.update, w.update);
        if reset {
            next.acc_low.set(0);
        } else if sw.write_3 {
            next.acc_low.set(i.data);
        } else if update_rose {
            next.acc_low.after_tco(w.sum.to_le_bytes()[0]);
        } else {
            // The flip-flops hold.
        }
        if reset {
            next.acc_high.set(0);
        } else if sw.write_4 {
            next.acc_high.set(i.data & 7);
        } else if update_rose {
            next.acc_high.after_tco(w.sum.to_le_bytes()[1] & 7);
        } else {
            // The flip-flops hold.
        }

        // The sweep's period, and its step at 2 MiHz's rise.
        // @hw die: CUPO CYPU CAXY (tffnl), DAFA CYMU; COZE, BEXA (dffr)
        if w.load_sweep {
            next.sweep_count.set(!self.sweep >> 4 & 7);
        } else if fell(pcw.hz128_ch1, cw.hz128_ch1) {
            next.sweep_count.after_tco((self.sweep_count.q + 1) & 7);
        } else {
            // The counter holds.
        }
        if reset | (self.sweep & 0x70 == 0) {
            next.sweep_step.set(false);
        } else if rose(pcw.ajer, cw.ajer) {
            next.sweep_step.after_tco(self.sweep_count.q == 7);
        } else {
            // The flip-flop holds.
        }

        // The shifter.
        // @hw die: FEMU (nand_latch), EVOL EPUK; COPA CAJA BYRA (tffnl);
        // BYTE (dffr); FABU..BEKU (dffsr)
        if reset | self.shift_done.q {
            next.shifting = false;
        } else if self.sweep_step.q | square.running.q {
            next.shifting = true;
        } else {
            // The latch holds.
        }
        let shift_rose = rose(pw.shift_clock, w.shift_clock);
        if w.load_shift {
            next.shifts.set(!self.sweep & 7);
        } else if shift_rose {
            next.shifts.after_tco((self.shifts.q + 1) & 7);
        } else {
            // The counter holds.
        }
        if reset | self.sweep_step.q {
            next.shift_done.set(false);
        } else if rose(pcw.ajer, cw.ajer) {
            next.shift_done.after_tco(self.shifts.q == 7);
        } else {
            // The flip-flop holds.
        }
        if reset {
            next.shifted.set(0);
        } else if w.load_shift {
            next.shifted.set(self.acc());
        } else if shift_rose {
            next.shifted.after_tco(self.shifted.q >> 1);
        } else {
            // The shifter holds.
        }

        // The sum's registers: the frequency and the shifted frequency, or
        // its inverse.
        // @hw die: GALO..DOLY, HORA..DEVA (dffr_cc_q)
        if w.sum_reset {
            next.augend.set(0);
            next.addend.set(0);
        } else if rose(prev.shift_done.q, self.shift_done.q) {
            next.augend.after_tco(self.acc());
            next.addend
                .after_tco(self.shifted.q ^ if w.negate { 0x7FF } else { 0 });
        } else {
            // The flip-flops hold.
        }
    }
}
