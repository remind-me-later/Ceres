//! The APU's channel 3 (`hdl/rtl/apu_ch3.vhd`): the wave channel. NR30
//! turns its DAC on, NR31 sets the length, NR32 the output level, NR33 and
//! NR34 the frequency, the length's enable and the trigger; wave RAM
//! (FF30 to FF3F) holds 32 samples of 4 bits.
//!
//! The frequency counter counts at 2 MiHz; each overflow steps the position
//! in wave RAM, which is read just after: while the channel plays, the RAM
//! is addressed by the position, and a CPU's read of it gets the byte the
//! channel reads. The RAM is dmg-sim's simplified SRAM: bit lines that are
//! precharged, then driven by a write or by the word line's cells.
//!
//! @hw die: dmg-schematics, category apu-ch3 (sheets "Channel 3", "NR30 to
//! NR34" and "Wave RAM")

use super::{ApuInputs, ControlWires, Ff, fell, rose};

/// Channel 3's flip-flops, latches and RAM.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the netlist's latches")]
pub struct NetWave {
    /// NR30's DAC, NR32's level, the frequency, NR34's length enable and
    /// trigger: latches.
    pub dac_on: bool,
    pub level: u8,
    pub frequency: u16,
    pub length_on: bool,
    pub trigger: bool,
    /// The trigger, at `apu_phi`'s rise, then at 2 MiHz's edges. FOBA;
    /// ~GOFY (latch); GARA GYTA GYRA
    pub start: Ff<bool>,
    pub triggered: bool,
    pub restart: Ff<bool>,
    pub restarted: Ff<bool>,
    pub running: Ff<bool>,
    /// The counter does not count; the channel plays, and at 2 MiHz (NR52).
    /// GUGU, FOZU (latches); DAVO
    pub frozen: bool,
    pub playing: bool,
    pub active: Ff<bool>,
    /// The frequency counter, its overflow, the position, its wrap. HUNO,
    /// FETY
    pub counter: Ff<u16>,
    pub reload: Ff<bool>,
    pub position: Ff<u8>,
    pub wrapped: Ff<bool>,
    /// The length (8 bits) and its end. FEXU
    pub length: Ff<u8>,
    pub expired: Ff<bool>,
    /// The RAM's read after a step, on the chip's clock's edges. BUSA BANO
    /// AZUS AZET
    pub busa: Ff<bool>,
    pub bano: Ff<bool>,
    pub fetch: Ff<bool>,
    pub azet: Ff<bool>,
    /// The bit lines, the sample's latch, the RAM.
    pub bit_lines: u8,
    pub sample_byte: u8,
    pub ram: [u8; 16],
    pub wires: WaveWires,
}

/// Channel 3's concurrent statements, each a delta behind its inputs.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the wires' levels")]
pub struct WaveWires {
    pub write_0: bool,
    pub write_1: bool,
    pub write_2: bool,
    pub write_3: bool,
    pub write_4: bool,
    pub write_4_late: bool,
    pub wave_area: bool,
    /// The ports: the DAC, the channel's play, its output.
    pub dac_on: bool,
    pub active: bool,
    pub output: u8,
    pub stop: bool,
    pub count_clock: bool,
    pub load_counter: bool,
    pub position_reset: bool,
    pub length_clock: bool,
    pub length_reset: bool,
    pub ram_addr: u8,
    pub ram_read: bool,
    pub ram_write: bool,
    pub precharge: bool,
    pub word_line: bool,
    pub sample: u8,
}

impl NetWave {
    /// A flip-flop's change is pending.
    #[must_use]
    pub const fn any_due(&self) -> bool {
        self.start.due
            | self.restart.due
            | self.restarted.due
            | self.running.due
            | self.active.due
            | self.counter.due
            | self.reload.due
            | self.position.due
            | self.wrapped.due
            | self.length.due
            | self.expired.due
            | self.busa.due
            | self.bano.due
            | self.fetch.due
            | self.azet.due
    }

    /// The next nanosecond.
    pub const fn advance(&mut self) {
        self.start.advance();
        self.restart.advance();
        self.restarted.advance();
        self.running.advance();
        self.active.advance();
        self.counter.advance();
        self.reload.advance();
        self.position.advance();
        self.wrapped.advance();
        self.length.advance();
        self.expired.advance();
        self.busa.advance();
        self.bano.advance();
        self.fetch.advance();
        self.azet.advance();
    }

