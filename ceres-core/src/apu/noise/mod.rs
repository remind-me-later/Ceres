//! The noise channel (4).

mod nr43;

use super::{
    Ctx, NOISE, envelope::Envelope, length::Length, mixer::ChannelOutput, revision::Revision,
};

#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent hardware flags of the channel state machine"
)]
#[derive(Clone, Copy)]
pub struct Noise {
    out: ChannelOutput,
    length: Length,
    envelope: Envelope,
    /// NR43: clock shift (bits 4-7), LFSR width (bit 3) and divider (0-2).
    nr43: u8,
    lfsr: u16,
    /// 7-bit LFSR (NR43 bit 3).
    narrow: bool,
    /// The bit of the LFSR that the channel outputs.
    lfsr_sample: bool,
    /// The LFSR steps on a rising edge of a bit (selected by the clock shift)
    /// of this counter, which the divider clocks.
    counter: u16,
    counter_countdown: u8,
    /// The counter runs while the DAC is on...
    counter_active: bool,
    /// ...and keeps running after the channel stops until the DAC is turned
    /// off.
    background_counter_active: bool,
    did_step_counter: bool,
    countdown_reloaded: bool,
    /// Counts 2 MHz ticks: the phase of the divider.
    alignment: u8,
    /// A DMG trigger out of phase with the divider starts after a delay.
    dmg_delayed_start: u8,
    started_with_dac_disabled: bool,
    stepped_in_narrow: bool,
    bit_7_before_step: bool,
}

impl Noise {
    pub const fn new() -> Self {
        Self {
            out: ChannelOutput::new(),
            length: Length::new(),
            envelope: Envelope::new(),
            nr43: 0,
            lfsr: 0,
            narrow: false,
            lfsr_sample: false,
            counter: 0,
            counter_countdown: 0,
            counter_active: false,
            background_counter_active: false,
            did_step_counter: false,
            countdown_reloaded: false,
            alignment: 0,
            dmg_delayed_start: 0,
            started_with_dac_disabled: false,
            stepped_in_narrow: false,
            bit_7_before_step: false,
        }
    }

    /// Clears the channel, but the DAC keeps its level until the next update.
    pub const fn power_off(&mut self) {
        let mut out = self.out;
        out.power_off();
        *self = Self { out, ..Self::new() };
    }

    pub const fn out(&self) -> &ChannelOutput {
        &self.out
    }

    pub const fn out_mut(&mut self) -> &mut ChannelOutput {
        &mut self.out
    }

    pub const fn length_counter(&self) -> u16 {
        self.length.counter
    }

    pub const fn set_length_counter(&mut self, counter: u16) {
        self.length.counter = counter;
    }

    pub const fn dac_enabled(&self) -> bool {
        self.envelope.dac_enabled()
    }

    pub const fn read_nr42(&self) -> u8 {
        self.envelope.nrx2
    }

    pub const fn set_envelope_countdown(&mut self, countdown: u8) {
        self.envelope.countdown = countdown;
    }

    /// Every 8th DIV event.
    pub const fn step_envelope_countdown(&mut self) {
        self.envelope.step_countdown();
    }

    /// The secondary DIV event.
    pub const fn reload_envelope(&mut self) {
        if self.out.active {
            self.envelope.reload();
        }
    }

    /// Every other DIV event.
    pub fn tick_length(&mut self, c: &Ctx) {
        if self.length.tick() {
            self.disable(c);
        }
    }

    pub const fn length_enabled(&self) -> bool {
        self.length.enabled
    }

    /// The phase of the divider advances by `cycles` 2 MHz ticks.
    pub const fn advance_alignment(&mut self, cycles: u32) {
        self.alignment = self.alignment.wrapping_add(cycles as u8);
    }

    pub const fn set_alignment(&mut self, alignment: u8) {
        self.alignment = alignment;
    }

    pub const fn read_nr43(&self) -> u8 {
        self.nr43
    }

    pub const fn read_nr44(&self) -> u8 {
        if self.length.enabled { 0xFF } else { 0xBF }
    }

    pub fn update_sample(&mut self, value: u8, c: &Ctx) {
        self.out.update(
            NOISE,
            value,
            self.envelope.dac_enabled(),
            self.envelope.volume,
            c,
        );
    }

    pub fn disable(&mut self, c: &Ctx) {
        self.out.active = false;
        self.update_sample(0, c);
    }

    fn update_lfsr(&mut self, c: &Ctx) {
        self.lfsr_sample = self.lfsr & 1 != 0;
        if self.out.active {
            self.update_sample(self.lfsr_output(), c);
        }
    }

    /// The sample for the current LFSR output.
    const fn lfsr_output(&self) -> u8 {
        if self.lfsr_sample {
            self.envelope.volume
        } else {
            0
        }
    }

