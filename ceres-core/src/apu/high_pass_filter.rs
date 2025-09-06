use crate::Sample;

#[derive(Default)]
pub struct HighPassFilter {
    capacitor_l: f32,
    capacitor_r: f32,
}

impl HighPassFilter {
    #[expect(clippy::float_arithmetic)]
    pub fn high_pass(&mut self, l: Sample, r: Sample) -> (Sample, Sample) {
        // FIXME: The exact value depends on the model. Research.
        const FILTER_COEFF: f32 = 0.998_943;

        let l_f32 = f32::from(l);
        let r_f32 = f32::from(r);

        let out_left_f32 = l_f32 - self.capacitor_l;
        let out_right_f32 = r_f32 - self.capacitor_r;

        self.capacitor_l = out_left_f32.mul_add(-FILTER_COEFF, l_f32);
        self.capacitor_r = out_right_f32.mul_add(-FILTER_COEFF, r_f32);

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
