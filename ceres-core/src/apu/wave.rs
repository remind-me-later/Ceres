//! The wave channel (3).

use super::{
    Ctx, NR30_DAC_B, NRX4_PERIOD_HIGH, NRX4_TRIGGER_B, PERIOD_MASK, WAVE, length::Length,
    mixer::ChannelOutput, revision::Revision,
};

#[derive(Clone, Copy)]
pub(super) struct Wave {
    out: ChannelOutput,
    length: Length,
    /// NR30 bit 7: the DAC is on.
    dac_enabled: bool,
    /// NR32: the output level (bits 5-6).
    nr32: u8,
    /// The 11-bit period from NR33 and NR34.
    period: u16,
    /// 2 MHz ticks until the next sample is read.
    countdown: u16,
    /// Position in the wave RAM, in nibbles (0-31).
    position: u8,
    /// The last byte read from the wave RAM.
    sample_byte: u8,
    /// The wave RAM was read on the last tick (only then can the CPU access
    /// it on the DMG while the channel plays).
    just_read: bool,
    /// The channel was triggered since the DAC was turned on.
    pulsed: bool,
    /// The DMG keeps reading the wave RAM with the channel stopped: it reads
    /// the byte on the address bus when this expires.
    bugged_read_countdown: u8,
    ram: [u8; 0x10],
}

impl Wave {
    pub(super) const fn new() -> Self {
        Self {
            out: ChannelOutput::new(WAVE),
            length: Length::new(0x100),
            dac_enabled: false,
            nr32: 0,
            period: 0,
            countdown: 0,
            position: 0,
            sample_byte: 0,
            just_read: false,
            pulsed: false,
            bugged_read_countdown: 0,
            ram: [0; 0x10],
        }
    }

    /// Clears the channel, but the DAC keeps its level until the next update
    /// and the wave RAM is unchanged.
    pub(super) const fn power_off(&mut self) {
        let mut out = self.out;
        out.power_off();
        *self = Self {
            out,
            ram: self.ram,
            ..Self::new()
        };
    }

    pub(super) const fn out(&self) -> &ChannelOutput {
        &self.out
    }

    pub(super) const fn out_mut(&mut self) -> &mut ChannelOutput {
        &mut self.out
    }

    pub(super) const fn length_counter(&self) -> u16 {
        self.length.counter
    }

    pub(super) const fn set_length_counter(&mut self, counter: u16) {
        self.length.counter = counter;
    }

    pub(super) const fn dac_enabled(&self) -> bool {
        self.dac_enabled
    }

    /// Resets the channel but the wave RAM.
    pub(super) const fn reset(&mut self) {
        *self = Self {
            ram: self.ram,
            ..Self::new()
        };
    }

    /// Every other DIV event.
    pub(super) fn tick_length(&mut self, c: &Ctx) {
        if self.length.tick() {
            self.expire(c);
        }
    }

    pub(super) const fn read_nr30(&self) -> u8 {
        if self.dac_enabled { 0xFF } else { 0x7F }
    }

    pub(super) const fn read_nr32(&self) -> u8 {
        self.nr32 | 0x9F
    }

    pub(super) const fn read_nr34(&self) -> u8 {
        if self.length.enabled { 0xFF } else { 0xBF }
    }

    /// The right shift of the 4-bit samples for the output level in NR32
    /// bits 5-6: mute (a shift by 4 clears them), 100%, 50% and 25%.
    const fn shift(&self) -> u8 {
        [4, 0, 1, 2][((self.nr32 >> 5) & 3) as usize]
    }

    pub(super) fn update_sample(&mut self, value: u8, c: &Ctx) {
        self.out.update(value, self.dac_enabled, 0, c);
    }

    fn update_wave_sample(&mut self, c: &Ctx) {
        let nibble = if self.position & 1 != 0 {
            self.sample_byte & 0xF
        } else {
            self.sample_byte >> 4
        };
        self.update_sample(nibble >> self.shift(), c);
    }

    pub(super) fn disable(&mut self, c: &Ctx) {
        self.out.active = false;
        self.update_sample(0, c);
    }

    /// The wave RAM byte the CPU accesses at `offset`: while the channel
    /// plays, the one it is reading, if any.
    fn ram_index(&self, offset: usize, rev: Revision) -> Option<usize> {
        if !self.out.active {
            return Some(offset);
        }
        if (!rev.is_cgb() && !self.just_read) || rev.is_agb() {
            return None;
        }
        Some(usize::from(self.position / 2))
    }

    pub(super) fn read_ram(&self, offset: usize, rev: Revision) -> u8 {
        self.ram_index(offset, rev).map_or(0xFF, |i| self.ram[i])
    }

    pub(super) fn write_ram(&mut self, offset: usize, value: u8, rev: Revision) {
        if let Some(i) = self.ram_index(offset, rev) {
            self.ram[i] = value;
        }
    }

    /// A stopped DMG wave channel has a read pending.
    pub(super) const fn has_bugged_read(&self) -> bool {
        self.bugged_read_countdown != 0
    }

