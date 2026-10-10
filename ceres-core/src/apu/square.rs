//! The square channels (1 and 2).

use super::{
    Ctx, NRX1_LENGTH, NRX2_DAC, NRX4_PERIOD_HIGH, NRX4_TRIGGER_B, PERIOD_MASK, envelope::Envelope,
    length::Length, mixer::ChannelOutput, revision::Revision,
};

/// The waveforms selected by NRx1 bits 6-7, high 12.5%, 25%, 50% and 75%
/// of the time.
const DUTIES: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

#[derive(Clone, Copy)]
pub(super) struct Square {
    /// 0 for channel 1, 1 for channel 2.
    index: usize,
    out: ChannelOutput,
    length: Length,
    envelope: Envelope,
    /// NRx1 bits 6-7.
    duty: u8,
    /// The last value written to NRx4.
    nrx4: u8,
    /// The 11-bit period from NRx3 and NRx4 (channel 1's sweep changes it).
    period: u16,
    /// 2 MHz ticks until the next duty step.
    countdown: u16,
    /// Position in the duty cycle (0-7).
    duty_step: u8,
    /// A just triggered channel outputs nothing until its first duty step.
    suppressed: bool,
    /// The start delay after a trigger.
    delay: u8,
    did_tick: bool,
    /// The countdown reloaded on the last tick.
    just_reloaded: bool,
}

impl Square {
    pub(super) const fn new(index: usize) -> Self {
        Self {
            index,
            out: ChannelOutput::new(),
            length: Length::new(),
            envelope: Envelope::new(),
            duty: 0,
            nrx4: 0,
            period: 0,
            countdown: 0,
            duty_step: 0,
            suppressed: false,
            delay: 0,
            did_tick: false,
            just_reloaded: false,
        }
    }

