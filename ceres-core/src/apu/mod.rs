//! Audio processing unit.
//!
//! A port of SameBoy's `apu.c` (register behaviour, the channel state
//! machines, the DIV-driven frame sequencer and the revision specific
//! glitches), with a simple cycle-averaged mixer for the audio output.
//!
//! The APU is clocked in 2 MHz ticks: two per M-cycle in single speed, one in
//! double speed.

mod high_pass_filter;

use {
    crate::{Model, timing::DOTS_PER_SEC},
    high_pass_filter::HighPassFilter,
};

pub type Sample = i16;

pub trait AudioCallback {
    fn audio_sample(&self, l: Sample, r: Sample);
}

// Register offsets inside 0xFF00.
const NR10: usize = 0x10;
const NR11: usize = 0x11;
const NR12: usize = 0x12;
const NR13: usize = 0x13;
const NR14: usize = 0x14;
const NR21: usize = 0x16;
const NR22: usize = 0x17;
const NR23: usize = 0x18;
const NR24: usize = 0x19;
const NR30: usize = 0x1A;
const NR31: usize = 0x1B;
const NR32: usize = 0x1C;
const NR33: usize = 0x1D;
const NR34: usize = 0x1E;
const NR41: usize = 0x20;
const NR42: usize = 0x21;
const NR43: usize = 0x22;
const NR44: usize = 0x23;
const NR50: usize = 0x24;
const NR51: usize = 0x25;
const NR52: usize = 0x26;
const WAV_START: usize = 0x30;
const WAV_END: usize = 0x3F;

const SQUARE_1: usize = 0;
const SQUARE_2: usize = 1;
const WAVE: usize = 2;
const NOISE: usize = 3;
const N_CHANNELS: usize = 4;

/// Output amplitude of one DAC step (SameBoy's `CH_STEP`).
const CH_STEP: i32 = 0xFF0 / 0xF / 8;

const DUTIES: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 1, //
    1, 0, 0, 0, 0, 0, 0, 1, //
    1, 0, 0, 0, 0, 1, 1, 1, //
    0, 1, 1, 1, 1, 1, 1, 0,
];

const READ_MASK: [u8; 0x20] = [
    // NRX0  NRX1  NRX2  NRX3  NRX4
    0x80, 0x3F, 0x00, 0xFF, 0xBF, // NR1X
    0xFF, 0x3F, 0x00, 0xFF, 0xBF, // NR2X
    0x7F, 0xFF, 0x9F, 0xFF, 0xBF, // NR3X
    0xFF, 0xFF, 0x00, 0x00, 0xBF, // NR4X
    0x00, 0x00, 0x70, 0xFF, 0xFF, // NR5X
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // Unused
];

/// Machine state the APU needs on each access.
#[derive(Clone, Copy, Default)]
pub struct ApuCtx {
    pub address_bus: u16,
    pub div_counter: u16,
    pub double_speed: bool,
    /// A DIV write is in progress (it can produce an APU event).
    pub during_div_write: bool,
    pub pc: u16,
    pub stopped: bool,
}

