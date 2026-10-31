//! The APU's channel 4 (`hdl/rtl/apu_ch4.vhd`): the noise. NR41 sets the
//! length, NR42 the envelope, NR43 the clock's divider, its shift and the
//! LFSR's width, NR44 the length's enable and the trigger. A divider of
//! 512 KiHz clocks a prescaler, one of whose bits clocks the LFSR.
//!
//! @hw die: dmg-schematics, category apu-ch4 (sheets "Channel 4", "NR41 to
//! NR44")

use super::{ApuInputs, ControlWires, Ff, fell, rose};

/// Channel 4's flip-flops and latches.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's latches")]
pub struct NetNoise {
    /// NR42, NR43, NR44's length enable and trigger: latches.
    pub envelope: u8,
    pub noise: u8,
    pub length_on: bool,
    pub trigger: bool,
    /// The trigger, at `apu_phi`'s rise, then at 512 KiHz. GYSU; ~HAZO
    /// (latch); GONE GORA GATY
    pub start: Ff<bool>,
    pub triggered: bool,
    pub restart: Ff<bool>,
    pub restarted: Ff<bool>,
    pub running: Ff<bool>,
    /// JERY, GENA (latches)
    pub frozen: bool,
    pub active: bool,
    /// The divider, its end, the prescaler. GARY
    pub divider: Ff<u8>,
    pub divided: Ff<bool>,
    pub prescaler: Ff<u16>,
    /// The LFSR's first stage, on the other edge, and its 15 stages (bit
    /// 0 is the first, as the trace prints them). JOTO
    pub joto: Ff<bool>,
    pub lfsr: Ff<u16>,
    /// The length and its end. FUGO
    pub length: Ff<u8>,
    pub expired: Ff<bool>,
    /// The envelope, as the square channels'. ABEL, FOSY, FYNO, EROX
    /// (latch)
    pub hz64: Ff<bool>,
    pub period: Ff<u8>,
    pub tick: Ff<bool>,
    pub volume: [Ff<bool>; 4],
    pub at_limit: Ff<bool>,
    pub env_stopped: bool,
    pub wires: NoiseWires,
}

/// Channel 4's concurrent statements, each a delta behind its inputs.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the wires' levels")]
pub struct NoiseWires {
    pub write_1: bool,
    pub write_2: bool,
    pub write_3: bool,
    pub write_4: bool,
    pub write_4_late: bool,
    pub dac_on_i: bool,
    pub dac_on: bool,
    pub active: bool,
    pub output: u8,
    pub stop: bool,
    pub divider_clock: bool,
    pub load_divider: bool,
    pub counter_clock: bool,
    pub shift_clock: bool,
    pub bypass: bool,
    pub feedback: bool,
    pub wave: bool,
    pub length_clock: bool,
    pub length_reset: bool,
    pub load_period: bool,
    pub no_period: bool,
    pub tick_reset: bool,
    pub volume_clock: bool,
    pub volume_clocks: [bool; 4],
}

impl NetNoise {
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
            | self.divider.due
            | self.divided.due
            | self.prescaler.due
            | self.joto.due
            | self.lfsr.due
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
        self.divider.advance();
        self.divided.advance();
        self.prescaler.advance();
        self.joto.advance();
        self.lfsr.advance();
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

    /// What a read of NR42, NR43 or NR44 drives on the data bus: NR44's
    /// length enable, and in test mode (`test`) the counter's clock where
    /// the length bypasses it. The bits nothing drives stay precharged, 1.
    // @hw die: HASU, HEZE KEKA JORA, BARE CURY; BAGU ATEL (test)
    #[must_use]
    pub const fn read(&self, i: &ApuInputs, test: bool) -> Option<u8> {
        let length_on = (self.length_on as u8) << 6;
        if !i.cpu_rd {
            None
        } else if i.addr == 0xFF21 {
            Some(self.envelope)
        } else if i.addr == 0xFF22 {
            Some(self.noise)
        } else if (i.addr == 0xFF23) & test {
            Some(0x80 | length_on | 0x3E | (self.wires.counter_clock & self.wires.bypass) as u8)
        } else if i.addr == 0xFF23 {
            Some(0x80 | length_on | 0x3F)
        } else {
            None
        }
    }

