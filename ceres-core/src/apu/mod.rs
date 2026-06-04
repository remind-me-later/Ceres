mod blip;
mod envelope;
mod high_pass_filter;
mod length_timer;
mod master_volume;
mod noise;
mod period_counter;
mod square;
mod sweep;
mod wave;

use {
    crate::{
        apu::{blip::Blip, high_pass_filter::HighPassFilter, master_volume::MasterVolume},
        timing::DOTS_PER_SEC,
    },
    length_timer::LengthTimer,
    noise::Noise,
    period_counter::PeriodCounter,
    square::Square,
    sweep::{Sweep, SweepTrait},
    wave::Wave,
};

pub type Sample = i16;

pub trait AudioCallback {
    fn audio_sample(&self, l: Sample, r: Sample);
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum PeriodHalf {
    #[default]
    First,
    Second,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum SkipDivEvent {
    #[default]
    Inactive,
    Skip,
    Skipped,
}

pub struct Apu<A: AudioCallback> {
    audio_callback: A,
    blips: [Blip; 4],
    ch1: Square<Sweep>,
    ch2: Square<()>,
    ch3: Wave,
    ch4: Noise,
    div_divider: u8,
    enabled: bool,
    ext_sample_period: i32,
    hpf: HighPassFilter,
    master_volume: MasterVolume,
    nr51: u8,
    render_timer: i32,
    skip_div_event: SkipDivEvent,
}

impl<A: AudioCallback> Apu<A> {
    pub fn new(sample_rate: i32, audio_callback: A) -> Self {
        Self {
            ext_sample_period: Self::sample_period_from_rate(sample_rate),
            audio_callback,
            blips: Default::default(),
            nr51: 0,
            enabled: false,
            master_volume: MasterVolume::default(),
            ch1: Square::default(),
            ch2: Square::default(),
            ch3: Wave::default(),
            ch4: Noise::default(),
            div_divider: 0,
            render_timer: 0,
            hpf: HighPassFilter::new(sample_rate),
            skip_div_event: SkipDivEvent::default(),
        }
    }

    pub fn reset(&mut self) {
        self.enabled = false;
        self.master_volume = MasterVolume::default();
        self.blips = Default::default();

        // reset registers
        self.ch1 = Square::default();
        self.ch2 = Square::default();
        self.ch3.reset();
        self.ch4 = Noise::default();
        self.nr51 = 0;

        // reset div divider
        self.div_divider = 0;

        // reset render timer
        self.render_timer = 0;

        self.skip_div_event = SkipDivEvent::default();
    }

    fn calculate_channel_output(&self, ch: usize) -> (i32, i32) {
        let out = match ch {
            0 => self.ch1.output() * u8::from(self.ch1.is_truly_enabled()),
            1 => self.ch2.output() * u8::from(self.ch2.is_truly_enabled()),
            2 => self.ch3.output() * u8::from(self.ch3.is_truly_enabled()),
            3 => self.ch4.output() * u8::from(self.ch4.is_truly_enabled()),
            _ => 0,
        };

        let right_on = i32::from(self.nr51 & (1 << ch) != 0);
        let left_on = i32::from(self.nr51 & (0x10 << ch) != 0);

        let out_i32 = i32::from(out);
        (out_i32 * left_on, out_i32 * right_on)
    }

    pub fn run(&mut self, dots: i32) {
        if self.enabled {
            let old_1 = self.calculate_channel_output(0);
            let res_1 = self.ch1.step_sample(dots);
            let new_1 = self.calculate_channel_output(0);
            if old_1 != new_1 {
                let t = self.render_timer + dots + res_1.unwrap_or(0);
                let phase =
                    (i64::from(t) * (blip::PHASES as i64)) / i64::from(self.ext_sample_period);
                self.blips[0].update(new_1.0, new_1.1, phase as usize);
            }

            let old_2 = self.calculate_channel_output(1);
            let res_2 = self.ch2.step_sample(dots);
            let new_2 = self.calculate_channel_output(1);
            if old_2 != new_2 {
                let t = self.render_timer + dots + res_2.unwrap_or(0);
                let phase =
                    (i64::from(t) * (blip::PHASES as i64)) / i64::from(self.ext_sample_period);
                self.blips[1].update(new_2.0, new_2.1, phase as usize);
            }

            let old_3 = self.calculate_channel_output(2);
            let res_3 = self.ch3.step_sample(dots);
            let new_3 = self.calculate_channel_output(2);
            if old_3 != new_3 {
                let t = self.render_timer + dots + res_3.unwrap_or(0);
                let phase =
                    (i64::from(t) * (blip::PHASES as i64)) / i64::from(self.ext_sample_period);
                self.blips[2].update(new_3.0, new_3.1, phase as usize);
            }

            let old_4 = self.calculate_channel_output(3);
            let res_4 = self.ch4.step_sample(dots);
            let new_4 = self.calculate_channel_output(3);
            if old_4 != new_4 {
                let t = self.render_timer + dots + res_4.unwrap_or(0);
                let phase =
                    (i64::from(t) * (blip::PHASES as i64)) / i64::from(self.ext_sample_period);
                self.blips[3].update(new_4.0, new_4.1, phase as usize);
            }
        }

        self.render_timer += dots;
        while self.render_timer >= self.ext_sample_period {
            self.render_timer -= self.ext_sample_period;

            // Read blips
            let (l1, r1) = self.blips[0].read();
            let (l2, r2) = self.blips[1].read();
            let (l3, r3) = self.blips[2].read();
            let (l4, r4) = self.blips[3].read();

            // Sum and scale
            // The accumulated values are roughly (Sample * ONE).
            // We divide by ONE.
            let l_sum = (l1 + l2 + l3 + l4) / blip::ONE;
            let r_sum = (r1 + r2 + r3 + r4) / blip::ONE;

            // transform to i16 sample
            // The formula from original Ceres:
            // let l = (0xF - i16::from(l) * 2) * i16::from(apu.master_volume.left_volume() + 1);
            // Note: `l` in original was 0..60. `l_sum` here is also 0..60 range.

            let l = (0xF - l_sum as i16 * 2) * i16::from(self.master_volume.left_volume() + 1);
            let r = (0xF - r_sum as i16 * 2) * i16::from(self.master_volume.right_volume() + 1);

            // amplify
            let l = l * 32;
            let r = r * 32;

            let (l, r) = if self.ch1.is_enabled()
                || self.ch2.is_enabled()
                || self.ch3.is_enabled()
                || self.ch4.is_enabled()
            {
                self.hpf.high_pass(l, r)
            } else {
                (l, r)
            };

            self.audio_callback.audio_sample(l, r);
        }
    }

    const fn sample_period_from_rate(sample_rate: i32) -> i32 {
        DOTS_PER_SEC / sample_rate
    }

    pub fn set_sample_rate(&mut self, sample_rate: i32) {
        self.ext_sample_period = Self::sample_period_from_rate(sample_rate);
        self.hpf.set_sample_rate(sample_rate);
    }

    /// Reset the APU's internal DIV phase counter. Called when the CPU
    /// writes to the DIV register (FF04) — gambatte's sound_unit
    /// resynchronises its cycle counter on every DIV write so the next
    /// APU tick lands on a known phase.
    pub fn reset_div_phase(&mut self) {
        self.div_divider = 0;
        self.skip_div_event = SkipDivEvent::default();
    }

    pub fn step_div_apu(&mut self) {
        const fn set_period_half<C1: AudioCallback>(apu: &mut Apu<C1>, p_half: PeriodHalf) {
            apu.ch1.set_period_half(p_half);
            apu.ch2.set_period_half(p_half);
            apu.ch3.set_period_half(p_half);
            apu.ch4.set_period_half(p_half);
        }

        if !self.enabled {
            return;
        }

        if self.skip_div_event == SkipDivEvent::Skip {
            self.skip_div_event = SkipDivEvent::Skipped;
            return;
        }

        if self.skip_div_event == SkipDivEvent::Skipped {
            self.skip_div_event = SkipDivEvent::Inactive;
        }

        self.div_divider = (self.div_divider + 1) & 7;

        match self.div_divider {
            0 | 4 => {
                self.ch1.step_length_timer();
                self.ch2.step_length_timer();
                self.ch3.step_length_timer();
                self.ch4.step_length_timer();
                set_period_half(self, PeriodHalf::First);
            }
            2 | 6 => {
                self.ch1.step_length_timer();
                self.ch2.step_length_timer();
                self.ch3.step_length_timer();
                self.ch4.step_length_timer();
                set_period_half(self, PeriodHalf::First);
                self.ch1.step_sweep();
            }
            7 => {
                self.ch1.step_envelope();
                self.ch2.step_envelope();
                self.ch4.step_envelope();
                set_period_half(self, PeriodHalf::Second);
            }
            _ => {
                set_period_half(self, PeriodHalf::Second);
            }
        }
    }
}

// IO
impl<A: AudioCallback> Apu<A> {
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn pcm12(&self) -> u8 {
        self.ch1.output() | (self.ch2.output() << 4)
    }

    pub const fn pcm34(&self) -> u8 {
        self.ch3.output() | (self.ch4.output() << 4)
    }

    pub fn read_nr10(&self) -> u8 {
        self.ch1.read_nrx0()
    }

    pub const fn read_nr11(&self) -> u8 {
        self.ch1.read_nrx1()
    }

    pub fn read_nr12(&self) -> u8 {
        self.ch1.read_nrx2()
    }

    pub const fn read_nr14(&self) -> u8 {
        self.ch1.read_nrx4()
    }

    pub const fn read_nr21(&self) -> u8 {
        self.ch2.read_nrx1()
    }

    pub fn read_nr22(&self) -> u8 {
        self.ch2.read_nrx2()
    }

    pub const fn read_nr24(&self) -> u8 {
        self.ch2.read_nrx4()
    }

    pub const fn read_nr30(&self) -> u8 {
        self.ch3.read_nr30()
    }

    pub const fn read_nr32(&self) -> u8 {
        self.ch3.read_nr32()
    }

    pub const fn read_nr34(&self) -> u8 {
        self.ch3.read_nr34()
    }

    pub fn read_nr42(&self) -> u8 {
        self.ch4.read_nr42()
    }

    pub const fn read_nr43(&self) -> u8 {
        self.ch4.read_nr43()
    }

    pub const fn read_nr44(&self) -> u8 {
        self.ch4.read_nr44()
    }

    #[must_use]
    pub fn read_nr50(&self) -> u8 {
        self.master_volume.read_nr50()
    }

    #[must_use]
    pub const fn read_nr51(&self) -> u8 {
        self.nr51
    }

    #[must_use]
    pub const fn read_nr52(&self) -> u8 {
        // println!("read nr52, ch2: {}", self.ch1.on());
        // println!(
        //     "Ch1 length timer: {}, Max: {}",
        //     self.ch1.length_timer.length, 0x3f
        // );
        // println!(
        //     "Ch1 sweep timer: {}, shadow pace: {}",
        //     self.ch1.period_counter.sweep.timer, self.ch1.period_counter.sweep.shadow_pace
        // );

        ((self.enabled as u8) << 7)
            | 0x70
            | ((self.ch4.is_enabled() as u8) << 3)
            | ((self.ch3.is_enabled() as u8) << 2)
            | ((self.ch2.is_enabled() as u8) << 1)
            | (self.ch1.is_enabled() as u8)
    }

    pub const fn read_wave_ram(&self, addr: u8, is_cgb: bool) -> u8 {
        self.ch3.read_wave_ram(addr, is_cgb)
    }

    fn update_ch1<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Square<Sweep>),
    {
        let old = self.calculate_channel_output(0);
        f(&mut self.ch1);
        let new = self.calculate_channel_output(0);

        if old != new {
            let phase = (i64::from(self.render_timer) * (blip::PHASES as i64))
                / i64::from(self.ext_sample_period);
            self.blips[0].update(new.0, new.1, phase as usize);
        }
    }

    fn update_ch2<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Square<()>),
    {
        let old = self.calculate_channel_output(1);
        f(&mut self.ch2);
        let new = self.calculate_channel_output(1);

        if old != new {
            let phase = (i64::from(self.render_timer) * (blip::PHASES as i64))
                / i64::from(self.ext_sample_period);
            self.blips[1].update(new.0, new.1, phase as usize);
        }
    }