    const fn high_bit_mask(&self) -> u16 {
        if self.narrow { 0x4040 } else { 0x4000 }
    }

    fn step_lfsr(&mut self, c: &Ctx) {
        self.bit_7_before_step = self.lfsr & 0x80 != 0;
        let high_bit_mask = self.high_bit_mask();
        let new_high_bit = (self.lfsr ^ (self.lfsr >> 1) ^ 1) & 1 != 0;
        self.lfsr >>= 1;
        if new_high_bit {
            self.lfsr |= high_bit_mask;
        } else {
            // Relevant when switching LFSR widths.
            self.lfsr &= !high_bit_mask;
        }
        self.update_lfsr(c);
        self.stepped_in_narrow = self.narrow;
    }

    /// The 2 MHz ticks per counter step that NR43's divider selects.
    const fn divisor(nr43: u8) -> u8 {
        match (nr43 & 7) << 2 {
            0 => 2,
            d => d,
        }
    }

    /// The counter bit whose rising edge steps the LFSR.
    const fn counter_bit(counter: u16, nr43: u8) -> bool {
        (counter >> (nr43 >> 4)) & 1 != 0
    }

    /// Steps the volume if the envelope clock is high.
    pub fn tick_envelope(&mut self, c: &Ctx) {
        if !self.envelope.clock.clock {
            return;
        }
        let Some(old_volume) = self.envelope.tick() else {
            return;
        };
        if c.double_speed {
            // The PCM register misses the bits the step changes.
            self.out.pcm_mask &= (old_volume | 1) & 0xF;
        }
        if self.out.active {
            let value = if self.lfsr & 1 != 0 {
                self.envelope.volume
            } else {
                0
            };
            self.update_sample(value, c);
        }
    }

    /// Advances the delayed DMG start. Returns the ticks to run before the
    /// channel starts, if it starts within `cycles`.
    pub const fn delayed_start(&mut self, cycles: u32) -> Option<u32> {
        let delayed = self.dmg_delayed_start as u32;
        if delayed == 0 {
            return None;
        }
        if delayed > cycles {
            self.dmg_delayed_start -= cycles as u8;
            return None;
        }
        if delayed == cycles {
            self.dmg_delayed_start = 0;
        }
        Some(delayed)
    }

    pub fn run(&mut self, cycles: u32, c: &Ctx) {
        if !self.counter_active && !self.background_counter_active {
            return;
        }
        let mut cycles_left = cycles;
        let divisor = Self::divisor(self.nr43);
        if self.counter_countdown == 0 {
            self.counter_countdown = divisor;
        }
        while cycles_left >= u32::from(self.counter_countdown) {
            cycles_left -= u32::from(self.counter_countdown);
            self.counter_countdown = divisor;
            let old_bit = Self::counter_bit(self.counter, self.nr43);
            self.counter = (self.counter + 1) & 0x3FFF;
            self.did_step_counter = true;
            let new_bit = Self::counter_bit(self.counter, self.nr43);

            if new_bit && !old_bit && self.out.active {
                if cycles_left == 0 && self.out.sample == 0 && !c.double_speed {
                    self.out.pcm_mask = 0;
                }
                self.step_lfsr(c);
            }
        }
        self.countdown_reloaded = cycles_left == 0;
        if cycles_left != 0 {
            self.counter_countdown -= cycles_left as u8;
        }
    }

    pub const fn write_nr41(&mut self, value: u8) {
        self.length.counter = 0x40 - (value & 0x3F) as u16;
    }

    pub fn write_nr42(&mut self, value: u8, c: &Ctx) {
        if value & 0xF8 == 0 {
            // This disables the DAC.
            if self.out.active && self.nr43 & 7 != 0 {
                if self.counter_countdown <= 2 {
                    self.counter += 1;
                }
                self.background_counter_active = false;
            }
            self.envelope.nrx2 = value;
            self.disable(c);
            self.counter_active = false;
        } else if self.out.active {
            self.envelope.write_while_active(c.rev, value);
            self.update_sample(self.lfsr_output(), c);
        } else {
            self.envelope.nrx2 = value;
        }
    }

    pub fn write_nr43(&mut self, value: u8, c: &Ctx) {
        if self.countdown_reloaded {
            let divisor = Self::divisor(value);
            let align = usize::from(self.alignment & 3);
            let table: [u8; 4] = if c.rev >= Revision::CgbD {
                [2, 1, 0, 3]
            } else {
                [2, 1, 4, 3]
            };
            self.counter_countdown = divisor + if divisor == 2 { 0 } else { table[align] };
        }
        if c.rev <= Revision::CgbC {
            // CGB <= C (and DMG) have various unemulated quirks when NR43 is
            // written just as the counter reloads.
            if self.countdown_reloaded {
                let counter = self.counter;
                let old_bit = Self::counter_bit(counter, self.nr43);
                let glitch_bit = counter & 0x80 != 0;
                let new_bit = Self::counter_bit(counter, value);
                if !old_bit && new_bit && glitch_bit {
                    let previous = counter.wrapping_sub(1) & 0x3FFF;
                    let previous_old_bit = Self::counter_bit(previous, self.nr43);
                    let previous_glitch_bit = previous & 0x80 != 0;
                    let previous_new_bit = Self::counter_bit(previous, value);
                    if previous_old_bit && !previous_new_bit && previous_glitch_bit {
                        self.step_lfsr(c);
                    }
                }
            }
            self.switch_clock(0xFF, c);
        }
        self.switch_clock(value, c);
    }