    /// Clears the channel, but the DAC keeps its level until the next update.
    pub(super) const fn power_off(&mut self) {
        let mut out = self.out;
        out.power_off();
        *self = Self {
            out,
            ..Self::new(self.index)
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
        self.envelope.dac_enabled()
    }

    pub(super) const fn read_nrx2(&self) -> u8 {
        self.envelope.nrx2
    }

    pub(super) const fn set_envelope_countdown(&mut self, countdown: u8) {
        self.envelope.countdown = countdown;
    }

    /// Every 8th DIV event.
    pub(super) const fn step_envelope_countdown(&mut self) {
        self.envelope.step_countdown();
    }

    /// The secondary DIV event.
    pub(super) const fn reload_envelope(&mut self) {
        if self.out.active {
            self.envelope.reload();
        }
    }

    /// Every other DIV event.
    pub(super) fn tick_length(&mut self, c: &Ctx) {
        if self.length.tick() {
            self.disable(c);
        }
    }

    pub(super) const fn period(&self) -> u16 {
        self.period
    }

    pub(super) const fn set_period(&mut self, period: u16) {
        self.period = period;
    }

    /// Powering the APU on leaves the countdown at its maximum.
    pub(super) const fn power_on(&mut self) {
        self.countdown = 0xFFFF;
    }

    /// The state the boot ROM leaves on channel 1: `played` when it played
    /// the start-up sound (see `PostBoot`).
    pub(super) const fn post_boot(
        &mut self,
        played: bool,
        countdown: u16,
        length: u16,
        volume_countdown: u8,
        duty_step: u8,
    ) {
        self.write_nrx1(0x80);
        self.envelope.nrx2 = 0xF3;
        self.write_nrx3(0xC1);
        self.nrx4 = if played { 0x87 } else { 0x07 };
        self.period = 0x7C1;
        self.countdown = countdown;
        self.length.counter = length;
        self.envelope.countdown = volume_countdown;
        self.duty_step = duty_step;
        if played {
            self.out.active = true;
            self.did_tick = true;
            self.envelope.clock.locked = true;
            self.envelope.clock.should_lock = true;
        }
    }

    pub(super) const fn read_nrx1(&self) -> u8 {
        (self.duty << 6) | NRX1_LENGTH
    }

    pub(super) const fn read_nrx4(&self) -> u8 {
        self.nrx4 | 0xBF
    }

    pub(super) fn update_sample(&mut self, value: u8, c: &Ctx) {
        self.out.update(
            self.index,
            value,
            self.envelope.dac_enabled(),
            self.envelope.volume,
            c,
        );
    }

    /// Outputs the current step of the duty cycle.
    pub(super) fn update_duty_sample(&mut self, c: &Ctx) {
        if self.suppressed {
            if c.rev.is_agb() {
                self.update_sample(self.out.sample, c);
            }
            return;
        }
        let on = DUTIES[usize::from(self.duty)][usize::from(self.duty_step)] != 0;
        self.update_sample(if on { self.envelope.volume } else { 0 }, c);
    }

    pub(super) fn disable(&mut self, c: &Ctx) {
        self.out.active = false;
        self.update_sample(0, c);
    }

    pub(super) fn run(&mut self, cycles: u32, c: &Ctx) {
        if !self.out.active {
            return;
        }
        let mut cycles_left = cycles;
        self.delay = u32::from(self.delay).saturating_sub(cycles_left) as u8;
        while cycles_left > u32::from(self.countdown) {
            cycles_left -= u32::from(self.countdown) + 1;
            self.countdown = (self.period ^ PERIOD_MASK) * 2 + 1;
            self.duty_step = (self.duty_step + 1) & 7;
            self.suppressed = false;
            if cycles_left == 0 && self.out.sample == 0 {
                self.out.pcm_mask = 0;
            }
            self.did_tick = true;
            self.update_duty_sample(c);
        }
        self.just_reloaded = cycles_left == 0;
        if cycles_left != 0 {
            self.countdown -= cycles_left as u16;
        }
    }

    /// Steps the volume if the envelope clock is high.
    pub(super) fn tick_envelope(&mut self, c: &Ctx) {
        if !self.envelope.clock.clock {
            return;
        }
        let Some(old_volume) = self.envelope.tick() else {
            return;
        };
        if c.double_speed {
            // The PCM register misses the bits the step changes.
            // CGB-0 behaviour is instance specific and non-deterministic.
            let cgb0_glitch =
                c.rev == Revision::Cgb0 && old_volume == 1 && self.envelope.nrx2 & 8 != 0;
            let bits = if self.index == 0 || cgb0_glitch { 1 } else { 3 };
            self.out.pcm_mask &= (old_volume | bits) & 0xF;
        }
        if self.out.active {
            self.update_duty_sample(c);
        }
    }

    /// `value` has bits 0-5 masked off when written while the APU is off.
    pub(super) const fn write_nrx1(&mut self, value: u8) {
        self.length.counter = 0x40 - (value & NRX1_LENGTH) as u16;
        self.duty = value >> 6;
    }

    pub(super) fn write_nrx2(&mut self, value: u8, c: &Ctx) {
        if value & NRX2_DAC == 0 {
            // This disables the DAC.
            self.envelope.nrx2 = value;
            self.disable(c);
        } else if self.out.active {
            self.envelope.write_while_active(c.rev, value);
            self.update_duty_sample(c);
        } else {
            self.envelope.nrx2 = value;
        }
    }

    pub(super) const fn write_nrx3(&mut self, value: u8) {
        self.period = (self.period & !0xFF) | value as u16;
        if self.just_reloaded {
            self.countdown = (self.period ^ PERIOD_MASK) * 2 + 1;
        }
    }

    pub(super) fn write_nrx4(&mut self, value: u8, c: &Ctx) {
        // When the period changes right before being updated from >=$700 to
        // <$700 the countdown should change to the old period but the current
        // sample should not change; step the index backwards instead.
        if value & NRX4_TRIGGER_B == 0
            && self.out.active
            && self.nrx4 & NRX4_PERIOD_HIGH == NRX4_PERIOD_HIGH
            && value & NRX4_PERIOD_HIGH != NRX4_PERIOD_HIGH
            && (c.rev.is_cgb_de() || self.countdown & 1 != 0)
            && self.did_tick
            && self.countdown >> 1 == (self.period ^ PERIOD_MASK)
        {
            self.duty_step = self.duty_step.wrapping_sub(1) & 7;
            self.suppressed = false;
        }

        let old_period = self.period;
        self.period = (self.period & 0xFF) | (u16::from(value & NRX4_PERIOD_HIGH) << 8);
        if self.just_reloaded {
            self.countdown = (self.period ^ PERIOD_MASK) * 2 + 1;
        }
        if value & NRX4_TRIGGER_B != 0 {
            self.trigger(value, old_period, c);
        }

        if self.length.write(
            value,
            c.rev.is_cgb() && c.rev <= Revision::CgbB,
            c.div_divider,
            0x40,
        ) {
            self.disable(c);
        }
        self.nrx4 = value;
    }

    fn trigger(&mut self, value: u8, old_period: u16, c: &Ctx) {
        // The duty step is unchanged when restarting the channel; only turning
        // the APU off resets it.
        self.envelope.unlock();
        self.did_tick = false;
        let mut force_unsuppressed = false;
        if self.out.active {
            let mut extra_delay = 0_u8;
            if c.rev.is_cgb_de() {
                if !self.just_reloaded
                    && value & 4 == 0
                    && (self
                        .countdown
                        .wrapping_sub(1)
                        .wrapping_sub(u16::from(self.delay))
                        / 2)
                        & 0x400
                        == 0
                {
                    self.duty_step = (self.duty_step + 1) & 7;
                    self.suppressed = false;
                } else if self.period == PERIOD_MASK && old_period != PERIOD_MASK && self.suppressed
                {
                    extra_delay += 2;
                }
            }
            // Timing quirk: if already active, the sound starts 2 (2 MHz)
            // ticks earlier.
            self.delay = 4_u8.wrapping_sub(c.lf_div).wrapping_add(extra_delay);
        } else {
            if c.rev.is_cgb_de()
                && value & 4 == 0
                && (self.countdown.wrapping_sub(u16::from(self.delay)) / 2) & 0x400 == 0
            {
                self.duty_step = (self.duty_step + 1) & 7;
                force_unsuppressed = true;
            }
            let lf_div = i32::from(c.lf_div);
            let delay = 6 + lf_div
                * if c.rev <= Revision::CgbC && c.double_speed {
                    1
                } else {
                    -1
                };
            self.delay = delay as u8;
        }
        self.countdown = (self.period ^ PERIOD_MASK) * 2 + u16::from(self.delay);

        self.envelope.restart();
        // The volume change caused by sound start takes effect instantly
        // (i.e. on the previously started sound).
        if self.out.active {
            self.update_duty_sample(c);
        }

        if self.envelope.dac_enabled() && !self.out.active {
            self.out.active = true;
            self.update_sample(0, c);
            self.suppressed = !force_unsuppressed;
        }
        self.length.trigger(0x40);
    }
}