    fn update_ch3<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Wave),
    {
        let old = self.calculate_channel_output(2);
        f(&mut self.ch3);
        let new = self.calculate_channel_output(2);

        if old != new {
            let phase = (i64::from(self.render_timer) * (blip::PHASES as i64))
                / i64::from(self.ext_sample_period);
            self.blips[2].update(new.0, new.1, phase as usize);
        }
    }

    fn update_ch4<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Noise),
    {
        let old = self.calculate_channel_output(3);
        f(&mut self.ch4);
        let new = self.calculate_channel_output(3);

        if old != new {
            let phase = (i64::from(self.render_timer) * (blip::PHASES as i64))
                / i64::from(self.ext_sample_period);
            self.blips[3].update(new.0, new.1, phase as usize);
        }
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.update_ch1(|ch| ch.write_nrx0(val));
    }

    pub const fn write_nr11(&mut self, val: u8) {
        self.ch1.write_nrx1(val);
    }

    pub fn write_nr12(&mut self, val: u8) {
        self.update_ch1(|ch| ch.write_nrx2(val));
    }

    pub fn write_nr13(&mut self, val: u8) {
        self.ch1.write_nrx3(val);
    }

    pub fn write_nr14(&mut self, val: u8) {
        self.update_ch1(|ch| ch.write_nrx4(val));
    }

    pub const fn write_nr21(&mut self, val: u8) {
        self.ch2.write_nrx1(val);
    }

    pub fn write_nr22(&mut self, val: u8) {
        self.update_ch2(|ch| ch.write_nrx2(val));
    }

    pub fn write_nr23(&mut self, val: u8) {
        self.ch2.write_nrx3(val);
    }

    pub fn write_nr24(&mut self, val: u8) {
        self.update_ch2(|ch| ch.write_nrx4(val));
    }

    pub fn write_nr30(&mut self, val: u8) {
        self.update_ch3(|ch| ch.write_nr30(val));
    }

    pub const fn write_nr31(&mut self, val: u8) {
        self.ch3.write_nr31(val);
    }

    pub fn write_nr32(&mut self, val: u8) {
        self.update_ch3(|ch| ch.write_nr32(val));
    }

    pub fn write_nr33(&mut self, val: u8) {
        self.ch3.write_nr33(val);
    }

    pub fn write_nr34(&mut self, val: u8, is_cgb: bool) {
        self.update_ch3(|ch| ch.write_nr34(val, is_cgb));
    }

    pub const fn write_nr41(&mut self, val: u8) {
        self.ch4.write_nr41(val);
    }

    pub fn write_nr42(&mut self, val: u8) {
        self.update_ch4(|ch| ch.write_nr42(val));
    }

    pub const fn write_nr43(&mut self, val: u8) {
        self.ch4.write_nr43(val);
    }

    pub fn write_nr44(&mut self, val: u8) {
        self.update_ch4(|ch| ch.write_nr44(val));
    }

    pub const fn write_nr50(&mut self, val: u8) {
        if self.enabled {
            self.master_volume.write_nr50(val);
        }
    }

    pub fn write_nr51(&mut self, val: u8) {
        if self.enabled {
            let old1 = self.calculate_channel_output(0);
            let old2 = self.calculate_channel_output(1);
            let old3 = self.calculate_channel_output(2);
            let old4 = self.calculate_channel_output(3);

            self.nr51 = val;

            let new1 = self.calculate_channel_output(0);
            let new2 = self.calculate_channel_output(1);
            let new3 = self.calculate_channel_output(2);
            let new4 = self.calculate_channel_output(3);

            let phase = (i64::from(self.render_timer) * (blip::PHASES as i64))
                / i64::from(self.ext_sample_period);

            if old1 != new1 {
                self.blips[0].update(new1.0, new1.1, phase as usize);
            }
            if old2 != new2 {
                self.blips[1].update(new2.0, new2.1, phase as usize);
            }
            if old3 != new3 {
                self.blips[2].update(new3.0, new3.1, phase as usize);
            }
            if old4 != new4 {
                self.blips[3].update(new4.0, new4.1, phase as usize);
            }
        }
    }

    pub fn write_nr52(&mut self, val: u8, div_bit: bool, is_cgb: bool) {
        let was_enabled = self.enabled;

        let enabling = val & 0x80 != 0;

        let old_lengths = (!is_cgb && !was_enabled && enabling).then(|| {
            [
                self.ch1.length(),
                self.ch2.length(),
                self.ch3.length(),
                self.ch4.length(),
            ]
        });

        if !was_enabled && enabling {
            self.reset();

            self.enabled = true;

            if div_bit {
                self.skip_div_event = SkipDivEvent::Skip;

                self.div_divider = 0;
            } else {
                self.skip_div_event = SkipDivEvent::Inactive;

                self.div_divider = 7;
            }
        } else {
            self.enabled = enabling;

            if !self.enabled {
                self.reset();

                self.div_divider = 7;
            }
        }

        if let Some([l1, l2, l3, l4]) = old_lengths {
            self.ch1.set_length(l1);

            self.ch2.set_length(l2);

            self.ch3.set_length(l3);

            self.ch4.set_length(l4);
        }
    }

    pub const fn write_wave_ram(&mut self, addr: u8, val: u8, is_cgb: bool) {
        self.ch3.write_wave_ram(addr, val, is_cgb);
    }
}
