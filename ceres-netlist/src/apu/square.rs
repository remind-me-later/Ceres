//! The APU's square channel (`hdl/rtl/apu_ch2.vhd`, and channel 1's
//! square wave, length and envelope in `apu_ch1.vhd`): the duty, the
//! length, the envelope, the trigger, and the frequency counter whose
//! overflow steps the duty.
//!
//! @hw die: dmg-schematics, categories apu-ch2 and apu-ch1 (sheets
//! "Channel 2", "NR21 to NR24")

use super::{ApuInputs, ControlWires, Ff, fell, rose};

/// A square channel's flip-flops and latches.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's latches")]
pub struct NetSquare {
    /// NRx1's duty, NRx2, the frequency (NRx3, NRx4), NRx4's length enable
    /// and trigger: latches, open while the CPU writes them (the length's
    /// enable only in the write's last two phases).
    pub duty: u8,
    pub envelope: u8,
    pub frequency: u16,
    pub length_on: bool,
    pub trigger: bool,
    /// The trigger: taken at `apu_phi`'s rise, then at the channel's clock.
    /// DOPE; ~DALA (latch); ELOX DORY CAZA
    pub start: Ff<bool>,
    pub triggered: bool,
    pub restart: Ff<bool>,
    pub restarted: Ff<bool>,
    pub running: Ff<bool>,
    /// The frequency counter does not count; the channel plays (NR52).
    /// BUTA, DANE (latches)
    pub frozen: bool,
    pub active: bool,
    /// The frequency counter, its overflow, the duty's step, the wave.
    /// GYKO, DOME
    pub counter: Ff<u16>,
    pub reload: Ff<bool>,
    pub step: Ff<u8>,
    pub wave: Ff<bool>,
    /// The length counter and its end. CYRE
    pub length: Ff<u8>,
    pub expired: Ff<bool>,
    /// The envelope: 64 Hz, its period, its tick, the volume's four
    /// stages, the limit and the stop. JYNA, JOPA, HEPO, JEME (latch)
    pub hz64: Ff<bool>,
    pub period: Ff<u8>,
    pub tick: Ff<bool>,
    pub volume: [Ff<bool>; 4],
    pub at_limit: Ff<bool>,
    pub env_stopped: bool,
    pub wires: SquareWires,
}

/// A square channel's concurrent statements, each a delta behind its
/// inputs.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the wires' levels")]
pub struct SquareWires {
    pub write_1: bool,
    pub write_2: bool,
    pub write_3: bool,
    pub write_4: bool,
    pub write_4_late: bool,
    pub dac_on_i: bool,
    /// The ports: the DAC, the channel's play (NR52), its output.
    pub dac_on: bool,
    pub active: bool,
    pub output: u8,
    pub stop: bool,
    pub count_clock: bool,
    pub load_counter: bool,
    pub pwm: bool,
    pub length_clock: bool,
    pub length_reset: bool,
    pub load_period: bool,
    pub no_period: bool,
    pub tick_reset: bool,
    pub volume_clock: bool,
    /// Each volume stage's clock: the stage before it, or its inverse.
    pub volume_clocks: [bool; 4],
}

/// The channel's clock (its 1 MiHz) as it is, and in the delta before;
/// the address of its first register (NRx1); and, for channel 1, its swept
/// frequency (`acc`, not NRx3's and NRx4's latches) and the sweep's
/// overflow, which stops it.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the clocks' levels")]
pub struct Clocks {
    pub channel: bool,
    pub channel_before: bool,
    pub nrx1: u16,
    pub swept: bool,
    pub acc: u16,
    pub overflow: bool,
}

impl NetSquare {
    /// The volume, from its four stages.
    #[must_use]
    pub const fn volume(&self) -> u8 {
        (self.volume[0].q as u8)
            | (self.volume[1].q as u8) << 1
            | (self.volume[2].q as u8) << 2
            | (self.volume[3].q as u8) << 3
    }

    /// A flip-flop's change is pending.
    #[must_use]
    pub const fn any_due(&self) -> bool {
        self.start.due
            | self.restart.due
            | self.restarted.due
            | self.running.due
            | self.counter.due
            | self.reload.due
            | self.step.due
            | self.wave.due
            | self.length.due
            | self.expired.due
            | self.hz64.due
            | self.period.due
            | self.tick.due
            | self.volume[0].due
            | self.volume[1].due
            | self.volume[2].due
            | self.volume[3].due
            | self.at_limit.due
    }

    /// The next nanosecond.
    pub const fn advance(&mut self) {
        self.start.advance();
        self.restart.advance();
        self.restarted.advance();
        self.running.advance();
        self.counter.advance();
        self.reload.advance();
        self.step.advance();
        self.wave.advance();
        self.length.advance();
        self.expired.advance();
        self.hz64.advance();
        self.period.advance();
        self.tick.advance();
        self.volume[0].advance();
        self.volume[1].advance();
        self.volume[2].advance();
        self.volume[3].advance();
        self.at_limit.advance();
    }