/// SameBoy's model ordering: every DMG-family model sorts below the CGBs.
const fn rank(model: Model) -> u8 {
    match model {
        Model::Dmg0 | Model::DmgB | Model::Mgb | Model::Sgb | Model::Sgb2 => 0,
        Model::Cgb0 => 10,
        Model::CgbA => 11,
        Model::CgbB => 12,
        Model::CgbC => 13,
        Model::CgbD => 14,
        Model::CgbE => 15,
        Model::Agb => 16,
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct EnvelopeClock {
    locked: bool,
    clock: bool,
    should_lock: bool,
}

impl EnvelopeClock {
    fn set(&mut self, value: bool, direction: bool, volume: u8) {
        if self.clock == value {
            return;
        }
        if value {
            self.clock = true;
            self.should_lock = (volume == 0xF && direction) || (volume == 0x0 && !direction);
        } else {
            self.clock = false;
            self.locked |= self.should_lock;
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SquareChannel {
    pulse_length: u16,
    current_volume: u8,
    volume_countdown: u8,
    current_sample_index: u8,
    sample_surpressed: bool,
    sample_countdown: u16,
    sample_length: u16,
    length_enabled: bool,
    envelope_clock: EnvelopeClock,
    delay: u8,
    did_tick: bool,
    just_reloaded: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct WaveChannel {
    enable: bool,
    pulse_length: u16,
    shift: u8,
    sample_length: u16,
    length_enabled: bool,
    sample_countdown: u16,
    current_sample_index: u8,
    current_sample_byte: u8,
    wave_form_just_read: bool,
    pulsed: bool,
    bugged_read_countdown: u8,
}

#[derive(Clone, Copy, Debug, Default)]
struct NoiseChannel {
    pulse_length: u16,
    current_volume: u8,
    volume_countdown: u8,
    lfsr: u16,
    narrow: bool,
    counter_countdown: u8,
    counter: u16,
    length_enabled: bool,
    alignment: u8,
    current_lfsr_sample: bool,
    did_step_counter: bool,
    countdown_reloaded: bool,
    dmg_delayed_start: u8,
    envelope_clock: EnvelopeClock,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SkipDivEvent {
    #[default]
    Inactive,
    Skipped,
    Skip,
}

/// The state that powering the APU off clears (SameBoy's `GB_apu_t`).
#[derive(Clone, Copy, Debug, Default)]
struct State {
    global_enable: bool,
    samples: [u8; N_CHANNELS],
    is_active: [bool; N_CHANNELS],
    div_divider: u8,
    lf_div: u8,
    square_sweep_countdown: u8,
    square_sweep_calculate_countdown: u8,
    square_sweep_calculate_countdown_reload_timer: u8,
    sweep_length_addend: u16,
    shadow_sweep_sample_length: u16,
    unshifted_sweep: bool,
    square_sweep_instant_calculation_done: bool,
    channel_1_restart_hold: u8,
    channel1_completed_addend: u16,
    squares: [SquareChannel; 2],
    wave: WaveChannel,
    noise: NoiseChannel,
    skip_div_event: SkipDivEvent,
    pcm_mask: [u8; 2],
    pending_envelope_tick: bool,
    noise_counter_active: bool,
    noise_background_counter_active: bool,
    lfsr_stepped_in_narrow: bool,
    lfsr_bit_7_before_step: bool,
    noise_started_with_dac_disabled: bool,
}

/// What the boot ROM leaves in the APU, measured by running the real boot
/// ROMs: the start-up "ding" of channel 1 has decayed to volume 0, and the
/// frame sequencer and channel phases follow the length of the boot.
#[derive(Clone, Copy)]
pub struct PostBoot {
    div_divider: u8,
    sweep_countdown: u8,
    /// Channel 1 was triggered by the boot ROM (everything but the SGB).
    ch1_active: bool,
    ch1_volume_countdown: u8,
    ch1_sample_index: u8,
    ch1_sample_countdown: u16,
    ch1_pulse_length: u16,
    /// Volume countdown of channels 2 and 4.
    other_volume_countdown: u8,
    noise_alignment: u8,
}

impl PostBoot {
    /// `cgb_cart` is set when the cartridge header has the CGB flag.
    #[must_use]
    pub const fn new(model: Model, cgb_cart: bool) -> Self {
        let mut pb = Self {
            div_divider: 57,
            sweep_countdown: 0,
            ch1_active: true,
            ch1_volume_countdown: 2,
            ch1_sample_index: 6,
            ch1_sample_countdown: 79,
            ch1_pulse_length: 64,
            other_volume_countdown: 1,
            noise_alignment: 2,
        };
        match model {
            Model::Dmg0 => {
                pb.div_divider = 188;
                pb.sweep_countdown = 5;
                pb.ch1_sample_index = 4;
                pb.ch1_sample_countdown = 31;
                pb.noise_alignment = 248;
            }
            Model::DmgB | Model::Mgb => {
                pb.div_divider = 17;
                pb.sweep_countdown = 2;
                pb.ch1_volume_countdown = 1;
                pb.ch1_sample_index = 2;
                pb.ch1_sample_countdown = 11;
                pb.other_volume_countdown = 6;
                pb.noise_alignment = 198;
            }
            Model::Sgb | Model::Sgb2 => {
                pb.div_divider = 210;
                pb.sweep_countdown = 4;
                pb.ch1_active = false;
                pb.ch1_volume_countdown = 6;
                pb.ch1_sample_index = 0;
                pb.ch1_sample_countdown = 0xFFFF;
                pb.other_volume_countdown = 6;
                pb.noise_alignment = 4;
            }
            Model::Cgb0 => {
                pb.ch1_pulse_length = 62;
                if cgb_cart {
                    pb.ch1_sample_countdown = 73;
                    pb.noise_alignment = 28;
                } else {
                    pb.noise_alignment = 8;
                }
            }
            Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE => {
                if cgb_cart {
                    pb.div_divider = 56;
                    pb.ch1_sample_countdown = 73;
                    pb.noise_alignment = 22;
                }
            }
            Model::Agb => {
                if cgb_cart {
                    pb.div_divider = 56;
                    pb.ch1_sample_countdown = 71;
                    pb.noise_alignment = 24;
                } else {
                    pb.ch1_sample_countdown = 77;
                    pb.noise_alignment = 4;
                }
            }
        }
        pb
    }
}

pub struct Apu<A: AudioCallback> {
    audio_callback: A,
    acc_left: i64,
    acc_right: i64,
    ext_sample_period: i32,
    hpf: HighPassFilter,
    /// Output level (left, right) of each channel.
    levels: [(i32, i32); N_CHANNELS],
    model: Model,
    regs: [u8; 0x40],
    render_timer: i32,
    s: State,
}

impl<A: AudioCallback> Apu<A> {
    pub fn new(sample_rate: i32, audio_callback: A) -> Self {
        let mut apu = Self {
            audio_callback,
            acc_left: 0,
            acc_right: 0,
            ext_sample_period: Self::sample_period_from_rate(sample_rate),
            hpf: HighPassFilter::new(sample_rate),
            levels: [(0, 0); N_CHANNELS],
            model: Model::default(),
            regs: [0; 0x40],
            render_timer: 0,
            s: State::default(),
        };
        apu.s.pcm_mask = [0xFF; 2];
        apu
    }

    pub fn debug_state(&self) -> alloc::string::String {
        alloc::format!("{:#?}\nregs {:02X?}", self.s, &self.regs[0x10..0x30])
    }

    pub const fn set_model(&mut self, model: Model) {
        self.model = model;
    }

    pub fn reset(&mut self) {
        let wave: [u8; 0x10] = self.regs[WAV_START..=WAV_END].try_into().unwrap_or([0; 0x10]);
        self.s = State::default();
        self.s.pcm_mask = [0xFF; 2];
        self.regs = [0; 0x40];
        self.regs[WAV_START..=WAV_END].copy_from_slice(&wave);
        self.levels = [(0, 0); N_CHANNELS];
    }

    /// Sets the state the boot ROM leaves behind (see [`PostBoot`]).
    pub fn post_boot(&mut self, pb: PostBoot) {
        self.reset();
        self.regs[NR11] = 0x80;
        self.regs[NR12] = 0xF3;
        self.regs[NR13] = 0xC1;
        self.regs[NR14] = if pb.ch1_active { 0x87 } else { 0x07 };
        self.regs[NR50] = 0x77;
        self.regs[NR51] = 0xF3;
        self.regs[NR52] = 0x80;

        let agb = self.rank() > 15;
        let s = &mut self.s;
        s.global_enable = true;
        s.lf_div = 1;
        s.wave.shift = 4;
        s.div_divider = pb.div_divider;
        s.square_sweep_countdown = pb.sweep_countdown;
        s.squares[SQUARE_1].sample_countdown = pb.ch1_sample_countdown;
        s.squares[SQUARE_2].sample_countdown = 0xFFFF;
        s.squares[SQUARE_1].sample_length = 0x7C1;
        s.squares[SQUARE_1].pulse_length = pb.ch1_pulse_length;
        s.squares[SQUARE_1].volume_countdown = pb.ch1_volume_countdown;
        s.squares[SQUARE_1].current_sample_index = pb.ch1_sample_index;
        s.squares[SQUARE_2].volume_countdown = pb.other_volume_countdown;
        s.noise.volume_countdown = pb.other_volume_countdown;
        s.noise.alignment = pb.noise_alignment;
        if pb.ch1_active {
            s.is_active[SQUARE_1] = true;
            s.squares[SQUARE_1].did_tick = true;
            s.squares[SQUARE_1].envelope_clock.locked = true;
            s.squares[SQUARE_1].envelope_clock.should_lock = true;
        }
        if agb {
            for i in 0..N_CHANNELS {
                self.update_sample(i, 0);
            }
        } else {
            if pb.ch1_active {
                // The decayed note leaves its DC level behind.
                self.update_sample(SQUARE_1, 1);
                self.update_sample(SQUARE_1, 0);
            }
            for sample in &mut self.s.samples[1..] {
                *sample = 0x10;
            }
        }
    }

    const fn sample_period_from_rate(sample_rate: i32) -> i32 {
        DOTS_PER_SEC / sample_rate
    }

    pub fn set_sample_rate(&mut self, sample_rate: i32) {
        self.ext_sample_period = Self::sample_period_from_rate(sample_rate);
        self.hpf.set_sample_rate(sample_rate);
    }

    // -- model helpers (SameBoy compares against its model enum) ----------

    const fn cgb(&self) -> bool {
        self.model.is_cgb_hardware()
    }

    const fn rank(&self) -> u8 {
        rank(self.model)
    }

    // -- accessors used by the rest of the emulator -----------------------

    /// Every machine step starts with an unmasked PCM (SameBoy's "sort of
    /// hacky, but too many cross-component interactions to do it right").
    pub const fn reset_pcm_mask(&mut self) {
        self.s.pcm_mask = [0xFF; 2];
    }

    #[must_use]
    pub const fn pcm12(&self) -> u8 {
        let value = (if self.s.is_active[SQUARE_2] {
            self.s.samples[SQUARE_2] << 4
        } else {
            0
        }) | (if self.s.is_active[SQUARE_1] {
            self.s.samples[SQUARE_1]
        } else {
            0
        });
        value & if self.rank() <= 13 { self.s.pcm_mask[0] } else { 0xFF }
    }

    #[must_use]
    pub const fn pcm34(&self) -> u8 {
        let value = (if self.s.is_active[NOISE] {
            self.s.samples[NOISE] << 4
        } else {
            0
        }) | (if self.s.is_active[WAVE] {
            self.s.samples[WAVE]
        } else {
            0
        });
        value & if self.rank() <= 13 { self.s.pcm_mask[1] } else { 0xFF }
    }

    // -- DAC / mixing -----------------------------------------------------

    fn dac_enabled(&self, index: usize) -> bool {
        if self.rank() > 15 {
            // On the AGB the channels are mixed digitally: there are no
            // per-channel DACs.
            return true;
        }
        match index {
            SQUARE_1 => self.regs[NR12] & 0xF8 != 0,
            SQUARE_2 => self.regs[NR22] & 0xF8 != 0,
            WAVE => self.s.wave.enable,
            _ => self.regs[NR42] & 0xF8 != 0,
        }
    }

    fn agb_bias_for_channel(&self, index: usize) -> u8 {
        if !self.s.is_active[index] {
            return 0;
        }
        match index {
            SQUARE_1 => self.s.squares[SQUARE_1].current_volume,
            SQUARE_2 => self.s.squares[SQUARE_2].current_volume,
            WAVE => 0,
            _ => self.s.noise.current_volume,
        }
    }

    fn update_sample(&mut self, index: usize, value: u8) {
        if self.rank() > 15 {
            // AGB: a channel that is not connected is identical to a
            // connected channel playing PCM sample 0.
            self.s.samples[index] = value;
            let right_volume = i32::from(self.regs[NR50] & 7) + 1;
            let left_volume = i32::from((self.regs[NR50] >> 4) & 7) + 1;
            let mut value = i32::from(value);
            let mut silence = 0;
            if index == WAVE {
                // Channel 3 is inverted on the AGB and has another "silence".
                value ^= 0xF;
                silence = 7 * 2;
            }
            let bias = i32::from(self.agb_bias_for_channel(index));
            let left = self.regs[NR51] & (0x10 << index) != 0;
            let right = self.regs[NR51] & (1 << index) != 0;
            self.levels[index] = (
                (0xF - if left { value * 2 + bias } else { silence }) * left_volume,
                (0xF - if right { value * 2 + bias } else { silence }) * right_volume,
            );
            return;
        }

        let mut value = value;
        if value == 0 && self.s.samples[index] == 0 {
            return;
        }
        if self.dac_enabled(index) {
            self.s.samples[index] = value;
        } else {
            value = self.s.samples[index];
        }

        let right_volume = if self.regs[NR51] & (1 << index) != 0 {
            i32::from(self.regs[NR50] & 7) + 1
        } else {
            0
        };
        let left_volume = if self.regs[NR51] & (0x10 << index) != 0 {
            i32::from((self.regs[NR50] >> 4) & 7) + 1
        } else {
            0
        };
        let v = 0xF - i32::from(value) * 2;
        self.levels[index] = (v * left_volume, v * right_volume);
    }

    fn update_square_sample(&mut self, index: usize) {
        if self.s.squares[index].sample_surpressed {
            if self.rank() > 15 {
                self.update_sample(index, self.s.samples[index]);
            }
            return;
        }
        let duty = self.regs[if index == SQUARE_1 { NR11 } else { NR21 }] >> 6;
        let volume = self.s.squares[index].current_volume;
        let on = DUTIES[usize::from(self.s.squares[index].current_sample_index)
            + usize::from(duty) * 8]
            != 0;
        self.update_sample(index, if on { volume } else { 0 });
    }

    fn update_wave_sample(&mut self) {
        let wave = self.s.wave;
        let nibble = if wave.current_sample_index & 1 != 0 {
            wave.current_sample_byte & 0xF
        } else {
            wave.current_sample_byte >> 4
        };
        self.update_sample(WAVE, nibble >> wave.shift);
    }

    fn update_lfsr(&mut self) {
        self.s.noise.current_lfsr_sample = self.s.noise.lfsr & 1 != 0;
        if self.s.is_active[NOISE] {
            let value = if self.s.noise.current_lfsr_sample {
                self.s.noise.current_volume
            } else {
                0
            };
            self.update_sample(NOISE, value);
        }
    }

    fn step_lfsr(&mut self) {
        self.s.lfsr_bit_7_before_step = self.s.noise.lfsr & 0x80 != 0;
        let high_bit_mask: u16 = if self.s.noise.narrow { 0x4040 } else { 0x4000 };
        let new_high_bit = (self.s.noise.lfsr ^ (self.s.noise.lfsr >> 1) ^ 1) & 1 != 0;
        self.s.noise.lfsr >>= 1;
        if new_high_bit {
            self.s.noise.lfsr |= high_bit_mask;
        } else {
            // Relevant when switching LFSR widths.
            self.s.noise.lfsr &= !high_bit_mask;
        }
        self.update_lfsr();
        self.s.lfsr_stepped_in_narrow = self.s.noise.narrow;
    }

    /// Cycle-averaged mixing of the channel levels over `ticks` APU ticks.
    fn mix(&mut self, ticks: u32) {
        let mut remaining = (ticks * 2) as i32;
        let (mut left, mut right) = (0i32, 0i32);
        for (l, r) in self.levels {
            left += l;
            right += r;
        }
        left *= CH_STEP;
        right *= CH_STEP;

        while remaining > 0 {
            let step = remaining.min(self.ext_sample_period - self.render_timer);
            self.acc_left += i64::from(left) * i64::from(step);
            self.acc_right += i64::from(right) * i64::from(step);
            self.render_timer += step;
            remaining -= step;
            if self.render_timer >= self.ext_sample_period {
                let period = i64::from(self.ext_sample_period);
                let l = (self.acc_left / period).clamp(-0x8000, 0x7FFF) as i16;
                let r = (self.acc_right / period).clamp(-0x8000, 0x7FFF) as i16;
                self.acc_left = 0;
                self.acc_right = 0;
                self.render_timer = 0;
                let (l, r) = self.hpf.high_pass(l, r);
                self.audio_callback.audio_sample(l, r);
            }
        }
    }

    // -- envelopes ----------------------------------------------------------

    fn nrx2_glitch_inner(
        volume: &mut u8,
        value: u8,
        old_value: u8,
        countdown: &mut u8,
        lock: &mut EnvelopeClock,
    ) {
        if lock.clock {
            *countdown = value & 7;
        }
        let mut should_tick = (value & 7) != 0 && (old_value & 7) == 0 && !lock.locked;
        let should_invert = (value & 8) ^ (old_value & 8) != 0;

        if (value & 0xF) == 8 && (old_value & 0xF) == 8 && !lock.locked {
            should_tick = true;
        }

        if should_invert {
            // The way the clocks of this counter are connected cause some
            // odd ways for it to invert.
            if value & 8 != 0 {
                if (old_value & 7) == 0 && !lock.locked {
                    *volume ^= 0xF;
                } else {
                    *volume = 0xEu8.wrapping_sub(*volume) & 0xF;
                }
                should_tick = false; // Somehow prevents ticking?
            } else {
                *volume = 0x10u8.wrapping_sub(*volume) & 0xF;
            }
        }
        if should_tick {
            if value & 8 != 0 {
                *volume = volume.wrapping_add(1);
            } else {
                *volume = volume.wrapping_sub(1);
            }
            *volume &= 0xF;
        } else if (value & 7) == 0 && lock.clock {
            lock.set(false, false, 0);
        }
    }

    /// Writes to NRx2 of an active channel (the "zombie mode" volume glitch).
    fn nrx2_glitch(
        rank: u8,
        volume: &mut u8,
        value: u8,
        old_value: u8,
        countdown: &mut u8,
        lock: &mut EnvelopeClock,
    ) {
        // Note: before CGB-D some of these are non-deterministic.
        if rank <= 13 {
            Self::nrx2_glitch_inner(volume, 0xFF, old_value, countdown, lock);
            Self::nrx2_glitch_inner(volume, value, 0xFF, countdown, lock);
        } else {
            Self::nrx2_glitch_inner(volume, value, old_value, countdown, lock);
        }
    }

    fn tick_square_envelope(&mut self, ctx: &ApuCtx, index: usize) {
        self.s.squares[index].envelope_clock.set(false, false, 0);
        if self.s.squares[index].envelope_clock.locked {
            return;
        }
        let nrx2 = self.regs[if index == SQUARE_1 { NR12 } else { NR22 }];
        if nrx2 & 7 == 0 {
            return;
        }
        if ctx.double_speed {
            if index == SQUARE_1 {
                self.s.pcm_mask[0] &= self.s.squares[SQUARE_1].current_volume | 0xF1;
            } else {
                // CGB-0 behaviour is instance specific and non-deterministic.
                let mask = if self.model == Model::Cgb0 {
                    if self.s.squares[SQUARE_2].current_volume == 1 && nrx2 & 8 != 0 {
                        0x1F
                    } else {
                        0x3F
                    }
                } else {
                    0x3F
                };
                self.s.pcm_mask[0] &= (self.s.squares[SQUARE_2].current_volume << 4) | mask;
            }
        }

        self.s.squares[index].envelope_clock.set(false, false, 0);

        let volume = &mut self.s.squares[index].current_volume;
        if nrx2 & 8 != 0 {
            *volume = volume.wrapping_add(1);
        } else {
            *volume = volume.wrapping_sub(1);
        }

        if self.s.is_active[index] {
            self.update_square_sample(index);
        }
    }

    fn tick_noise_envelope(&mut self, ctx: &ApuCtx) {
        self.s.noise.envelope_clock.set(false, false, 0);
        if self.s.noise.envelope_clock.locked {
            return;
        }
        let nr42 = self.regs[NR42];
        if nr42 & 7 == 0 {
            return;
        }
        if ctx.double_speed {
            self.s.pcm_mask[1] &= (self.s.noise.current_volume << 4) | 0x1F;
        }

        if nr42 & 8 != 0 {
            self.s.noise.current_volume = self.s.noise.current_volume.wrapping_add(1);
        } else {
            self.s.noise.current_volume = self.s.noise.current_volume.wrapping_sub(1);
        }

        if self.s.is_active[NOISE] {
            let value = if self.s.noise.lfsr & 1 != 0 {
                self.s.noise.current_volume
            } else {
                0
            };
            self.update_sample(NOISE, value);
        }
    }

    // -- sweep ----------------------------------------------------------------

    fn sweep_calculation_done(&mut self) {
        // APU bug: the sweep frequency is checked after adding the delta twice.
        if self.s.channel_1_restart_hold == 0 {
            self.s.shadow_sweep_sample_length = self.s.squares[SQUARE_1].sample_length;
        }
        if self.regs[NR10] & 8 != 0 {
            self.s.sweep_length_addend ^= 0x7FF;
        }
        if u32::from(self.s.shadow_sweep_sample_length) + u32::from(self.s.sweep_length_addend)
            > 0x7FF
            && self.regs[NR10] & 8 == 0
        {
            self.s.is_active[SQUARE_1] = false;
            self.update_sample(SQUARE_1, 0);
        }
        self.s.channel1_completed_addend = self.s.sweep_length_addend;
    }

    fn trigger_sweep_calculation(&mut self, ctx: &ApuCtx) {
        if self.regs[NR10] & 0x70 != 0 && self.s.square_sweep_countdown == 7 {
            if self.regs[NR10] & 0x07 != 0 {
                let sq = &mut self.s.squares[SQUARE_1];
                sq.sample_length = self
                    .s
                    .sweep_length_addend
                    .wrapping_add(self.s.shadow_sweep_sample_length)
                    .wrapping_add(u16::from(self.regs[NR10] & 0x8 != 0));
                sq.sample_length &= 0x7FF;
            }
            if self.s.channel_1_restart_hold == 0 {
                self.s.sweep_length_addend = self.s.squares[SQUARE_1].sample_length;
                self.s.sweep_length_addend >>= self.regs[NR10] & 7;
            }

            // Recalculation and the overflow check only occur after a delay.
            self.s.square_sweep_calculate_countdown = self.regs[NR10] & 0x7;
            self.s.square_sweep_calculate_countdown_reload_timer = 1 + self.s.lf_div;
            if !ctx.double_speed && ctx.during_div_write {
                self.s.square_sweep_calculate_countdown_reload_timer = 1;
            }
            self.s.unshifted_sweep = self.regs[NR10] & 0x7 == 0;
            self.s.square_sweep_countdown = ((self.regs[NR10] >> 4) & 7) ^ 7;
            if self.s.square_sweep_calculate_countdown == 0 {
                self.s.square_sweep_instant_calculation_done = true;
            }
        }
    }

    // -- frame sequencer ------------------------------------------------------

    pub fn delayed_envelope_tick(&mut self, ctx: &ApuCtx) {
        self.s.pending_envelope_tick = false;
        if !self.s.global_enable {
            return;
        }
        self.s.pcm_mask = [0xFF; 2];

        for i in SQUARE_1..=SQUARE_2 {
            if self.s.squares[i].envelope_clock.clock {
                self.tick_square_envelope(ctx, i);
            }
        }
        if self.s.noise.envelope_clock.clock {
            self.tick_noise_envelope(ctx);
        }
    }

    #[must_use]
    pub const fn pending_envelope_tick(&self) -> bool {
        self.s.pending_envelope_tick
    }

    pub fn div_event(&mut self, ctx: &ApuCtx) {
        self.s.pcm_mask = [0xFF; 2];

        if !self.s.global_enable {
            return;
        }
        if self.s.skip_div_event == SkipDivEvent::Skip {
            self.s.skip_div_event = SkipDivEvent::Skipped;
            return;
        }
        if self.s.skip_div_event == SkipDivEvent::Skipped {
            self.s.skip_div_event = SkipDivEvent::Inactive;
        } else {
            self.s.div_divider = self.s.div_divider.wrapping_add(1);
        }

        if self.s.div_divider & 7 == 7 {
            for i in SQUARE_1..=SQUARE_2 {
                if !self.s.squares[i].envelope_clock.clock {
                    self.s.squares[i].volume_countdown =
                        self.s.squares[i].volume_countdown.wrapping_sub(1) & 7;
                }
            }
            if !self.s.noise.envelope_clock.clock {
                self.s.noise.volume_countdown = self.s.noise.volume_countdown.wrapping_sub(1) & 7;
            }
        }

        if ctx.double_speed && matches!(self.model, Model::CgbD | Model::CgbE) {
            self.s.pending_envelope_tick = true;
        } else {
            for i in SQUARE_1..=SQUARE_2 {
                if self.s.squares[i].envelope_clock.clock {
                    self.tick_square_envelope(ctx, i);
                }
            }
            if self.s.noise.envelope_clock.clock {
                self.tick_noise_envelope(ctx);
            }
        }

        if self.s.div_divider & 1 == 1 {
            for i in SQUARE_1..=SQUARE_2 {
                if self.s.squares[i].length_enabled && self.s.squares[i].pulse_length != 0 {
                    self.s.squares[i].pulse_length -= 1;
                    if self.s.squares[i].pulse_length == 0 {
                        self.s.is_active[i] = false;
                        self.update_sample(i, 0);
                    }
                }
            }

            if self.s.wave.length_enabled && self.s.wave.pulse_length != 0 {
                self.s.wave.pulse_length -= 1;
                if self.s.wave.pulse_length == 0 {
                    if self.s.is_active[WAVE] && self.rank() > 15 {
                        if self.s.wave.sample_countdown == 0 {
                            self.s.wave.current_sample_byte = self.regs[WAV_START
                                + usize::from(((self.s.wave.current_sample_index + 1) & 0xF) >> 1)];
                        } else if self.s.wave.sample_countdown == 9 {
                            self.s.wave.current_sample_byte = self.regs[WAV_START];
                        }
                    }
                    self.s.is_active[WAVE] = false;
                    self.update_sample(WAVE, 0);
                }
            }

            if self.s.noise.length_enabled && self.s.noise.pulse_length != 0 {
                self.s.noise.pulse_length -= 1;
                if self.s.noise.pulse_length == 0 {
                    self.s.is_active[NOISE] = false;
                    self.update_sample(NOISE, 0);
                }
            }
        }

        if self.s.div_divider & 3 == 3 {
            self.s.square_sweep_countdown = (self.s.square_sweep_countdown + 1) & 7;
            self.trigger_sweep_calculation(ctx);
        }
    }

    pub fn div_secondary_event(&mut self) {
        self.s.pcm_mask = [0xFF; 2];

        if !self.s.global_enable {
            return;
        }
        for i in SQUARE_1..=SQUARE_2 {
            let nrx2 = self.regs[if i == SQUARE_1 { NR12 } else { NR22 }];
            if self.s.is_active[i] && self.s.squares[i].volume_countdown == 0 {
                self.s.squares[i].volume_countdown = nrx2 & 7;
                let volume = self.s.squares[i].current_volume;
                self.s.squares[i]
                    .envelope_clock
                    .set(nrx2 & 7 != 0, nrx2 & 8 != 0, volume);
            }
        }

        if self.s.is_active[NOISE] && self.s.noise.volume_countdown == 0 {
            let nr42 = self.regs[NR42];
            self.s.noise.volume_countdown = nr42 & 7;
            let volume = self.s.noise.current_volume;
            self.s.noise
                .envelope_clock
                .set(nr42 & 7 != 0, nr42 & 8 != 0, volume);
        }
    }

    // -- running --------------------------------------------------------------

    /// Runs the APU for `ticks` ticks and feeds the mixer.
    pub fn tick(&mut self, ctx: &ApuCtx, ticks: u32) {
        self.run_ticks(ctx, ticks);
        self.mix(ticks);
    }

    fn run_ticks(&mut self, ctx: &ApuCtx, mut cycles: u32) {
        if cycles == 0 {
            return;
        }

        if self.s.wave.bugged_read_countdown != 0 {
            let mut cycles_left = cycles;
            while cycles_left != 0 {
                cycles_left -= 1;
                self.s.wave.bugged_read_countdown = self.s.wave.bugged_read_countdown.wrapping_sub(1);
                if self.s.wave.bugged_read_countdown == 0 {
                    self.s.wave.current_sample_byte =
                        self.regs[WAV_START + usize::from(ctx.address_bus & 0xF)];
                    if self.s.is_active[WAVE] {
                        self.update_wave_sample();
                    }
                    break;
                }
            }
        }

        let mut start_ch4 = false;
        if !ctx.stopped || self.cgb() {
            let delayed = u32::from(self.s.noise.dmg_delayed_start);
            if delayed != 0 {
                if delayed == cycles {
                    self.s.noise.dmg_delayed_start = 0;
                    start_ch4 = true;
                } else if delayed > cycles {
                    self.s.noise.dmg_delayed_start -= cycles as u8;
                } else {
                    // Split it into two.
                    cycles -= delayed;
                    self.run_ticks(ctx, delayed);
                }
            }

            // To align the square signal to 1 MHz.
            self.s.lf_div ^= (cycles & 1) as u8;
            self.s.noise.alignment = self.s.noise.alignment.wrapping_add(cycles as u8);

            let mut sweep_cycles = cycles / 2;
            if cycles & 1 != 0 && self.s.lf_div == 0 {
                sweep_cycles += 1;
            }

            let reload = u32::from(self.s.square_sweep_calculate_countdown_reload_timer);
            if reload > sweep_cycles {
                self.s.square_sweep_calculate_countdown_reload_timer -= sweep_cycles as u8;
                sweep_cycles = 0;
            } else {
                if reload != 0
                    && self.s.square_sweep_calculate_countdown == 0
                    && self.s.square_sweep_instant_calculation_done
                {
                    self.sweep_calculation_done();
                }
                self.s.square_sweep_instant_calculation_done = false;
                sweep_cycles -= reload;
                self.s.square_sweep_calculate_countdown_reload_timer = 0;
            }

            if self.s.square_sweep_calculate_countdown != 0
                && (self.regs[NR10] & 7 != 0 || self.s.unshifted_sweep)
            {
                // The calculation is paused if the lower bits are 0.
                if u32::from(self.s.square_sweep_calculate_countdown) > sweep_cycles {
                    self.s.square_sweep_calculate_countdown -= sweep_cycles as u8;
                } else {
                    self.s.square_sweep_calculate_countdown = 0;
                    self.sweep_calculation_done();
                }
            }

            if self.s.channel_1_restart_hold != 0 {
                if u32::from(self.s.channel_1_restart_hold) > cycles {
                    self.s.channel_1_restart_hold -= cycles as u8;
                } else {
                    self.s.channel_1_restart_hold = 0;
                }
            }

            for i in SQUARE_1..=SQUARE_2 {
                if !self.s.is_active[i] {
                    continue;
                }
                let mut cycles_left = cycles;
                if self.s.squares[i].delay != 0 {
                    if u32::from(self.s.squares[i].delay) < cycles_left {
                        self.s.squares[i].delay = 0;
                    } else {
                        self.s.squares[i].delay -= cycles_left as u8;
                    }
                }
                while cycles_left > u32::from(self.s.squares[i].sample_countdown) {
                    cycles_left -= u32::from(self.s.squares[i].sample_countdown) + 1;
                    let sq = &mut self.s.squares[i];
                    sq.sample_countdown = (sq.sample_length ^ 0x7FF) * 2 + 1;
                    sq.current_sample_index = (sq.current_sample_index + 1) & 7;
                    sq.sample_surpressed = false;
                    if cycles_left == 0 && self.s.samples[i] == 0 {
                        self.s.pcm_mask[0] &= if i == SQUARE_1 { 0xF0 } else { 0x0F };
                    }
                    self.s.squares[i].did_tick = true;
                    self.update_square_sample(i);
                }
                self.s.squares[i].just_reloaded = cycles_left == 0;
                if cycles_left != 0 {
                    self.s.squares[i].sample_countdown -= cycles_left as u16;
                }
            }

            self.s.wave.wave_form_just_read = false;
            if self.s.is_active[WAVE] {
                let mut cycles_left = cycles;
                while cycles_left > u32::from(self.s.wave.sample_countdown) {
                    cycles_left -= u32::from(self.s.wave.sample_countdown) + 1;
                    self.s.wave.sample_countdown = self.s.wave.sample_length ^ 0x7FF;
                    self.s.wave.current_sample_index = (self.s.wave.current_sample_index + 1) & 0x1F;
                    self.s.wave.current_sample_byte =
                        self.regs[WAV_START + usize::from(self.s.wave.current_sample_index >> 1)];
                    self.update_wave_sample();
                    self.s.wave.wave_form_just_read = true;
                }
                if cycles_left != 0 {
                    self.s.wave.sample_countdown -= cycles_left as u16;
                    self.s.wave.wave_form_just_read = false;
                }
            } else if self.s.wave.enable && self.s.wave.pulsed && self.rank() <= 15 {
                let mut cycles_left = cycles;
                while cycles_left > u32::from(self.s.wave.sample_countdown) {
                    cycles_left -= u32::from(self.s.wave.sample_countdown) + 1;
                    self.s.wave.sample_countdown = self.s.wave.sample_length ^ 0x7FF;
                    if cycles_left != 0 {
                        self.s.wave.current_sample_byte =
                            self.regs[WAV_START + usize::from(ctx.address_bus & 0xF)];
                    } else {
                        self.s.wave.bugged_read_countdown = 1;
                    }
                }
                if cycles_left != 0 {
                    self.s.wave.sample_countdown -= cycles_left as u16;
                }
                if self.s.wave.sample_countdown == 0 {
                    self.s.wave.bugged_read_countdown = 2;
                }
            }

            if self.s.noise_counter_active || self.s.noise_background_counter_active {
                let mut cycles_left = cycles;
                let mut divisor = (self.regs[NR43] & 0x07) << 2;
                if divisor == 0 {
                    divisor = 2;
                }
                if self.s.noise.counter_countdown == 0 {
                    self.s.noise.counter_countdown = divisor;
                }
                while cycles_left >= u32::from(self.s.noise.counter_countdown) {
                    cycles_left -= u32::from(self.s.noise.counter_countdown);
                    self.s.noise.counter_countdown = divisor;
                    let mask: u16 = 1 << (self.regs[NR43] >> 4);
                    let old_bit = self.s.noise.counter & mask != 0;
                    self.s.noise.counter = (self.s.noise.counter + 1) & 0x3FFF;
                    self.s.noise.did_step_counter = true;
                    let new_bit = self.s.noise.counter & mask != 0;

                    // Step the LFSR.
                    if new_bit && !old_bit && self.s.is_active[NOISE] {
                        if cycles_left == 0 && self.s.samples[NOISE] == 0 && !ctx.double_speed {
                            self.s.pcm_mask[1] &= 0x0F;
                        }
                        self.step_lfsr();
                    }
                }
                if cycles_left != 0 {
                    if self.s.noise_counter_active || self.s.noise_background_counter_active {
                        self.s.noise.counter_countdown -= cycles_left as u8;
                        self.s.noise.countdown_reloaded = false;
                    }
                } else {
                    self.s.noise.countdown_reloaded = true;
                }
            }
        }

        if start_ch4 {
            let value = self.regs[NR44] | 0x80;
            self.write(ctx, NR44, value);
        }
    }

    // -- register access ----------------------------------------------------

    /// Reads a register (`reg` is the low byte of the 0xFF10..=0xFF3F address).
    #[must_use]
    pub fn read(&self, reg: usize) -> u8 {
        if reg == NR52 {
            let mut value = 0;
            for i in 0..N_CHANNELS {
                value >>= 1;
                if self.s.is_active[i] {
                    value |= 0x8;
                }
            }
            if self.s.global_enable {
                value |= 0x80;
            }
            return value | 0x70;
        }

        let mut reg = reg;
        if (WAV_START..=WAV_END).contains(&reg) && self.s.is_active[WAVE] {
            if !self.cgb() && !self.s.wave.wave_form_just_read {
                return 0xFF;
            }
            if self.rank() > 15 {
                return 0xFF;
            }
            reg = WAV_START + usize::from(self.s.wave.current_sample_index / 2);
        }

        let mask = if reg < 0x30 { READ_MASK[reg - NR10] } else { 0 };
        self.regs[reg] | mask
    }

    fn nr10_write_glitch(&mut self, ctx: &ApuCtx, value: u8) {
        // TODO (SameBoy): check all of these in APU odd mode.
        if self.rank() <= 13 {
            if self.s.square_sweep_calculate_countdown_reload_timer == 1 && self.s.lf_div == 0 {
                if ctx.double_speed {
                    // Instance-specific data corruption (two CGB-Cs and a CGB-A).
                    const CORRUPTION: [u8; 8] = [7, 7, 5, 7, 3, 3, 5, 7];
                    self.s.square_sweep_calculate_countdown =
                        CORRUPTION[usize::from(self.s.square_sweep_calculate_countdown & 7)];
                }
            } else if self.s.square_sweep_calculate_countdown_reload_timer > 1 {
                if ctx.double_speed {
                    self.s.square_sweep_calculate_countdown = value & 7;
                }
            } else if self.s.square_sweep_calculate_countdown != 0 {
                let mut should_zombie_step = false;
                if self.regs[NR10] & 7 == 0 {
                    should_zombie_step = (self.s.lf_div != 0) ^ ctx.double_speed;
                } else if ctx.double_speed && self.s.square_sweep_calculate_countdown == 1 {
                    should_zombie_step = true;
                }

                if should_zombie_step {
                    self.s.square_sweep_calculate_countdown -= 1;
                    if self.s.square_sweep_calculate_countdown <= 1 {
                        self.s.square_sweep_calculate_countdown = 0;
                        self.sweep_calculation_done();
                    }
                }
            }
        } else {
            if self.s.square_sweep_calculate_countdown_reload_timer == 2 {
                // The countdown just reloaded: re-reload it.
                self.s.square_sweep_calculate_countdown = value & 0x7;
                if self.s.square_sweep_calculate_countdown == 0 {
                    self.s.square_sweep_calculate_countdown_reload_timer = 0;
                }
            }
            if value & 7 != 0
                && self.regs[NR10] & 7 == 0
                && self.s.lf_div == 0
                && self.s.square_sweep_calculate_countdown > 1
            {
                self.s.square_sweep_calculate_countdown -= 1;
                if self.s.square_sweep_calculate_countdown == 0 {
                    self.sweep_calculation_done();
                }
            }
        }
    }

    fn prepare_noise_start(&mut self, ctx: &ApuCtx) {
        let rank = self.rank();
        let ds = ctx.double_speed;
        self.s.noise_counter_active = self.regs[NR42] & 0xF8 != 0;
        let was_started_with_dac_disabled = self.s.noise_started_with_dac_disabled;
        self.s.noise_started_with_dac_disabled = !self.s.noise_counter_active;
        let mut divisor = i32::from(self.regs[NR43] & 0x07);
        let was_background_counting = self.s.noise_background_counter_active;
        self.s.noise_background_counter_active = true;
        let mut instant_step = false;
        let mut div_1_glitch = false;
        let active = self.s.is_active[NOISE];

        if divisor > 1 && self.s.noise.counter_countdown == 1 {
            self.s.noise.counter = (self.s.noise.counter + 1) & 0x3FFF;
        } else if divisor > 1
            && self.s.noise.counter_countdown == 2
            && active
            && rank <= 13
            && ds
        {
            self.s.noise.counter = (self.s.noise.counter + 1) & 0x3FFF;
        } else if self.s.noise.counter_countdown == 2
            && self.s.noise.alignment & 3 == 0
            && active
        {
            if divisor == 0 {
                divisor = 8;
            } else if divisor == 1 {
                if !self.s.noise.did_step_counter {
                    div_1_glitch = true;
                }
                let mask: u16 = 1 << (self.regs[NR43] >> 4);
                let old_bit = self.s.noise.counter & mask != 0;
                self.s.noise.counter = (self.s.noise.counter + 1) & 0x3FFF;
                let new_bit = self.s.noise.counter & mask != 0;
                if new_bit && !old_bit {
                    instant_step = true;
                }
            }
        }
        let mut countdown: i32 = if divisor == 0 { 6 } else { divisor * 4 + 6 };
        let alignment = self.s.noise.alignment;
        if alignment & 1 != 0 {
            if divisor == 0 {
                if rank <= 13 || !was_background_counting {
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
                if ds && rank <= 13 && divisor == 1 {
                    countdown += 2;
                } else {
                    countdown -= 2;
                }
            } else if divisor > 1 && (!ds || rank > 13) {
                countdown -= 4;
            } else if divisor == 1 && active && self.regs[NR43] & 0xF0 == 0 {
                // This quirk seems way too specific.
                countdown -= 4;
            }
        } else if ds && rank <= 13 {
            countdown += 2;
        }

        // Background counting glitches (double speed is not tested).
        if divisor > 1 {
            if !self.s.noise_counter_active && alignment & 3 == 0 {
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

        if divisor == 0 && rank <= 13 && was_background_counting && !active && ds {
            countdown -= 1;
        }
        if div_1_glitch {
            countdown -= 4;
        }
        self.s.noise.counter_countdown = countdown as u8;

        if divisor == 0 && active && alignment & 3 == 3 {
            // Seemingly arbitrary, but confirmed for this edge case.
            self.s.noise.lfsr = 0x0055;
        } else {
            self.s.noise.lfsr = 0;
        }
        if instant_step {
            self.step_lfsr();
        }
    }

    fn nr43_write(&mut self, new: u8) {
        let rank = self.rank();
        let old_narrow = self.s.noise.narrow;
        self.s.noise.narrow = new & 8 != 0;
        let old = self.regs[NR43];
        self.regs[NR43] = new;

        if old & 0xF0 == new & 0xF0 {
            return;
        }

        let mut effective_counter = self.s.noise.counter;
        if rank <= 13 && self.s.noise.countdown_reloaded {
            effective_counter |= effective_counter.wrapping_sub(1) & 0x3FFF;
        }
        let bit = |counter: u16, shift: u8| (counter >> shift) & 1 != 0;
        let old_bit = bit(effective_counter, old >> 4);

        let mut glitch_value = (old & 0x7F) | (new & 0x80);
        let mut glitch_bit = bit(effective_counter, glitch_value >> 4);
        let new_bit = bit(effective_counter, new >> 4);
        let mut force_glitch = false;

        if self.model == Model::CgbD && new_bit && glitch_bit && old_bit && (old ^ new) & 0x70 != 0 {
            force_glitch = true;
        }

        if rank > 15 {
            // AGB behaviour is very glitchy and inconsistent; this is a very
            // rough approximation.
            let glitch_value2;
            if new >= 0x80 && old >= 0x80 {
                glitch_value = (old & 0xCF) | (new & 0x30);
                glitch_value2 = (old & 0x8F) | (new & 0x70);
            } else {
                glitch_value = (old & 0xDF) | (new & 0x20);
                glitch_value2 = (old & 0xCF) | (new & 0x30);
            }
            glitch_bit = bit(self.s.noise.counter, glitch_value >> 4);
            let glitch_bit2 = bit(self.s.noise.counter, glitch_value2 >> 4);
            if glitch_bit != glitch_bit2 {
                if new_bit == old_bit {
                    glitch_bit = !new_bit;
                } else if !glitch_bit && old_bit {
                    force_glitch = true;
                }
            }
        }

        // Step the LFSR.
        if (old_bit == new_bit && new_bit != glitch_bit) || force_glitch {
            // Glitching write, in two categories (both have non-deterministic
            // variants; these are the most common, deterministic ones).
            if new_bit {
                // Category 1.
                if rank >= 15 {
                    if new & 0x80 == 0 {
                        self.step_lfsr();
                    } else {
                        // Only happens under this odd condition.
                        let t1 = (old >> 4) & 7;
                        let t2 = (new >> 4) & 7;
                        if u32::from(t1 ^ 7) + u32::from(t2) > 7 || (t1 ^ 7) & t2 != 0 {
                            // Copy bit 8 to bit 7.
                            let n = &mut self.s.noise;
                            n.lfsr &= !0x80;
                            n.lfsr |= (n.lfsr >> 1) & 0x80;

                            // All specific cases have non-deterministic behaviours.
                            if (t1 == 0 || t1 == 4) && t2 == 3 {
                                self.s.noise.lfsr &= (self.s.noise.lfsr >> 1) | 0x545;
                                self.update_lfsr();
                            } else if t1 == 2 && t2 == 3 {
                                let mut mask: u16 = 0x555;
                                if self.s.noise.lfsr & 0xC == 0xC {
                                    mask |= 8;
                                }
                                if self.s.noise.lfsr & 0xC00 == 0xC00 {
                                    mask |= 0x800;
                                }
                                self.s.noise.lfsr &= (self.s.noise.lfsr >> 1) | mask;
                                self.update_lfsr();
                            }
                            if !self.s.noise.narrow && old_narrow && self.s.lfsr_stepped_in_narrow {
                                if self.s.lfsr_bit_7_before_step {
                                    self.s.noise.lfsr |= 0x40;
                                } else {
                                    self.s.noise.lfsr &= !0x40;
                                }
                            }
                            self.s.noise.lfsr |= if self.s.noise.narrow { 0x4040 } else { 0x4000 };
                            self.s.lfsr_stepped_in_narrow = self.s.noise.narrow;
                        }
                    }
                } else if self.model == Model::CgbD {
                    self.glitch_category_1_cgb_d(old, new, force_glitch);
                }
            } else if rank >= 15 {
                // Category 2.
                self.glitch_category_2_cgb_e(old, new);
            } else {
                self.step_lfsr();
            }
        } else if !old_bit && new_bit {
            if rank <= 13 {
                let previous_narrow = self.s.noise.narrow;
                self.s.noise.narrow = true;
                self.step_lfsr();
                self.s.noise.narrow = previous_narrow;
                if (new & 0xF0) <= 0x20 && glitch_bit && effective_counter & 8 == 0 {
                    // Non-deterministic, not fully tested for revision differences.
                    self.step_lfsr();
                    let narrow = self.s.noise.narrow;
                    self.s.noise.lfsr &= !(if narrow { 0x4040 } else { 0x4000 });
                    self.s.noise.lfsr |=
                        (self.s.noise.lfsr & (if narrow { 0x2020 } else { 0x2000 })) << 1;
                }
            } else {
                self.step_lfsr();
            }
        } else if rank <= 13
            && (new & 0xF0) <= 0x20
            && !glitch_bit
            && !new_bit
            && !old_bit
            && effective_counter & 8 != 0
        {
            // Step twice?
            self.step_lfsr();
        }
    }

    fn glitch_category_1_cgb_d(&mut self, old: u8, new: u8, force_glitch: bool) {
        const GLITCH_MAP_L2H: [u8; 64] = {
            let mut m = [0u8; 64];
            let rows: [[u8; 6]; 8] = [
                [0x00, 0x01, 0x01, 0x21, 0x02, 0x21],
                [0x03, 0x00, 0x21, 0x01, 0x04, 0x04],
                [0x05, 0x01, 0x00, 0x01, 0x04, 0x21],
                [0x03, 0x05, 0x05, 0x00, 0x01, 0x01],
                [0x05, 0x01, 0x01, 0x21, 0x00, 0x01],
                [0x05, 0x05, 0x21, 0x01, 0x05, 0x00],
                [0x05, 0x01, 0x05, 0x01, 0x05, 0x01],
                [0x03, 0x05, 0x05, 0x05, 0x05, 0x05],
            ];
            let mut r = 0;
            while r < 8 {
                let mut c = 0;
                while c < 6 {
                    m[r * 8 + c] = rows[r][c];
                    c += 1;
                }
                r += 1;
            }
            m
        };
        const GLITCH_MAP_H2L: [u8; 64] = {
            let mut m = [0u8; 64];
            let rows: [[u8; 8]; 6] = [
                [0x00, 0x27, 0x26, 0x37, 0x21, 0x38, 0x01, 0x01],
                [0x01, 0x00, 0x38, 0x21, 0x21, 0x21, 0x01, 0x01],
                [0x01, 0x27, 0x00, 0x28, 0x21, 0x38, 0x01, 0x01],
                [0x01, 0x02, 0x01, 0x00, 0x31, 0x21, 0x01, 0x01],
                [0x06, 0x28, 0x28, 0x38, 0x00, 0x27, 0x01, 0x01],
                [0x01, 0x03, 0x38, 0x21, 0x01, 0x00, 0x01, 0x01],
            ];
            let mut r = 0;
            while r < 6 {
                let mut c = 0;
                while c < 8 {
                    m[r * 8 + c] = rows[r][c];
                    c += 1;
                }
                r += 1;
            }
            m
        };

        let map = if old & 0x80 != 0 { &GLITCH_MAP_H2L } else { &GLITCH_MAP_L2H };
        let mut glitch = u32::from(map[usize::from(((old & 0x70) >> 1) | ((new & 0x70) >> 4))]);
        if force_glitch {
            if (new ^ old) & 0x80 == 0 {
                glitch = if glitch & 0x20 != 0 { 5 } else { 0 };
            } else if new & 0x80 == 0 {
                glitch = if glitch & 0x10 != 0 { 5 } else { 0 };
            } else if glitch & 0xF == 1 || glitch & 0xF == 4 {
                glitch = 5;
            } else {
                glitch = 0;
            }
        } else {
            glitch &= 0xF;
        }
        let old_lfsr = self.s.noise.lfsr;
        let lfsr_mask: u16 = if self.s.noise.narrow { 0x4040 } else { 0x4000 };

        // Emulates the C `switch` with its deliberate fall-through chain:
        // 6/4 -> 2 -> 1/8 -> 5.
        let mut stage = match glitch {
            6 | 4 => 6,
            2 => 5,
            1 | 8 => 4,
            5 => 3,
            7 => 100,
            3 => 101,
            _ => 0,
        };
        if stage == 6 {
            let probe = if glitch == 4 { 0x60 } else { 0x40 };
            if self.s.noise.lfsr & probe != 0x40 {
                stage = 5;
            } else {
                stage = 4;
            }
        }
        if stage == 5 {
            if self.s.noise.lfsr & 1 == 0 {
                self.s.noise.lfsr &= !2;
            }
            stage = 4;
        }
        if stage == 4 {
            self.step_lfsr();
            stage = 3;
        }
        if stage == 3 {
            if glitch != 8 || old_lfsr & 3 != 2 {
                self.s.noise.lfsr |= lfsr_mask;
            } else {
                self.s.noise.lfsr |= old_lfsr & lfsr_mask;
            }
        } else if stage == 100 {
            self.step_lfsr();
            self.s.noise.lfsr |= old_lfsr & lfsr_mask;
        } else if stage == 101 {
            self.step_lfsr();
            self.s.noise.lfsr &= old_lfsr;
            self.s.noise.lfsr |= old_lfsr & 1;
            self.s.noise.lfsr |= lfsr_mask;
            self.update_lfsr();
        }
    }

    fn glitch_category_2_cgb_e(&mut self, old: u8, new: u8) {
        const GLITCH_MAP: [u8; 64] = {
            let mut m = [0u8; 64];
            // Indexed by (old & 0x70) >> 1 | (new & 0x70) >> 4, octal in the C source.
            m[0o02] = 4;
            m[0o03] = 2;
            m[0o04] = 2;
            m[0o05] = 2;
            m[0o12] = 2;
            m[0o13] = 4;
            m[0o14] = 2;
            m[0o15] = 2;
            m[0o20] = 1;
            m[0o21] = 2;
            m[0o23] = 1;
            m[0o24] = 5;
            m[0o25] = 3;
            m[0o34] = 2;
            m[0o35] = 2;
            m[0o41] = 2;
            m[0o42] = 2;
            m[0o43] = 2;
            m[0o50] = 6;
            m[0o52] = 2;
            m[0o53] = 2;
            m
        };

        let glitch = if new & 0x80 != 0 {
            GLITCH_MAP[usize::from(((old & 0x70) >> 1) | ((new & 0x70) >> 4))]
        } else {
            0
        };
        match glitch {
            // Step, followed by bit 1 &= bit 0 (6: a variant).
            1 | 6 => {
                self.step_lfsr();
                if glitch == 6 {
                    let n = &mut self.s.noise;
                    if (n.narrow && n.lfsr & 0x71 == 0x20) || n.lfsr & 0x71 == 0x61 {
                        n.lfsr &= !0x20;
                    }
                    if n.lfsr & 0x7001 == 0x2000 || n.lfsr & 0x7001 == 0x6001 {
                        n.lfsr &= !0x2000;
                    }
                }
                if self.s.noise.lfsr & 0x3 == 2 {
                    self.s.noise.lfsr &= !2;
                }
            }
            // Step, bitwise AND with the previous value, except for bit 0.
            2 => {
                let prev = self.s.noise.lfsr;
                self.step_lfsr();
                self.s.noise.lfsr &= prev | 1;
            }
            // 5: non-deterministic variant of 3 (falls through into it).
            3 | 5 => {
                if glitch == 5 {
                    if self.s.noise.lfsr & 0x3 == 2 {
                        self.s.noise.lfsr &= if self.s.noise.narrow { !0x4040 } else { !0x4000 };
                    }
                    if self.s.noise.lfsr & 0x19 == 8 {
                        self.s.noise.lfsr &= !8;
                    }
                }
                // No step, bit 0 = bit 1.
                self.s.noise.lfsr &= !1;
                self.s.noise.lfsr |= (self.s.noise.lfsr >> 1) & 1;
                self.update_lfsr();
                self.s.lfsr_stepped_in_narrow = self.s.noise.narrow;
            }
            // Step, bit 1 &= bit 0, LFSR bit -1 &= LFSR bit.
            4 => {
                let prev = self.s.noise.lfsr;
                self.step_lfsr();
                self.s.noise.lfsr &= prev
                    | if self.s.noise.narrow { !0x2022 } else { !0x2002 };
            }
            _ => self.step_lfsr(),
        }
    }

    fn apu_init(&mut self, ctx: &ApuCtx) {
        self.s = State::default();
        self.s.pcm_mask = [0xFF; 2];
        self.s.lf_div = 1;
        self.s.wave.shift = 4;
        // APU glitch: turning the APU on while DIV's bit 4 (5 in double speed)
        // is set skips the first DIV/APU event.
        if ctx.div_counter & if ctx.double_speed { 0x2000 } else { 0x1000 } != 0 {
            self.s.skip_div_event = SkipDivEvent::Skip;
            self.s.div_divider = 1;
        }
        self.s.squares[SQUARE_1].sample_countdown = 0xFFFF;
        self.s.squares[SQUARE_2].sample_countdown = 0xFFFF;
    }

    /// Writes a register (`reg` is the low byte of the 0xFF10..=0xFF3F address).
    pub fn write(&mut self, ctx: &ApuCtx, reg: usize, value: u8) {
        let cgb = self.cgb();
        let rank = self.rank();
        let mut value = value;
        let mut reg = reg;

        if !self.s.global_enable
            && reg != NR52
            && reg < WAV_START
            && (cgb || (reg != NR11 && reg != NR21 && reg != NR31 && reg != NR41))
        {
            return;
        }

        if (WAV_START..=WAV_END).contains(&reg) && self.s.is_active[WAVE] {
            if (!cgb && !self.s.wave.wave_form_just_read) || rank > 15 {
                return;
            }
            reg = WAV_START + usize::from(self.s.wave.current_sample_index / 2);
        }

        match reg {
            // Globals.
            NR50 | NR51 => {
                self.regs[reg] = value;
                // These registers affect the output of all 4 channels (but not
                // the PCM registers): refresh the mixer inputs.
                for i in (0..N_CHANNELS).rev() {
                    let sample = self.s.samples[i];
                    self.s.samples[i] = 0x10; // Invalidate to force the update.
                    self.update_sample(i, sample);
                }
            }
            NR52 => {
                let old_pulse_lengths = [
                    self.s.squares[0].pulse_length,
                    self.s.squares[1].pulse_length,
                    self.s.wave.pulse_length,
                    self.s.noise.pulse_length,
                ];
                if value & 0x80 != 0 && !self.s.global_enable {
                    self.apu_init(ctx);
                    self.s.global_enable = true;
                } else if value & 0x80 == 0 && self.s.global_enable {
                    for i in (0..N_CHANNELS).rev() {
                        self.update_sample(i, 0);
                    }
                    self.s = State::default();
                    self.s.pcm_mask = [0xFF; 2];
                    for r in &mut self.regs[NR10..WAV_START] {
                        *r = 0;
                    }
                    self.s.global_enable = false;
                }

                if !cgb && value & 0x80 != 0 {
                    self.s.squares[0].pulse_length = old_pulse_lengths[0];
                    self.s.squares[1].pulse_length = old_pulse_lengths[1];
                    self.s.wave.pulse_length = old_pulse_lengths[2];
                    self.s.noise.pulse_length = old_pulse_lengths[3];
                }
            }

            // Square channels.
            NR10 => {
                if self.s.square_sweep_calculate_countdown != 0
                    || self.s.square_sweep_calculate_countdown_reload_timer != 0
                {
                    self.nr10_write_glitch(ctx, value);
                }
                let mut old_negate = self.regs[NR10] & 8 != 0;
                self.regs[NR10] = value;
                if rank <= 13 {
                    old_negate = true;
                }
                if u32::from(self.s.shadow_sweep_sample_length)
                    + u32::from(self.s.channel1_completed_addend)
                    + u32::from(old_negate)
                    > 0x7FF
                    && value & 8 == 0
                {
                    self.s.is_active[SQUARE_1] = false;
                    self.update_sample(SQUARE_1, 0);
                }
                self.trigger_sweep_calculation(ctx);
            }

            NR11 | NR21 => {
                let index = if reg == NR21 { SQUARE_2 } else { SQUARE_1 };
                self.s.squares[index].pulse_length = 0x40 - u16::from(value & 0x3F);
                if !self.s.global_enable {
                    value &= 0x3F;
                }
            }

            NR12 | NR22 => {
                let index = if reg == NR22 { SQUARE_2 } else { SQUARE_1 };
                if value & 0xF8 == 0 {
                    // This disables the DAC.
                    self.regs[reg] = value;
                    self.s.is_active[index] = false;
                    self.update_sample(index, 0);
                } else if self.s.is_active[index] {
                    let old = self.regs[reg];
                    let sq = &mut self.s.squares[index];
                    Self::nrx2_glitch(
                        rank,
                        &mut sq.current_volume,
                        value,
                        old,
                        &mut sq.volume_countdown,
                        &mut sq.envelope_clock,
                    );
                    self.update_square_sample(index);
                }
            }

            NR13 | NR23 => {
                let index = if reg == NR23 { SQUARE_2 } else { SQUARE_1 };
                let sq = &mut self.s.squares[index];
                sq.sample_length &= !0xFF;
                sq.sample_length |= u16::from(value);
                if sq.just_reloaded {
                    sq.sample_countdown = (sq.sample_length ^ 0x7FF) * 2 + 1;
                }
            }

            NR14 | NR24 => self.write_square_nrx4(ctx, reg, value),

            // Wave channel.
            NR30 => {
                self.s.wave.enable = value & 0x80 != 0;
                if !self.s.wave.enable {
                    self.s.wave.pulsed = false;
                    if self.s.is_active[WAVE] {
                        // Assumed to also happen on pre-CGB models.
                        if self.s.wave.sample_countdown == 0 && rank <= 15 {
                            self.s.wave.current_sample_byte =
                                self.regs[WAV_START + usize::from(ctx.pc & 0xF)];
                        } else if self.s.wave.wave_form_just_read && rank <= 13 {
                            self.s.wave.current_sample_byte = self.regs[WAV_START + (NR30 & 0xF)];
                        }
                    }
                    self.s.is_active[WAVE] = false;
                    self.update_sample(WAVE, 0);
                }
            }
            NR31 => self.s.wave.pulse_length = 0x100 - u16::from(value),
            NR32 => {
                self.s.wave.shift = [4, 0, 1, 2][usize::from((value >> 5) & 3)];
                if self.s.is_active[WAVE] {
                    self.update_wave_sample();
                }
            }
            NR33 => {
                self.s.wave.sample_length &= !0xFF;
                self.s.wave.sample_length |= u16::from(value);
                if self.s.wave.bugged_read_countdown == 1 {
                    // Just reloaded the countdown.
                    self.s.wave.sample_countdown = self.s.wave.sample_length ^ 0x7FF;
                }
            }
            NR34 => self.write_wave_nr34(value),

            // Noise channel.
            NR41 => self.s.noise.pulse_length = 0x40 - u16::from(value & 0x3F),
            NR42 => {
                if value & 0xF8 == 0 {
                    // This disables the DAC.
                    if self.s.is_active[NOISE] && self.regs[NR43] & 7 != 0 {
                        if self.s.noise.counter_countdown <= 2 {
                            self.s.noise.counter += 1;
                        }
                        self.s.noise_background_counter_active = false;
                    }
                    self.regs[reg] = value;
                    self.s.is_active[NOISE] = false;
                    self.update_sample(NOISE, 0);
                    self.s.noise_counter_active = false;
                } else if self.s.is_active[NOISE] {
                    let old = self.regs[reg];
                    let n = &mut self.s.noise;
                    Self::nrx2_glitch(
                        rank,
                        &mut n.current_volume,
                        value,
                        old,
                        &mut n.volume_countdown,
                        &mut n.envelope_clock,
                    );
                    let volume = if self.s.noise.current_lfsr_sample {
                        self.s.noise.current_volume
                    } else {
                        0
                    };
                    self.update_sample(NOISE, volume);
                }
            }
            NR43 => {
                if self.s.noise.countdown_reloaded {
                    let mut divisor = (value & 0x07) << 2;
                    if divisor == 0 {
                        divisor = 2;
                    }
                    let align = usize::from(self.s.noise.alignment & 3);
                    let table: [u8; 4] = if rank > 13 { [2, 1, 0, 3] } else { [2, 1, 4, 3] };
                    self.s.noise.counter_countdown =
                        divisor + if divisor == 2 { 0 } else { table[align] };
                }
                if rank <= 13 {
                    // CGB <= C (and DMG) have various unemulated quirks when
                    // NR43 is written just as the counter reloads.
                    if self.s.noise.countdown_reloaded {
                        let counter = self.s.noise.counter;
                        let bit = |c: u16, s: u8| (c >> s) & 1 != 0;
                        let old_bit = bit(counter, self.regs[NR43] >> 4);
                        let glitch_bit = bit(counter, 7);
                        let new_bit = bit(counter, value >> 4);
                        if !old_bit && new_bit && glitch_bit {
                            let previous = counter.wrapping_sub(1) & 0x3FFF;
                            let old_bit = bit(previous, self.regs[NR43] >> 4);
                            let glitch_bit = bit(previous, 7);
                            let new_bit = bit(previous, value >> 4);
                            if old_bit && !new_bit && glitch_bit {
                                self.step_lfsr();
                            }
                        }
                    }
                    self.nr43_write(0xFF);
                }
                self.nr43_write(value);
            }
            NR44 => self.write_noise_nr44(ctx, value),

            _ => {}
        }
        self.regs[reg] = value;
    }

    fn write_square_nrx4(&mut self, ctx: &ApuCtx, reg: usize, value: u8) {
        let rank = self.rank();
        let cgb = self.cgb();
        let index = if reg == NR24 { SQUARE_2 } else { SQUARE_1 };
        let nrx2 = if index == SQUARE_1 { NR12 } else { NR22 };
        let was_active = self.s.is_active[index];

        // When the sample length changes right before being updated from >=$700
        // to <$700 the countdown should change to the old length but the
        // current sample should not change; step the index backwards instead.
        if value & 0x80 == 0
            && self.s.is_active[index]
            && self.regs[reg] & 0x7 == 7
            && value & 7 != 7
            && (matches!(self.model, Model::CgbE | Model::CgbD)
                || self.s.squares[index].sample_countdown & 1 != 0)
        {
            let sq = &mut self.s.squares[index];
            if sq.did_tick && sq.sample_countdown >> 1 == (sq.sample_length ^ 0x7FF) {
                sq.current_sample_index = sq.current_sample_index.wrapping_sub(1) & 7;
                sq.sample_surpressed = false;
            }
        }

        let old_sample_length = self.s.squares[index].sample_length;
        {
            let sq = &mut self.s.squares[index];
            sq.sample_length &= 0xFF;
            sq.sample_length |= u16::from(value & 7) << 8;
            if sq.just_reloaded {
                sq.sample_countdown = (sq.sample_length ^ 0x7FF) * 2 + 1;
            }
        }
        if value & 0x80 != 0 {
            // The sample index is unchanged when restarting channels 1 or 2;
            // only turning the APU off resets it.
            {
                let sq = &mut self.s.squares[index];
                sq.envelope_clock.locked = false;
                sq.envelope_clock.clock = false;
                sq.did_tick = false;
            }
            let mut force_unsurpressed = false;
            let lf_div = i32::from(self.s.lf_div);
            if !self.s.is_active[index] {
                if matches!(self.model, Model::CgbE | Model::CgbD) {
                    let sq = &mut self.s.squares[index];
                    if value & 4 == 0
                        && (sq.sample_countdown.wrapping_sub(u16::from(sq.delay)) / 2) & 0x400 == 0
                    {
                        sq.current_sample_index = (sq.current_sample_index + 1) & 7;
                        force_unsurpressed = true;
                    }
                }
                let delay = 6 + lf_div * if rank < 14 && ctx.double_speed { 1 } else { -1 };
                let sq = &mut self.s.squares[index];
                sq.delay = delay as u8;
                sq.sample_countdown = (sq.sample_length ^ 0x7FF) * 2 + u16::from(sq.delay);
            } else {
                let mut extra_delay = 0u8;
                if matches!(self.model, Model::CgbE | Model::CgbD) {
                    let sq = &mut self.s.squares[index];
                    if !sq.just_reloaded
                        && value & 4 == 0
                        && (sq
                            .sample_countdown
                            .wrapping_sub(1)
                            .wrapping_sub(u16::from(sq.delay))
                            / 2)
                            & 0x400
                            == 0
                    {
                        sq.current_sample_index = (sq.current_sample_index + 1) & 7;
                        sq.sample_surpressed = false;
                    } else if sq.sample_length == 0x7FF
                        && old_sample_length != 0x7FF
                        && sq.sample_surpressed
                    {
                        extra_delay += 2;
                    }
                }
                // Timing quirk: if already active, the sound starts 2 (2 MHz)
                // ticks earlier.
                let sq = &mut self.s.squares[index];
                sq.delay = 4u8.wrapping_sub(self.s.lf_div).wrapping_add(extra_delay);
                sq.sample_countdown = (sq.sample_length ^ 0x7FF) * 2 + u16::from(sq.delay);
            }
            self.s.squares[index].current_volume = self.regs[nrx2] >> 4;
            // The volume change caused by sound start takes effect instantly
            // (i.e. on the previously started sound).
            if self.s.is_active[index] {
                self.update_square_sample(index);
            }

            self.s.squares[index].volume_countdown = self.regs[nrx2] & 7;

            if self.regs[nrx2] & 0xF8 != 0 && !self.s.is_active[index] {
                self.s.is_active[index] = true;
                self.update_sample(index, 0);
                self.s.squares[index].sample_surpressed = !force_unsurpressed;
            }
            if self.s.squares[index].pulse_length == 0 {
                self.s.squares[index].pulse_length = 0x40;
                self.s.squares[index].length_enabled = false;
            }

            if index == SQUARE_1 {
                self.s.square_sweep_instant_calculation_done = false;
                self.s.shadow_sweep_sample_length = 0;
                self.s.channel1_completed_addend = 0;
                if self.regs[NR10] & 7 != 0 {
                    // APU bug: if the shift is nonzero the overflow check also
                    // happens on trigger.
                    self.s.square_sweep_calculate_countdown = self.regs[NR10] & 0x7;
                    if ((self.s.lf_div != 0) ^ !ctx.double_speed) && rank <= 13 {
                        self.s.square_sweep_calculate_countdown_reload_timer = 3;
                    } else {
                        self.s.square_sweep_calculate_countdown_reload_timer = 2;
                    }
                    self.s.unshifted_sweep = false;
                    if !was_active {
                        self.s.square_sweep_calculate_countdown_reload_timer += 1;
                    }
                    self.s.sweep_length_addend = self.s.squares[SQUARE_1].sample_length;
                    self.s.sweep_length_addend >>= self.regs[NR10] & 7;
                } else {
                    self.s.sweep_length_addend = 0;
                }
                self.s.channel_1_restart_hold = 2
                    - self.s.lf_div
                    + u8::from(cgb && self.model != Model::CgbD) * 2;
                self.s.square_sweep_countdown = ((self.regs[NR10] >> 4) & 7) ^ 7;
            }
        }

        // APU glitch: enabling the length while the DIV divider's LSB is 1
        // ticks the length once.
        if (value & 0x40 != 0 || (cgb && rank <= 12))
            && !self.s.squares[index].length_enabled
            && self.s.div_divider & 1 != 0
            && self.s.squares[index].pulse_length != 0
        {
            self.s.squares[index].pulse_length -= 1;
            if self.s.squares[index].pulse_length == 0 {
                if value & 0x80 != 0 {
                    self.s.squares[index].pulse_length = 0x3F;
                } else {
                    self.s.is_active[index] = false;
                    self.update_sample(index, 0);
                }
            }
        }
        self.s.squares[index].length_enabled = value & 0x40 != 0;
    }

    fn write_wave_nr34(&mut self, value: u8) {
        let cgb = self.cgb();
        self.s.wave.sample_length &= 0xFF;
        self.s.wave.sample_length |= u16::from(value & 7) << 8;
        if value & 0x80 != 0 {
            self.s.wave.pulsed = true;
            // DMG bug: wave RAM gets corrupted if the channel is retriggered 1
            // cycle before the APU reads from it.
            if !cgb && self.s.is_active[WAVE] && self.s.wave.sample_countdown == 0 {
                let offset = usize::from(((self.s.wave.current_sample_index + 1) >> 1) & 0xF);
                // The most common DMG-B behaviour (what blargg's tests expect);
                // the MGB emulates a deterministic Game Boy Light.
                if offset < 4 && self.model != Model::Mgb {
                    self.regs[WAV_START] = self.regs[WAV_START + offset];
                } else {
                    let base = WAV_START + (offset & !3);
                    for i in 0..4 {
                        self.regs[WAV_START + i] = self.regs[base + i];
                    }
                }
            }
            self.s.wave.current_sample_index = 0;
            if self.s.is_active[WAVE] && self.s.wave.sample_countdown == 0 {
                self.s.wave.current_sample_byte = self.regs[WAV_START];
            }
            if self.s.wave.enable {
                self.s.is_active[WAVE] = true;
                let value = (self.s.wave.current_sample_byte >> 4) >> self.s.wave.shift;
                self.update_sample(WAVE, value);
            }
            self.s.wave.sample_countdown = (self.s.wave.sample_length ^ 0x7FF) + 3;
            if self.s.wave.pulse_length == 0 {
                self.s.wave.pulse_length = 0x100;
                self.s.wave.length_enabled = false;
            }
            // The sample is not changed just yet (verified on hardware).
        }

        // APU glitch: enabling the length while the DIV divider's LSB is 1
        // ticks the length once.
        if (value & 0x40 != 0 || (cgb && self.rank() <= 12))
            && !self.s.wave.length_enabled
            && self.s.div_divider & 1 != 0
            && self.s.wave.pulse_length != 0
        {
            self.s.wave.pulse_length -= 1;
            if self.s.wave.pulse_length == 0 {
                if value & 0x80 != 0 {
                    self.s.wave.pulse_length = 0xFF;
                } else {
                    self.s.is_active[WAVE] = false;
                    self.update_sample(WAVE, 0);
                }
            }
        }
        self.s.wave.length_enabled = value & 0x40 != 0;
    }

    fn write_noise_nr44(&mut self, ctx: &ApuCtx, value: u8) {
        let cgb = self.cgb();
        if value & 0x80 != 0 {
            self.s.noise.envelope_clock.locked = false;
            self.s.noise.envelope_clock.clock = false;
            if !cgb && self.s.noise.alignment & 3 != 0 {
                self.s.noise.dmg_delayed_start = 6;
            } else {
                self.s.noise.lfsr = 0;
                self.prepare_noise_start(ctx);

                self.s.noise.current_volume = self.regs[NR42] >> 4;
                self.s.noise.current_lfsr_sample = false;
                self.s.noise.volume_countdown = self.regs[NR42] & 7;
                self.s.noise.did_step_counter = self.s.noise.alignment & 3 == 2;

                if self.regs[NR42] & 0xF8 != 0 {
                    self.s.is_active[NOISE] = true;
                    self.update_sample(NOISE, 0);
                }

                if self.s.noise.pulse_length == 0 {
                    self.s.noise.pulse_length = 0x40;
                    self.s.noise.length_enabled = false;
                }
            }
        }

        // APU glitch: enabling the length while the DIV divider's LSB is 1
        // ticks the length once.
        if value & 0x40 != 0
            && !self.s.noise.length_enabled
            && self.s.div_divider & 1 != 0
            && self.s.noise.pulse_length != 0
        {
            self.s.noise.pulse_length -= 1;
            if self.s.noise.pulse_length == 0 {
                if value & 0x80 != 0 {
                    self.s.noise.pulse_length = 0x3F;
                } else {
                    self.s.is_active[NOISE] = false;
                    self.update_sample(NOISE, 0);
                }
            }
        }
        self.s.noise.length_enabled = value & 0x40 != 0;
    }
}
