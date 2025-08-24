use crate::Sample;

pub struct HighPassFilter {
    capacitor_l: i32,
    capacitor_r: i32,
}

impl HighPassFilter {
    pub fn high_pass(&mut self, l: Sample, r: Sample) -> (Sample, Sample) {
        // Using 16-bit fixed-point arithmetic
        const FILTER_COEFF: i32 = 0xFFFE; // Q16 format of 0.999958
        const PRECISION_BITS: i32 = 16;

        // Convert samples to larger type to avoid overflow
        let l_i32 = i32::from(l);
        let r_i32 = i32::from(r);

        let out_left_i32 = l_i32 - self.capacitor_l;
        let out_right_i32 = r_i32 - self.capacitor_r;

        self.capacitor_l = l_i32 - ((out_left_i32 * FILTER_COEFF) >> PRECISION_BITS);
        self.capacitor_r = r_i32 - ((out_right_i32 * FILTER_COEFF) >> PRECISION_BITS);

        #[expect(clippy::cast_possible_truncation)]
        let out_left = out_left_i32.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        #[expect(clippy::cast_possible_truncation)]
        let out_right = out_right_i32.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;

        (out_left, out_right)
    }

    pub const fn new() -> Self {
        Self {
            capacitor_l: 0,
            capacitor_r: 0,
        }
    }
}
