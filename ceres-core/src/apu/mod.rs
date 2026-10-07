//! Audio processing unit.
//!
//! Modelled after SameBoy's `apu.c`: the channel state machines with their
//! revision specific glitches, a frame sequencer driven by DIV and a
//! cycle-averaged mixer for the audio output.
//!
//! The APU is clocked in 2 MHz ticks: two per M-cycle in single speed, one in
//! double speed.

#![expect(
    clippy::else_if_without_else,
    reason = "Most hardware quirks are chains of special cases without a general one"
)]
#![expect(
    clippy::verbose_bit_mask,
    reason = "The register bit tests read like the hardware documentation"
)]
#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "The counters are bounded by the hardware: a tick count is a handful of cycles"
)]

mod envelope;
mod high_pass_filter;
mod length;
mod mixer;
mod noise;
mod post_boot;
mod revision;
mod square;
mod sweep;
mod wave;

pub use post_boot::PostBoot;

use {
    crate::Model, mixer::ChannelOutput, mixer::Mixer, noise::Noise, revision::Revision,
    square::Square, sweep::Sweep, wave::Wave,
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

/// What the channels see of the rest of the APU and the machine.
#[derive(Clone, Copy)]
struct Ctx {
    rev: Revision,
    double_speed: bool,
    during_div_write: bool,
    address_bus: u16,
    pc: u16,
    nr50: u8,
    nr51: u8,
    div_divider: u8,
    lf_div: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SkipDivEvent {
    Inactive,
    Skipped,
    Skip,
}

pub struct Apu<A: AudioCallback> {
    audio_callback: A,
    mixer: Mixer,
    rev: Revision,
    /// NR52 bit 7: the APU is powered.
    enabled: bool,
    nr50: u8,
    nr51: u8,
    /// The frame sequencer: counts the DIV events, which clock the
    /// envelopes, the length timers and the sweep.
    div_divider: u8,
    skip_div_event: SkipDivEvent,
    /// The CGB-D and E tick the envelopes a little after the DIV event in
    /// double speed.
    pending_envelope_tick: bool,
    /// The phase of the 1 MHz clock (the squares and the sweep run on it).
    lf_div: u8,
    squares: [Square; 2],
    sweep: Sweep,
    wave: Wave,
    noise: Noise,
}

impl<A: AudioCallback> Apu<A> {
    pub fn new(sample_rate: i32, audio_callback: A) -> Self {
        Self {
            audio_callback,
            mixer: Mixer::new(sample_rate),
            rev: Revision::new(Model::default()),
            enabled: false,
            nr50: 0,
            nr51: 0,
            div_divider: 0,
            skip_div_event: SkipDivEvent::Inactive,
            pending_envelope_tick: false,
            lf_div: 0,
            squares: [Square::new(SQUARE_1), Square::new(SQUARE_2)],
            sweep: Sweep::new(),
            wave: Wave::new(),
            noise: Noise::new(),
        }
    }

    pub const fn set_model(&mut self, model: Model) {
        self.rev = Revision::new(model);
    }

    pub fn set_sample_rate(&mut self, sample_rate: i32) {
        self.mixer.set_sample_rate(sample_rate);
    }

    /// Resets everything but the wave RAM.
    pub fn reset(&mut self) {
        self.enabled = false;
        self.nr50 = 0;
        self.nr51 = 0;
        self.div_divider = 0;
        self.skip_div_event = SkipDivEvent::Inactive;
        self.pending_envelope_tick = false;
        self.lf_div = 0;
        self.squares = [Square::new(SQUARE_1), Square::new(SQUARE_2)];
        self.sweep = Sweep::new();
        self.wave.reset();
        self.noise = Noise::new();
    }

    const fn ctx(&self, ctx: &ApuCtx) -> Ctx {
        Ctx {
            rev: self.rev,
            double_speed: ctx.double_speed,
            during_div_write: ctx.during_div_write,
            address_bus: ctx.address_bus,
            pc: ctx.pc,
            nr50: self.nr50,
            nr51: self.nr51,
            div_divider: self.div_divider,
            lf_div: self.lf_div,
        }
    }

    // -- channels -------------------------------------------------------------

    const fn output(&self, index: usize) -> &ChannelOutput {
        match index {
            SQUARE_1 | SQUARE_2 => self.squares[index].out(),
            WAVE => self.wave.out(),
            _ => self.noise.out(),
        }
    }

    const fn output_mut(&mut self, index: usize) -> &mut ChannelOutput {
        match index {
            SQUARE_1 | SQUARE_2 => self.squares[index].out_mut(),
            WAVE => self.wave.out_mut(),
            _ => self.noise.out_mut(),
        }
    }

    fn update_sample(&mut self, index: usize, value: u8, c: &Ctx) {
        match index {
            SQUARE_1 | SQUARE_2 => self.squares[index].update_sample(value, c),
            WAVE => self.wave.update_sample(value, c),
            _ => self.noise.update_sample(value, c),
        }
    }

    // -- PCM registers ----------------------------------------------------------

    /// Every machine step starts with an unmasked PCM (SameBoy's "sort of
    /// hacky, but too many cross-component interactions to do it right").
    pub const fn reset_pcm_mask(&mut self) {
        self.squares[SQUARE_1].out_mut().pcm_mask = 0xF;
        self.squares[SQUARE_2].out_mut().pcm_mask = 0xF;
        self.wave.out_mut().pcm_mask = 0xF;
        self.noise.out_mut().pcm_mask = 0xF;
    }

    #[must_use]
    pub fn pcm12(&self) -> u8 {
        let masked = self.rev <= Revision::CgbC;
        (self.output(SQUARE_2).pcm(masked) << 4) | self.output(SQUARE_1).pcm(masked)
    }

    #[must_use]
    pub fn pcm34(&self) -> u8 {
        let masked = self.rev <= Revision::CgbC;
        (self.output(NOISE).pcm(masked) << 4) | self.output(WAVE).pcm(masked)
    }

    // -- frame sequencer ------------------------------------------------------

    fn tick_envelopes(&mut self, c: &Ctx) {
        for sq in &mut self.squares {
            sq.tick_envelope(c);
        }
        self.noise.tick_envelope(c);
    }

    pub fn delayed_envelope_tick(&mut self, ctx: &ApuCtx) {
        self.pending_envelope_tick = false;
        if !self.enabled {
            return;
        }
        self.reset_pcm_mask();
        let c = self.ctx(ctx);
        self.tick_envelopes(&c);
    }

    #[must_use]
    pub const fn pending_envelope_tick(&self) -> bool {
        self.pending_envelope_tick
    }

    /// A falling edge of DIV bit 4 (5 in double speed).
    pub fn div_event(&mut self, ctx: &ApuCtx) {
        self.reset_pcm_mask();
        if !self.enabled {
            return;
        }
        match self.skip_div_event {
            SkipDivEvent::Skip => {
                self.skip_div_event = SkipDivEvent::Skipped;
                return;
            }
            SkipDivEvent::Skipped => self.skip_div_event = SkipDivEvent::Inactive,
            SkipDivEvent::Inactive => self.div_divider = self.div_divider.wrapping_add(1),
        }
        let c = self.ctx(ctx);

        if self.div_divider & 7 == 7 {
            for sq in &mut self.squares {
                sq.step_envelope_countdown();
            }
            self.noise.step_envelope_countdown();
        }

        if ctx.double_speed && self.rev.is_cgb_de() {
            self.pending_envelope_tick = true;
        } else {
            self.tick_envelopes(&c);
        }

        if self.div_divider & 1 == 1 {
            for sq in &mut self.squares {
                sq.tick_length(&c);
            }
            self.wave.tick_length(&c);
            self.noise.tick_length(&c);
        }

        if self.div_divider & 3 == 3 {
            self.sweep.div_event(&mut self.squares[SQUARE_1], &c);
        }
    }

    /// A rising edge of DIV bit 4 (5 in double speed): the envelopes whose
    /// countdown expired raise their clock.
    pub fn div_secondary_event(&mut self) {
        self.reset_pcm_mask();
        if !self.enabled {
            return;
        }
        for sq in &mut self.squares {
            sq.reload_envelope();
        }
        self.noise.reload_envelope();
    }

    // -- running --------------------------------------------------------------

    /// Runs the APU for `ticks` ticks and feeds the mixer.
    pub fn tick(&mut self, ctx: &ApuCtx, ticks: u32) {
        self.run(ctx, ticks);
        let levels = [
            self.output(SQUARE_1).level,
            self.output(SQUARE_2).level,
            self.output(WAVE).level,
            self.output(NOISE).level,
        ];
        let dacs = || {
            (!self.rev.is_agb()).then(|| {
                [
                    self.squares[SQUARE_1].dac_enabled(),
                    self.squares[SQUARE_2].dac_enabled(),
                    self.wave.dac_enabled(),
                    self.noise.dac_enabled(),
                ]
            })
        };
        self.mixer.mix(ticks, levels, dacs, &self.audio_callback);
    }

    fn run(&mut self, ctx: &ApuCtx, mut cycles: u32) {
        if cycles == 0 {
            return;
        }
        if self.wave.has_bugged_read() {
            self.wave.run_bugged_read(cycles, &self.ctx(ctx));
        }
        if ctx.stopped && !self.rev.is_cgb() {
            return;
        }

        let mut start_noise = false;
        if let Some(delay) = self.noise.delayed_start(cycles) {
            if delay == cycles {
                start_noise = true;
            } else {
                // Split it into two.
                cycles -= delay;
                self.run(ctx, delay);
            }
        }

        // To align the square signal to 1 MHz.
        self.lf_div ^= (cycles & 1) as u8;
        self.noise.advance_alignment(cycles);

        let c = self.ctx(ctx);
        self.sweep.run(cycles, &mut self.squares[SQUARE_1], &c);
        for sq in &mut self.squares {
            sq.run(cycles, &c);
        }
        self.wave.run(cycles, &c);
        self.noise.run(cycles, &c);

        if start_noise {
            let value = if self.noise.length_enabled() {
                0xC0
            } else {
                0x80
            };
            self.write(ctx, NR44, value);
        }
    }

    // -- register access ----------------------------------------------------

    /// Reads a register (`reg` is the low byte of the 0xFF10..=0xFF3F address).
    #[must_use]
    pub fn read(&self, reg: usize) -> u8 {
        match reg {
            NR10 => self.sweep.read_nr10(),
            NR11 | NR21 => self.squares[usize::from(reg == NR21)].read_nrx1(),
            NR12 | NR22 => self.squares[usize::from(reg == NR22)].read_nrx2(),
            NR14 | NR24 => self.squares[usize::from(reg == NR24)].read_nrx4(),
            NR30 => self.wave.read_nr30(),
            NR32 => self.wave.read_nr32(),
            NR34 => self.wave.read_nr34(),
            NR42 => self.noise.read_nr42(),
            NR43 => self.noise.read_nr43(),
            NR44 => self.noise.read_nr44(),
            NR50 => self.nr50,
            NR51 => self.nr51,
            NR52 => {
                let mut value = if self.enabled { 0xF0 } else { 0x70 };
                for i in 0..N_CHANNELS {
                    if self.output(i).active {
                        value |= 1 << i;
                    }
                }
                value
            }
            WAV_START..=WAV_END => self.wave.read_ram(reg - WAV_START, self.rev),
            _ => 0xFF,
        }
    }

    /// Writes a register (`reg` is the low byte of the 0xFF10..=0xFF3F address).
    pub fn write(&mut self, ctx: &ApuCtx, reg: usize, value: u8) {
        // Only the wave RAM, and the length timers on the DMG, can be written
        // with the APU off.
        if !self.enabled
            && reg != NR52
            && reg < WAV_START
            && (self.rev.is_cgb() || !matches!(reg, NR11 | NR21 | NR31 | NR41))
        {
            return;
        }

        let c = self.ctx(ctx);
        let square = usize::from(matches!(reg, NR21..=NR24));
        match reg {
            NR50 | NR51 => self.write_nr50_nr51(ctx, reg, value),
            NR52 => self.write_nr52(ctx, value),

            NR10 => self
                .sweep
                .write_nr10(value, &mut self.squares[SQUARE_1], &c),
            NR11 | NR21 => {
                let value = if self.enabled { value } else { value & 0x3F };
                self.squares[square].write_nrx1(value);
            }
            NR12 | NR22 => self.squares[square].write_nrx2(value, &c),
            NR13 | NR23 => self.squares[square].write_nrx3(value),
            NR14 | NR24 => {
                let was_active = self.output(square).active;
                self.squares[square].write_nrx4(value, &c);
                if square == SQUARE_1 && value & 0x80 != 0 {
                    self.sweep.trigger(&self.squares[SQUARE_1], was_active, &c);
                }
            }

            NR30 => self.wave.write_nr30(value, &c),
            NR31 => self.wave.write_nr31(value),
            NR32 => self.wave.write_nr32(value, &c),
            NR33 => self.wave.write_nr33(value),
            NR34 => self.wave.write_nr34(value, &c),

            NR41 => self.noise.write_nr41(value),
            NR42 => self.noise.write_nr42(value, &c),
            NR43 => self.noise.write_nr43(value, &c),
            NR44 => self.noise.write_nr44(value, &c),

            WAV_START..=WAV_END => self.wave.write_ram(reg - WAV_START, value, self.rev),
            _ => {}
        }
    }

    fn write_nr50_nr51(&mut self, ctx: &ApuCtx, reg: usize, value: u8) {
        if reg == NR50 {
            self.nr50 = value;
        } else {
            self.nr51 = value;
        }
        // These registers affect the output of all 4 channels (but not the
        // PCM registers): refresh the mixer inputs.
        let c = self.ctx(ctx);
        for i in (0..N_CHANNELS).rev() {
            let sample = self.output(i).sample;
            self.output_mut(i).sample = 0x10; // Invalidate to force the update.
            self.update_sample(i, sample, &c);
        }
    }

    fn write_nr52(&mut self, ctx: &ApuCtx, value: u8) {
        let lengths = [
            self.squares[SQUARE_1].length_counter(),
            self.squares[SQUARE_2].length_counter(),
            self.wave.length_counter(),
            self.noise.length_counter(),
        ];
        if value & 0x80 != 0 && !self.enabled {
            self.power_on(ctx);
        } else if value & 0x80 == 0 && self.enabled {
            let c = self.ctx(ctx);
            for i in (0..N_CHANNELS).rev() {
                self.update_sample(i, 0, &c);
            }
            self.clear();
            self.nr50 = 0;
            self.nr51 = 0;
        }

        // The DMG keeps the length timers.
        if !self.rev.is_cgb() && value & 0x80 != 0 {
            self.squares[SQUARE_1].set_length_counter(lengths[SQUARE_1]);
            self.squares[SQUARE_2].set_length_counter(lengths[SQUARE_2]);
            self.wave.set_length_counter(lengths[WAVE]);
            self.noise.set_length_counter(lengths[NOISE]);
        }
    }

    /// What powering the APU off clears.
    fn clear(&mut self) {
        self.enabled = false;
        self.div_divider = 0;
        self.skip_div_event = SkipDivEvent::Inactive;
        self.pending_envelope_tick = false;
        self.lf_div = 0;
        for sq in &mut self.squares {
            sq.power_off();
        }
        self.sweep = Sweep::new();
        self.wave.power_off();
        self.noise.power_off();
    }

    fn power_on(&mut self, ctx: &ApuCtx) {
        self.clear();
        self.enabled = true;
        self.lf_div = 1;
        // APU glitch: turning the APU on while DIV's bit 4 (5 in double speed)
        // is set skips the first DIV/APU event. The write takes effect an
        // M-cycle after the one it started in (measured with Gambatte's tests).
        let div_counter = ctx.div_counter.wrapping_add(4);
        if div_counter & if ctx.double_speed { 0x2000 } else { 0x1000 } != 0 {
            self.skip_div_event = SkipDivEvent::Skip;
            self.div_divider = 1;
        }
        for sq in &mut self.squares {
            sq.power_on();
        }
    }
}
