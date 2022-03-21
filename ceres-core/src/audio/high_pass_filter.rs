use crate::T_CYCLES_PER_SECOND;

pub struct HighPassFilter {
    capacitor: f32,
    charge_factor: f32,
}

impl HighPassFilter {
    pub fn new(sample_rate: u32) -> Self {
        let charge_factor = 0.998943_f32.powf(T_CYCLES_PER_SECOND as f32 / sample_rate as f32);

        Self {
            capacitor: 0.0,
            charge_factor,
        }
    }

    pub fn filter(&mut self, input: i16, dac_enabled: bool) -> f32 {
        // TODO: amplification
        let input = (input * 32) as f32 / 32768.0;
        if dac_enabled {
            let output = input - self.capacitor;
            self.capacitor = input - output * self.charge_factor;
            output
        } else {
            0.0
        }
    }
}
