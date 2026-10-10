//! The channel outputs and the mixer that turns them into output samples.

use super::{Ctx, N_CHANNELS, WAVE, high_pass_filter::HighPassFilter};
use crate::{AudioCallback, timing::DOTS_PER_SEC};

/// Output amplitude of one DAC step (SameBoy's `CH_STEP`): a channel's
/// largest amplitude, 0xFF0, over its 15 steps and the 8 master volumes.
const CH_STEP: i32 = 0xFF0 / 0xF / 8;

/// A channel's connection to its DAC: whether it is playing, the digital
/// sample it outputs and the resulting level after NR50/NR51.
#[derive(Clone, Copy)]
pub(super) struct ChannelOutput {
    /// The channel (`SQUARE_1` to `NOISE`): its NR51 bits.
    pub index: usize,
    /// The channel is on (NR52 bits 0-3).
    pub active: bool,
    /// The 4-bit sample read back through PCM12/PCM34. Set to 0x10 when it
    /// has to be refreshed, which leaves a DC offset on a channel whose DAC
    /// is off.
    pub sample: u8,
    /// Output level (left, right).
    pub level: (i32, i32),
    /// Bits of the sample that a PCM register read sees this M-cycle (the
    /// CGB-C and older miss some while the channel updates).
    pub pcm_mask: u8,
}

impl ChannelOutput {
    pub(super) const fn new(index: usize) -> Self {
        Self {
            index,
            active: false,
            sample: 0,
            level: (0, 0),
            pcm_mask: 0xF,
        }
    }

    /// What powering the APU off clears: the level of the DAC stays until the
    /// next update.
    pub(super) const fn power_off(&mut self) {
        *self = Self {
            level: self.level,
            ..Self::new(self.index)
        };
    }

    /// The sample a PCM register read sees.
    pub(super) const fn pcm(&self, masked: bool) -> u8 {
        if !self.active {
            return 0;
        }
        if masked {
            self.sample & self.pcm_mask
        } else {
            self.sample
        }
    }

    /// Sets the sample to `value` (SameBoy's `update_sample`). `volume` is
    /// the envelope volume, which biases the output on the AGB.
    pub(super) fn update(&mut self, value: u8, dac_enabled: bool, volume: u8, c: &Ctx) {
        let index = self.index;
        let left = c.nr51 & (0x10 << index) != 0;
        let right = c.nr51 & (1 << index) != 0;
        let left_volume = i32::from((c.nr50 >> 4) & 7) + 1;
        let right_volume = i32::from(c.nr50 & 7) + 1;

        if c.rev.is_agb() {
            // A channel that is not connected is identical to a connected
            // channel playing PCM sample 0.
            self.sample = value;
            // Channel 3 is inverted on the AGB and has another "silence".
            let (value, silence) = if index == WAVE {
                (i32::from(value) ^ 0xF, 7 * 2)
            } else {
                (i32::from(value), 0)
            };
            let bias = if self.active { i32::from(volume) } else { 0 };
            self.level = (
                (0xF - if left { value * 2 + bias } else { silence }) * left_volume,
                (0xF - if right { value * 2 + bias } else { silence }) * right_volume,
            );
            return;
        }

        if value == 0 && self.sample == 0 {
            return;
        }
        let value = if dac_enabled {
            self.sample = value;
            value
        } else {
            self.sample
        };
        let v = 0xF - i32::from(value) * 2;
        self.level = (
            if left { v * left_volume } else { 0 },
            if right { v * right_volume } else { 0 },
        );
    }
}

/// Averages the channel levels over each output sample and sends the result
/// through the DAC model and a high-pass filter.
pub(super) struct Mixer {
    /// Per-channel (left, right) level integrated over the current output
    /// sample, in level x 1/65536 dot.
    acc: [(i64, i64); N_CHANNELS],
    /// How much each channel's DAC is charged (1.0 = fully).
    dac_charge: [f32; N_CHANNELS],
    sample_rate: i32,
    /// Dots per output sample, 16.16 fixed point.
    sample_period: i64,
    hpf: HighPassFilter,
    render_timer: i64,
}

impl Mixer {
    pub(super) fn new(sample_rate: i32) -> Self {
        Self {
            acc: [(0, 0); N_CHANNELS],
            dac_charge: [0.0; N_CHANNELS],
            sample_rate,
            sample_period: Self::sample_period_from_rate(sample_rate),
            hpf: HighPassFilter::new(sample_rate),
            render_timer: 0,
        }
    }

    fn sample_period_from_rate(sample_rate: i32) -> i64 {
        (i64::from(DOTS_PER_SEC) << 16) / i64::from(sample_rate.max(1))
    }

    pub(super) fn set_sample_rate(&mut self, sample_rate: i32) {
        self.sample_rate = sample_rate;
        self.sample_period = Self::sample_period_from_rate(sample_rate);
        self.render_timer = 0;
        self.acc = [(0, 0); N_CHANNELS];
        self.hpf.set_sample_rate(sample_rate);
    }

    /// Mixes the channel `levels` over `ticks` APU ticks. `dacs` gives the
    /// state of the DAC of each channel, `None` when there are none (the
    /// AGB); it is only called when an output sample is rendered.
    pub(super) fn mix<C: AudioCallback, D: Fn() -> Option<[bool; N_CHANNELS]>>(
        &mut self,
        ticks: u32,
        levels: [(i32, i32); N_CHANNELS],
        dacs: D,
        callback: &C,
    ) {
        let mut remaining = (i64::from(ticks) * 2) << 16;
        while remaining > 0 {
            let step = remaining.min(self.sample_period - self.render_timer);
            for (acc, level) in self.acc.iter_mut().zip(&levels) {
                acc.0 += i64::from(level.0) * step;
                acc.1 += i64::from(level.1) * step;
            }
            self.render_timer += step;
            remaining -= step;
            if self.render_timer >= self.sample_period {
                self.render(dacs(), callback);
            }
        }
    }

    /// Produces one output sample (SameBoy's `render`).
    #[expect(clippy::float_arithmetic, clippy::cast_precision_loss)]
    fn render<C: AudioCallback>(&mut self, dacs: Option<[bool; N_CHANNELS]>, callback: &C) {
        const DAC_SPEED: f32 = 20_000.0;
        let period = self.sample_period as f32;
        let (mut left, mut right) = (0.0_f32, 0.0_f32);
        for ch in 0..N_CHANNELS {
            let mut multiplier = CH_STEP as f32;
            if let Some(dacs) = dacs {
                // The DAC charges and discharges instead of switching.
                let speed = DAC_SPEED / self.sample_rate as f32;
                let charge = &mut self.dac_charge[ch];
                *charge = if dacs[ch] {
                    (*charge + speed).min(1.0)
                } else {
                    (*charge - speed).max(0.0)
                };
                // Smoothstep: exactly 1 when charged and 0 when discharged.
                let c = *charge;
                multiplier *= (3.0 * c).mul_add(c, -(2.0 * c * c * c));
            }
            left = (self.acc[ch].0 as f32 / period).mul_add(multiplier, left);
            right = (self.acc[ch].1 as f32 / period).mul_add(multiplier, right);
        }
        self.acc = [(0, 0); N_CHANNELS];
        self.render_timer -= self.sample_period;

        #[expect(clippy::cast_possible_truncation)]
        let (l, r) = (
            left.clamp(-32768.0, 32767.0) as i16,
            right.clamp(-32768.0, 32767.0) as i16,
        );
        let (l, r) = self.hpf.high_pass(l, r);
        callback.audio_sample(l, r);
    }
}