    /// What a read of the channel's registers from NRx1 (`nrx1`) on drives
    /// on the data bus: the duty, NRx2, NRx4's length enable; in test mode
    /// (`test`), NRx3 and NRx4 read the frequency counter. The bits nothing
    /// drives stay precharged, 1.
    // @hw die: BEXU, HAMY HOCU, BALE BYTU; DAXA DYPU EVAJ DUPY (channel 1);
    // CORO CECY CEKA, GURU GEXA HUVU..HYRY, HUMA GOJY; FAPE GUZA FUTY HYPO,
    // FAVA..GAZO HUNA JARO JEKE (channel 2)
    #[must_use]
    pub const fn read(&self, i: &ApuInputs, test: bool, nrx1: u16) -> Option<u8> {
        let counter = self.counter.q.to_le_bytes();
        let length_on = (self.length_on as u8) << 6;
        if !i.cpu_rd {
            None
        } else if i.addr == nrx1 {
            Some(self.duty << 6 | 0x3F)
        } else if i.addr == nrx1 + 1 {
            Some(self.envelope)
        } else if (i.addr == nrx1 + 2) & test {
            Some(counter[0])
        } else if (i.addr == nrx1 + 3) & test {
            Some(0x80 | length_on | 0x38 | counter[1] & 7)
        } else if i.addr == nrx1 + 3 {
            Some(0x80 | length_on | 0x3F)
        } else {
            None
        }
    }

    /// The wires, from the signals as they are.
    // @hw die: the registers' write strobes; HOCA; COWE BOTO, ACEG AGOF ASON
    // AMOP (the output)
    #[must_use]
    pub const fn next_wires(
        &self,
        i: &ApuInputs,
        cw: &ControlWires,
        clocks: Clocks,
    ) -> SquareWires {
        let nrx1 = clocks.nrx1;
        let w = &self.wires;
        let write_4 = i.cpu_wr & (i.addr == nrx1 + 3);
        let step = self.step.q;
        let (s2, s1, s0) = (step & 4 != 0, step & 2 != 0, step & 1 != 0);
        let v = [self.volume[0].q, self.volume[1].q, self.volume[2].q];
        let up = self.envelope & 8 != 0;
        SquareWires {
            write_1: i.cpu_wr & (i.addr == nrx1),
            write_2: i.cpu_wr & (i.addr == nrx1 + 1),
            write_3: i.cpu_wr & (i.addr == nrx1 + 2),
            write_4,
            write_4_late: w.write_4 & i.clk_t4,
            dac_on_i: self.envelope & 0xF8 != 0,
            dac_on: w.dac_on_i,
            active: self.active,
            output: if (self.active & self.wave.q) | cw.test {
                self.volume()
            } else {
                0
            },
            stop: cw.apu_reset | (self.expired.q & self.length_on) | clocks.overflow | !w.dac_on_i,
            count_clock: clocks.channel | self.frozen,
            load_counter: self.reload.q | self.restart.q,
            pwm: match self.duty & 3 {
                0 => s2 & s1 & !s0,
                1 => s2 & s1,
                2 => s2,
                _ => !(s2 & s1),
            },
            length_clock: !(self.length_on & !self.expired.q & !cw.hz256),
            length_reset: w.write_1 | cw.apu_reset | self.restart.q,
            load_period: self.restart.q | self.tick.q,
            no_period: self.envelope & 7 == 0,
            tick_reset: (!cw.hz512 & self.tick.q) | w.no_period | self.restart.q | cw.apu_reset,
            volume_clock: self.tick.q | w.no_period | self.env_stopped,
            volume_clocks: [
                w.volume_clock,
                if up { v[0] } else { !v[0] },
                if up { v[1] } else { !v[1] },
                if up { v[2] } else { !v[2] },
            ],
        }
    }