    /// What a read of NR30, NR32, NR34 or wave RAM drives on the data bus:
    /// the DAC, the level, the length's enable, and wave RAM's bit lines,
    /// which while the channel plays hold the byte it reads, and are
    /// precharged (FF) otherwise. The bits nothing drives stay precharged,
    /// 1.
    // @hw die: FASY GEKO, HENU HAMU HUCO, GAWA HACA; DUGU..BEZU (not_if1)
    #[must_use]
    pub const fn read(&self, i: &ApuInputs) -> Option<u8> {
        if !i.cpu_rd {
            None
        } else if i.addr == 0xFF1A {
            Some((self.dac_on as u8) << 7 | 0x7F)
        } else if i.addr == 0xFF1C {
            Some(0x80 | self.level << 5 | 0x1F)
        } else if i.addr == 0xFF1E {
            Some(0x80 | (self.length_on as u8) << 6 | 0x3F)
        } else if self.wires.wave_area {
            Some(self.bit_lines)
        } else {
            None
        }
    }

    /// The wires, from the signals as they are.
    #[must_use]
    pub const fn next_wires(&self, i: &ApuInputs, cw: &ControlWires) -> WaveWires {
        let w = &self.wires;
        let active = self.active.q;
        let wave_area = i.addr >> 4 == 0xFF3;
        let sample = if self.position.q & 1 != 0 {
            self.sample_byte & 0x0F
        } else {
            self.sample_byte >> 4
        };
        let mask = if active { 0x0F } else { 0 };
        WaveWires {
            write_0: i.cpu_wr & (i.addr == 0xFF1A),
            write_1: i.cpu_wr & (i.addr == 0xFF1B),
            write_2: i.cpu_wr & (i.addr == 0xFF1C),
            write_3: i.cpu_wr & (i.addr == 0xFF1D),
            write_4: i.cpu_wr & (i.addr == 0xFF1E),
            write_4_late: w.write_4 & i.clk_t4,
            wave_area,
            dac_on: self.dac_on,
            active,
            output: match self.level & 3 {
                0 => 0,
                1 => w.sample & mask,
                2 => (w.sample >> 1) & mask,
                _ => (w.sample >> 2) & mask,
            },
            stop: cw.apu_reset | (self.expired.q & self.length_on) | !self.dac_on,
            count_clock: cw.ch3_2mhz | self.frozen,
            load_counter: self.reload.q | self.restart.q,
            position_reset: self.restart.q | self.wrapped.q,
            length_clock: !(self.length_on & !self.expired.q & !cw.hz256),
            length_reset: w.write_1 | cw.apu_reset | self.restart.q,
            ram_addr: if active {
                self.position.q >> 1 & 0x0F
            } else {
                i.addr.to_le_bytes()[0] & 0x0F
            },
            ram_read: if active {
                self.fetch.q
            } else {
                i.cpu_rd & w.wave_area
            },
            ram_write: i.cpu_wr & w.wave_area,
            precharge: if active {
                !(self.fetch.q | self.azet.q)
            } else {
                !(w.ram_write | w.ram_read | !i.pch_phase)
            },
            word_line: !w.precharge & (w.ram_read | (w.ram_write & !active)),
            sample,
        }
    }