    /// The pending read of a stopped DMG wave channel.
    pub(super) fn run_bugged_read(&mut self, cycles: u32, c: &Ctx) {
        for _ in 0..cycles {
            self.bugged_read_countdown = self.bugged_read_countdown.wrapping_sub(1);
            if self.bugged_read_countdown == 0 {
                self.sample_byte = self.ram[usize::from(c.address_bus & 0xF)];
                if self.out.active {
                    self.update_wave_sample(c);
                }
                break;
            }
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "cycles_left is below the countdown"
    )]
    pub(super) fn run(&mut self, cycles: u32, c: &Ctx) {
        self.just_read = false;
        if self.out.active {
            let mut cycles_left = cycles;
            while cycles_left > u32::from(self.countdown) {
                cycles_left -= u32::from(self.countdown) + 1;
                self.countdown = self.period ^ PERIOD_MASK;
                self.position = (self.position + 1) & 0x1F;
                self.sample_byte = self.ram[usize::from(self.position >> 1)];
                self.update_wave_sample(c);
                self.just_read = true;
            }
            if cycles_left != 0 {
                self.countdown -= cycles_left as u16;
                self.just_read = false;
            }
        } else if self.dac_enabled && self.pulsed && c.rev <= Revision::CgbE {
            let mut cycles_left = cycles;
            while cycles_left > u32::from(self.countdown) {
                cycles_left -= u32::from(self.countdown) + 1;
                self.countdown = self.period ^ PERIOD_MASK;
                if cycles_left != 0 {
                    self.sample_byte = self.ram[usize::from(c.address_bus & 0xF)];
                } else {
                    self.bugged_read_countdown = 1;
                }
            }
            if cycles_left != 0 {
                self.countdown -= cycles_left as u16;
            }
            if self.countdown == 0 {
                self.bugged_read_countdown = 2;
            }
        }
    }

    /// The length timer expired.
    fn expire(&mut self, c: &Ctx) {
        if self.out.active && c.rev.is_agb() {
            if self.countdown == 0 {
                self.sample_byte = self.ram[usize::from(((self.position + 1) & 0xF) >> 1)];
            } else if self.countdown == 9 {
                self.sample_byte = self.ram[0];
            }
        }
        self.disable(c);
    }

    pub(super) fn write_nr30(&mut self, value: u8, c: &Ctx) {
        self.dac_enabled = value & NR30_DAC_B != 0;
        if !self.dac_enabled {
            self.pulsed = false;
            if self.out.active {
                // Assumed to also happen on pre-CGB models.
                if self.countdown == 0 && c.rev <= Revision::CgbE {
                    self.sample_byte = self.ram[usize::from(c.pc & 0xF)];
                } else if self.just_read && c.rev <= Revision::CgbC {
                    // The low nibble of NR30's address.
                    self.sample_byte = self.ram[0xA];
                }
            }
            self.disable(c);
        }
    }

    pub(super) const fn write_nr31(&mut self, value: u8) {
        self.length.load(value as u16);
    }

    pub(super) fn write_nr32(&mut self, value: u8, c: &Ctx) {
        self.nr32 = value;
        if self.out.active {
            self.update_wave_sample(c);
        }
    }

    pub(super) const fn write_nr33(&mut self, value: u8) {
        self.period = (self.period & !0xFF) | value as u16;
        if self.bugged_read_countdown == 1 {
            // Just reloaded the countdown.
            self.countdown = self.period ^ PERIOD_MASK;
        }
    }

    pub(super) fn write_nr34(&mut self, value: u8, c: &Ctx) {
        self.period = (self.period & 0xFF) | (u16::from(value & NRX4_PERIOD_HIGH) << 8);
        if value & NRX4_TRIGGER_B != 0 {
            self.trigger(c);
        }
        if self.length.write(
            value,
            c.rev.is_cgb() && c.rev <= Revision::CgbB,
            c.div_divider,
        ) {
            self.disable(c);
        }
    }

    fn trigger(&mut self, c: &Ctx) {
        self.pulsed = true;
        // DMG bug: wave RAM gets corrupted if the channel is retriggered 1
        // cycle before the APU reads from it.
        if !c.rev.is_cgb() && self.out.active && self.countdown == 0 {
            let offset = usize::from(((self.position + 1) >> 1) & 0xF);
            // The most common DMG-B behaviour (what blargg's tests expect);
            // the MGB emulates a deterministic Game Boy Light.
            if offset < 4 && c.rev != Revision::Mgb {
                self.ram[0] = self.ram[offset];
            } else {
                let base = offset & !3;
                self.ram.copy_within(base..base + 4, 0);
            }
        }
        self.position = 0;
        if self.out.active && self.countdown == 0 {
            self.sample_byte = self.ram[0];
        }
        if self.dac_enabled {
            self.out.active = true;
            self.update_sample((self.sample_byte >> 4) >> self.shift(), c);
        }
        self.countdown = (self.period ^ PERIOD_MASK) + 3;
        self.length.trigger();
        // The sample is not changed just yet (verified on hardware).
    }
}