    /// One delta cycle into `next`: the processes on the signals as they
    /// are (`self`), with their edges against the delta before (`prev`).
    #[expect(
        clippy::too_many_lines,
        reason = "the channel's processes in the VHDL's order"
    )]
    #[expect(
        clippy::cognitive_complexity,
        reason = "the channel's processes in the VHDL's order"
    )]
    pub fn delta(
        &self,
        prev: &Self,
        i: &ApuInputs,
        cw: &ControlWires,
        pcw: &ControlWires,
        clocks: Clocks,
        next: &mut Self,
    ) {
        let (w, pw) = (&self.wires, &prev.wires);
        next.wires = self.next_wires(i, cw, clocks);
        let reset = cw.apu_reset;

        // The registers.
        // @hw die: the drlatch_ee of NRx1 to NRx4
        if reset {
            next.duty = 0;
            next.envelope = 0;
            next.frequency = 0;
            next.length_on = false;
        } else {
            if w.write_1 {
                next.duty = i.data >> 6;
            }
            if w.write_2 {
                next.envelope = i.data;
            }
            if w.write_3 & !clocks.swept {
                next.frequency = next.frequency & 0x700 | u16::from(i.data);
            }
            if w.write_4 & !clocks.swept {
                next.frequency = next.frequency & 0xFF | u16::from(i.data & 7) << 8;
            }
            if w.write_4_late {
                next.length_on = i.data & 0x40 != 0;
            }
        }

        // The trigger: taken as `apu_phi` rises, then at the channel's
        // clock.
        // @hw die: DUPE (drlatch_ee), DOPE (dffr); DALA (nor_latch), ELOX
        // DORY CAZA (dffr), BUTA (nand_latch); DANE (nor_latch)
        if reset | self.start.q {
            next.trigger = false;
        } else if w.write_4 {
            next.trigger = i.data & 0x80 != 0;
        } else {
            // The latch holds.
        }
        if reset {
            next.start.set(false);
        } else if rose(pcw.apu_phi, cw.apu_phi) {
            next.start.after_tco(self.trigger);
        } else {
            // The flip-flop holds.
        }
        if reset | self.restarted.q {
            next.triggered = false;
        } else if self.start.q {
            next.triggered = true;
        } else {
            // The latch holds.
        }
        let channel_rose = rose(clocks.channel_before, clocks.channel);
        if reset {
            next.restarted.set(false);
            next.running.set(false);
        } else if channel_rose {
            next.restarted.after_tco(self.restart.q);
            next.running.after_tco(self.restarted.q);
        } else {
            // The flip-flops hold.
        }
        if reset | self.restarted.q {
            next.restart.set(false);
        } else if channel_rose {
            next.restart.after_tco(self.triggered);
        } else {
            // The flip-flop holds.
        }
        if !w.dac_on_i | reset {
            next.frozen = true;
        } else if self.running.q {
            next.frozen = false;
        } else {
            // The latch holds.
        }
        if w.stop {
            next.active = false;
        } else if self.restart.q {
            next.active = true;
        } else {
            // The latch holds.
        }

        // The frequency counter, its overflow, the duty and the wave.
        // @hw die: GAXE..COPU (tffnl), GYKO (dffr); the duty's (dffr_cc);
        // DOME (dffr)
        if w.load_counter {
            next.counter.set(if clocks.swept {
                clocks.acc
            } else {
                self.frequency
            });
        } else if fell(pw.count_clock, w.count_clock) {
            next.counter.after_tco((self.counter.q + 1) & 0x7FF);
        } else {
            // The counter holds.
        }
        if reset | self.restart.q | (self.reload.q & clocks.channel) {
            next.reload.set(false);
        } else if fell(prev.counter.q & 0x400 != 0, self.counter.q & 0x400 != 0) {
            next.reload.after_tco(true);
        } else {
            // The flip-flop holds.
        }
        if reset {
            next.step.set(0);
        } else if fell(prev.reload.q, self.reload.q) {
            next.step.after_tco((self.step.q + 1) & 7);
        } else {
            // The counter holds.
        }
        if reset {
            next.wave.set(false);
        } else if rose(prev.reload.q, self.reload.q) {
            next.wave.after_tco(w.pwm);
        } else {
            // The flip-flop holds.
        }

        // The length.
        // @hw die: the length's (tffnl), CYRE (dffr)
        if w.write_1 {
            next.length.set(i.data & 0x3F);
        } else if fell(pw.length_clock, w.length_clock) {
            next.length.after_tco((self.length.q + 1) & 0x3F);
        } else {
            // The counter holds.
        }
        if w.length_reset {
            next.expired.set(false);
        } else if fell(prev.length.q & 0x20 != 0, self.length.q & 0x20 != 0) {
            next.expired.after_tco(!self.expired.q);
        } else {
            // The flip-flop holds.
        }

        // The envelope.
        // @hw die: JYNA (dffr); the period's (tffnl); JOPA (dffr); the
        // volume's (tffnl); HEPO (dffr), JEME (nor_latch)
        if reset {
            next.hz64.set(false);
        } else if fell(pcw.hz128, cw.hz128) {
            next.hz64.after_tco(!self.hz64.q);
        } else {
            // The flip-flop holds.
        }
        if w.load_period {
            next.period.set(!self.envelope & 7);
        } else if fell(prev.hz64.q, self.hz64.q) {
            next.period.after_tco((self.period.q + 1) & 7);
        } else {
            // The counter holds.
        }
        if w.tick_reset {
            next.tick.set(false);
        } else if rose(pcw.hz512, cw.hz512) {
            next.tick.after_tco(self.period.q == 7);
        } else {
            // The flip-flop holds.
        }
        for stage in 0..4 {
            if self.restart.q {
                next.volume[stage].set(self.envelope >> (4 + stage) & 1 != 0);
            } else if fell(pw.volume_clocks[stage], w.volume_clocks[stage]) {
                next.volume[stage].after_tco(!self.volume[stage].q);
            } else {
                // The stage holds.
            }
        }
        let up = self.envelope & 8 != 0;
        let volume = self.volume();
        if self.restart.q | reset {
            next.at_limit.set(false);
        } else if rose(prev.tick.q, self.tick.q) {
            next.at_limit
                .after_tco((!up & (volume == 0)) | (up & (volume == 15)));
        } else {
            // The flip-flop holds.
        }
        if reset | self.restart.q {
            next.env_stopped = false;
        } else if self.at_limit.q {
            next.env_stopped = true;
        } else {
            // The latch holds.
        }
    }
}