    /// One delta cycle into `next`: the processes on the signals as they
    /// are (`self`), with their edges against the delta before (`prev`);
    /// `pi` are the inputs in the delta before.
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
        pi: &ApuInputs,
        cw: &ControlWires,
        pcw: &ControlWires,
        next: &mut Self,
    ) {
        let (w, pw) = (&self.wires, &prev.wires);
        next.wires = self.next_wires(i, cw);
        let reset = cw.apu_reset;

        // The registers.
        // @hw die: the drlatch_ee of NR30 to NR34
        if reset {
            next.dac_on = false;
            next.level = 0;
            next.frequency = 0;
            next.length_on = false;
        } else {
            if w.write_0 {
                next.dac_on = i.data & 0x80 != 0;
            }
            if w.write_2 {
                next.level = i.data >> 5 & 3;
            }
            if w.write_3 {
                next.frequency = next.frequency & 0x700 | u16::from(i.data);
            }
            if w.write_4 {
                next.frequency = next.frequency & 0xFF | u16::from(i.data & 7) << 8;
            }
            if w.write_4_late {
                next.length_on = i.data & 0x40 != 0;
            }
        }

        // The trigger: at `apu_phi`'s rise, then at 2 MiHz's fall (and the
        // run at its rise).
        // @hw die: FOBA (dffr); GOFY (nor_latch), GARA GYTA GYRA (dffr),
        // GUGU (nand_latch); FOZU (nor_latch), DAVO (dffr)
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
        let (clock_fell, clock_rose) = (
            fell(pcw.ch3_2mhz, cw.ch3_2mhz),
            rose(pcw.ch3_2mhz, cw.ch3_2mhz),
        );
        if reset {
            next.restarted.set(false);
            next.running.set(false);
        } else {
            if clock_fell {
                next.restarted.after_tco(self.restart.q);
            }
            if clock_rose {
                next.running.after_tco(self.restarted.q);
            }
        }
        if reset | self.restarted.q {
            next.restart.set(false);
        } else if clock_fell {
            next.restart.after_tco(self.triggered);
        } else {
            // The flip-flop holds.
        }
        if !self.dac_on | reset {
            next.frozen = true;
        } else if self.running.q {
            next.frozen = false;
        } else {
            // The latch holds.
        }
        if w.stop {
            next.playing = false;
        } else if self.restart.q {
            next.playing = true;
        } else {
            // The latch holds.
        }
        if reset {
            next.active.set(false);
        } else if rose(pcw.ajer, cw.ajer) {
            next.active.after_tco(self.playing);
        } else {
            // The flip-flop holds.
        }

        // The frequency counter, its overflow, the position and its wrap.
        // @hw die: the counter's (tffnl), HUNO (dffr); the position's
        // (tffnl), FETY (dffr)
        if w.load_counter {
            next.counter.set(self.frequency);
        } else if fell(pw.count_clock, w.count_clock) {
            next.counter.after_tco((self.counter.q + 1) & 0x7FF);
        } else {
            // The counter holds.
        }
        if reset | self.restart.q | (self.reload.q & cw.ch3_2mhz) {
            next.reload.set(false);
        } else if fell(prev.counter.q & 0x400 != 0, self.counter.q & 0x400 != 0) {
            next.reload.after_tco(true);
        } else {
            // The flip-flop holds.
        }
        if w.position_reset {
            next.position.set(0);
        } else if fell(prev.reload.q, self.reload.q) {
            next.position.after_tco((self.position.q + 1) & 0x1F);
        } else {
            // The counter holds.
        }
        if reset | self.restart.q | (self.wrapped.q & self.reload.q) {
            next.wrapped.set(false);
        } else if fell(prev.position.q & 0x10 != 0, self.position.q & 0x10 != 0) {
            next.wrapped.after_tco(!self.wrapped.q);
        } else {
            // The flip-flop holds.
        }

        // The length.
        // @hw die: the length's (tffnl), FEXU (dffr)
        if w.write_1 {
            next.length.set(i.data);
        } else if fell(pw.length_clock, w.length_clock) {
            next.length.after_tco(self.length.q.wrapping_add(1));
        } else {
            // The counter holds.
        }
        if w.length_reset {
            next.expired.set(false);
        } else if fell(prev.length.q & 0x80 != 0, self.length.q & 0x80 != 0) {
            next.expired.after_tco(!self.expired.q);
        } else {
            // The flip-flop holds.
        }

        // The RAM's read after a step, on the chip's clock.
        // @hw die: BUSA BANO AZUS AZET (dffr)
        if reset {
            next.busa.set(false);
            next.bano.set(false);
            next.fetch.set(false);
            next.azet.set(false);
        } else {
            if fell(pi.clk, i.clk) {
                next.busa.after_tco(self.reload.q);
                next.fetch.after_tco(self.bano.q);
            }
            if rose(pi.clk, i.clk) {
                next.bano.after_tco(self.busa.q);
                next.azet.after_tco(self.fetch.q);
            }
        }

        // The RAM's cells and bit lines, and the sample's latch.
        // @hw die: the cell wave_ram (dmg-sim's simplified SRAM)
        let cell = usize::from(w.ram_addr & 0x0F);
        if w.precharge {
            next.bit_lines = 0xFF;
        } else if w.ram_write {
            next.bit_lines = i.data;
            if w.word_line {
                next.ram[cell] = i.data;
            }
        } else if w.word_line {
            next.bit_lines = self.ram[cell];
        } else {
            // The bit lines keep their charge.
        }
        if reset {
            next.sample_byte = 0;
        } else if self.fetch.q {
            next.sample_byte = self.bit_lines;
        } else {
            // The latch holds.
        }
    }
}