    pub fn write_nr44(&mut self, value: u8, c: &Ctx) {
        if value & 0x80 != 0 {
            self.envelope.unlock();
            if !c.rev.is_cgb() && self.alignment & 3 != 0 {
                self.dmg_delayed_start = 6;
            } else {
                self.start(c);
            }
        }
        if self.length.write(value, false, c.div_divider, 0x40) {
            self.disable(c);
        }
    }

    fn start(&mut self, c: &Ctx) {
        self.lfsr = 0;
        self.prepare_start(c);

        self.envelope.restart();
        self.lfsr_sample = false;
        self.did_step_counter = self.alignment & 3 == 2;

        if self.envelope.dac_enabled() {
            self.out.active = true;
            self.update_sample(0, c);
        }
        self.length.trigger(0x40);
    }

    /// Restarts the counter and the LFSR, with the many timing quirks of the
    /// divider phase.
    fn prepare_start(&mut self, c: &Ctx) {
        let old_revision = c.rev <= Revision::CgbC;
        let ds = c.double_speed;
        self.counter_active = self.envelope.dac_enabled();
        let was_started_with_dac_disabled = self.started_with_dac_disabled;
        self.started_with_dac_disabled = !self.counter_active;
        let mut divisor = i32::from(self.nr43 & 0x07);
        let was_background_counting = self.background_counter_active;
        self.background_counter_active = true;
        let mut instant_step = false;
        let mut div_1_glitch = false;
        let active = self.out.active;

        if divisor > 1
            && (self.counter_countdown == 1
                || self.counter_countdown == 2 && active && old_revision && ds)
        {
            self.counter = (self.counter + 1) & 0x3FFF;
        } else if self.counter_countdown == 2 && self.alignment & 3 == 0 && active {
            if divisor == 0 {
                divisor = 8;
            } else if divisor == 1 {
                if !self.did_step_counter {
                    div_1_glitch = true;
                }
                let old_bit = Self::counter_bit(self.counter, self.nr43);
                self.counter = (self.counter + 1) & 0x3FFF;
                let new_bit = Self::counter_bit(self.counter, self.nr43);
                if new_bit && !old_bit {
                    instant_step = true;
                }
            }
        }
        let mut countdown: i32 = if divisor == 0 { 6 } else { divisor * 4 + 6 };
        let alignment = self.alignment;
        if alignment & 1 != 0 {
            if divisor == 0 {
                if old_revision || !was_background_counting {
                    countdown += 1;
                } else {
                    countdown -= 1;
                }
            } else if alignment & 2 != 0 {
                if divisor == 1 && !active {
                    countdown += 1;
                } else {
                    countdown -= 3;
                }
            } else {
                countdown -= 1;
                if divisor == 1 && active {
                    countdown -= 4;
                }
            }
        } else if divisor != 0 {
            if alignment & 2 != 0 {
                if ds && old_revision && divisor == 1 {
                    countdown += 2;
                } else {
                    countdown -= 2;
                }
            } else if divisor > 1 && (!ds || !old_revision) {
                countdown -= 4;
            } else if divisor == 1 && active && self.nr43 & 0xF0 == 0 {
                // This quirk seems way too specific.
                countdown -= 4;
            }
        } else if ds && old_revision {
            countdown += 2;
        }

        // Background counting glitches (double speed is not tested).
        if divisor > 1 {
            if !self.counter_active && alignment & 3 == 0 {
                countdown += 4;
            }
        } else if was_background_counting && !active && alignment & 3 == 0 {
            if divisor == 0 {
                if was_started_with_dac_disabled {
                    countdown += 28;
                }
            } else {
                countdown -= 4;
            }
        }

        if divisor == 0 && old_revision && was_background_counting && !active && ds {
            countdown -= 1;
        }
        if div_1_glitch {
            countdown -= 4;
        }
        self.counter_countdown = countdown as u8;

        if divisor == 0 && active && alignment & 3 == 3 {
            // Seemingly arbitrary, but confirmed for this edge case.
            self.lfsr = 0x0055;
        } else {
            self.lfsr = 0;
        }
        if instant_step {
            self.step_lfsr(c);
        }
    }
}
