use crate::{Sample, timing::DOTS_PER_SEC};

pub struct HighPassFilter {
    capacitor_l: f32,
    capacitor_r: f32,
    filter_coeff: f32,
}

impl Default for HighPassFilter {
    fn default() -> Self {
        Self {
            capacitor_l: 0.0,
            capacitor_r: 0.0,
            filter_coeff: 0.999,
        }
    }
}

impl HighPassFilter {
    pub fn new(sample_rate: i32) -> Self {
        let mut hpf = Self::default();
        hpf.set_sample_rate(sample_rate);
        hpf
    }

    pub fn set_sample_rate(&mut self, sample_rate: i32) {
        // Value from SameBoy apu.c: pow(0.999958, 4194304 / sample_rate)
        let cycles_per_sample = DOTS_PER_SEC as f32 / sample_rate as f32;
        self.filter_coeff = 0.999_958_f32.powf(cycles_per_sample);
    }

    #[expect(clippy::float_arithmetic)]
    pub fn high_pass(&mut self, l: Sample, r: Sample) -> (Sample, Sample) {
        let l_f32 = f32::from(l);
        let r_f32 = f32::from(r);

        let out_left_f32 = l_f32 - self.capacitor_l;
        let out_right_f32 = r_f32 - self.capacitor_r;

        self.capacitor_l = out_left_f32.mul_add(-self.filter_coeff, l_f32);
        self.capacitor_r = out_right_f32.mul_add(-self.filter_coeff, r_f32);

        #[expect(clippy::cast_possible_truncation)]
        let out_left = out_left_f32
            .round()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        #[expect(clippy::cast_possible_truncation)]
        let out_right = out_right_f32
            .round()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;

        (out_left, out_right)
    }
}