    /// The wires, from the signals as they are.
    #[must_use]
    pub const fn next_wires(&self, i: &ApuInputs, cw: &ControlWires) -> NoiseWires {
        let w = &self.wires;
        let shift = self.noise >> 4;
        let v = [self.volume[0].q, self.volume[1].q, self.volume[2].q];
        let up = self.envelope & 8 != 0;
        let bypass = self.length_on & cw.test;
        NoiseWires {
            write_1: i.cpu_wr & (i.addr == 0xFF20),
            write_2: i.cpu_wr & (i.addr == 0xFF21),
            write_3: i.cpu_wr & (i.addr == 0xFF22),
            write_4: i.cpu_wr & (i.addr == 0xFF23),
            write_4_late: w.write_4 & i.clk_t4,
            dac_on_i: self.envelope & 0xF8 != 0,
            dac_on: w.dac_on_i,
            active: self.active,
            output: if w.wave | (cw.test & !self.length_on) {
                self.volume()
            } else {
                0
            },
            stop: cw.apu_reset | (self.expired.q & self.length_on) | !w.dac_on_i,
            divider_clock: self.frozen | cw.khz512,
            load_divider: self.restart.q | self.divided.q,
            counter_clock: cw.ch4_1mhz & self.divided.q,
            shift_clock: if shift <= 13 {
                self.prescaler.q >> shift & 1 != 0
            } else {
                false
            },
            bypass,
            feedback: !((self.lfsr.q >> 13 & 1 != 0) ^ (self.lfsr.q >> 14 & 1 != 0)),
            wave: if w.bypass {
                w.shift_clock
            } else {
                self.active & (self.lfsr.q >> 14 & 1 != 0)
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
    pub const fn delta(
        &self,
        prev: &Self,
        i: &ApuInputs,
        cw: &ControlWires,
        pcw: &ControlWires,
        next: &mut Self,
    ) {
        let (w, pw) = (&self.wires, &prev.wires);
        next.wires = self.next_wires(i, cw);
        let reset = cw.apu_reset;

        // The registers.
        // @hw die: the drlatch_ee of NR42 to NR44
        if reset {
            next.envelope = 0;
            next.noise = 0;
            next.length_on = false;
        } else {
            if w.write_2 {
                next.envelope = i.data;
            }
            if w.write_3 {
                next.noise = i.data;
            }
            if w.write_4_late {
                next.length_on = i.data & 0x40 != 0;
            }
        }

        // The trigger, at `apu_phi`'s rise, then at 512 KiHz.
        // @hw die: GYSU (dffr); HAZO (nor_latch), GONE GORA GATY (dffr),
        // JERY (nand_latch); GENA (nor_latch)
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
        let khz512_rose = rose(pcw.khz512, cw.khz512);
        if reset {
            next.restarted.set(false);
            next.running.set(false);
        } else if khz512_rose {
            next.restarted.after_tco(self.restart.q);
            next.running.after_tco(self.restarted.q);
        } else {
            // The flip-flops hold.
        }
        if reset | self.restarted.q {
            next.restart.set(false);
        } else if khz512_rose {
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

        // The divider, the prescaler, the LFSR.
        // @hw die: the divider's (tffnl), GARY (dffr); the prescaler's
        // (tffnl); JOTO and the LFSR's (dffr)
        if w.load_divider {
            next.divider.set(!self.noise & 7);
        } else if fell(pw.divider_clock, w.divider_clock) {
            next.divider.after_tco((self.divider.q + 1) & 7);
        } else {
            // The counter holds.
        }
        if reset | self.restart.q {
            next.divided.set(false);
        } else if fell(pcw.ch4_1mhz, cw.ch4_1mhz) {
            next.divided.after_tco(self.divider.q == 7);
        } else {
            // The flip-flop holds.
        }
        if reset {
            next.prescaler.set(0);
        } else if rose(pw.counter_clock, w.counter_clock) {
            next.prescaler.after_tco((self.prescaler.q + 1) & 0x3FFF);
        } else {
            // The counter holds.
        }
        if self.restart.q | reset {
            next.joto.set(false);
        } else if fell(pw.shift_clock, w.shift_clock) {
            next.joto.after_tco(w.feedback);
        } else {
            // The flip-flop holds.
        }
        if self.restart.q | reset {
            next.lfsr.set(0);
        } else if rose(pw.shift_clock, w.shift_clock) {
            let joto = self.joto.q as u16;
            let mut shifted = (self.lfsr.q << 1) & 0x7FFE | joto;
            if self.noise & 8 != 0 {
                // The short LFSR: stage 9 takes the first stage's input too.
                shifted = shifted & !0x100 | joto << 8;
            }
            next.lfsr.after_tco(shifted);
        } else {
            // The stages hold.
        }

        // The length.
        // @hw die: the length's (tffnl), FUGO (dffr)
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
        // @hw die: ABEL (dffr); the period's (tffnl); FOSY (dffr); the
        // volume's (tffnl); FYNO (dffr), EROX (nor_latch)
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
        let mut stage = 0;
        while stage < 4 {
            if self.restart.q {
                next.volume[stage].set(self.envelope >> (4 + stage) & 1 != 0);
            } else if fell(pw.volume_clocks[stage], w.volume_clocks[stage]) {
                next.volume[stage].after_tco(!self.volume[stage].q);
            } else {
                // The stage holds.
            }
            stage += 1;
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
