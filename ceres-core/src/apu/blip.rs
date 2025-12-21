use std::f64;

// From SameBoy apu.h
pub const WIDTH: usize = 64; // GB_BAND_LIMITED_WIDTH
pub const PHASES: usize = 256; // GB_BAND_LIMITED_PHASES
pub const ONE: i32 = 0x10_000; // GB_BAND_LIMITED_ONE
const MASK: usize = WIDTH - 1;

// Band limited synthesis loosely based on: http://www.slack.net/~ant/bl-synth/
pub struct Blip {
    buffer_l: [i32; WIDTH],
    buffer_r: [i32; WIDTH],
    pos: usize,
    output_l: i32,
    output_r: i32,
    last_l: i32,
    last_r: i32,
    steps: Box<[[i32; WIDTH]; PHASES]>,
}

impl Default for Blip {
    fn default() -> Self {
        Self::new()
    }
}

impl Blip {
    pub fn new() -> Self {
        let mut steps = Box::new([[0; WIDTH]; PHASES]);
        let mut master = vec![0.0; WIDTH * PHASES];

        // From SameBoy apu.c
        let lowpass = 15.0 / 16.0;
        #[expect(clippy::cast_precision_loss)]
        let to_angle = f64::consts::PI / (PHASES as f64) * lowpass;

        let mut sum = 0.0;

        for (i, m) in master.iter_mut().enumerate() {
            // Exact Blackman window
            // From SameBoy apu.c
            const A0: f64 = 7938.0 / 18608.0;
            const A1: f64 = 9240.0 / 18608.0;
            const A2: f64 = 1430.0 / 18608.0;

            #[expect(clippy::cast_precision_loss)]
            let i_f = i as f64;

            #[expect(clippy::cast_precision_loss)]
            let window_angle = (2.0 * f64::consts::PI * i_f) / ((WIDTH * PHASES) as f64);
            let window = A2.mul_add(
                (2.0 * window_angle).cos(),
                A1.mul_add(-window_angle.cos(), A0),
            );

            #[expect(clippy::cast_precision_loss)]
            let angle = (i_f - (WIDTH * PHASES) as f64 / 2.0) * to_angle;
            let val = if angle == 0.0 {
                1.0
            } else {
                angle.sin() / angle
            } * window;

            *m = val;
            sum += val;
        }

        for m in &mut master {
            *m /= sum;
        }

        for phase in 0..PHASES {
            let mut error = ONE;
            for i in 0..WIDTH {
                let mut sum = 0.0;
                for j in 0..PHASES {
                    let index = i * PHASES + j;
                    if index >= phase {
                        sum += master[index - phase];
                    }
                }
                #[expect(clippy::cast_possible_truncation)]
                let cur = (sum * f64::from(ONE)) as i32;
                error -= cur;
                steps[phase][i] = cur;
            }
            steps[phase][WIDTH / 2] += error;
        }

        Self {
            buffer_l: [0; WIDTH],
            buffer_r: [0; WIDTH],
            pos: 0,
            output_l: 0,
            output_r: 0,
            last_l: 0,
            last_r: 0,
            steps,
        }
    }

    pub fn update(&mut self, l: i32, r: i32, phase: usize) {
        let delta_l = l - self.last_l;
        let delta_r = r - self.last_r;

        if delta_l == 0 && delta_r == 0 {
            return;
        }

        self.last_l = l;
        self.last_r = r;

        let delay = phase / PHASES;
        let phase = phase & (PHASES - 1);

        let steps = &self.steps[phase];

        for (i, &step) in steps.iter().enumerate().take(WIDTH) {
            let offset = (i + self.pos + delay) & MASK;
            self.buffer_l[offset] += delta_l * step;
            self.buffer_r[offset] += delta_r * step;
        }
    }

    pub fn read(&mut self) -> (i32, i32) {
        self.output_l += self.buffer_l[self.pos];
        self.output_r += self.buffer_r[self.pos];

        self.buffer_l[self.pos] = 0;
        self.buffer_r[self.pos] = 0;

        self.pos = (self.pos + 1) & MASK;

        (self.output_l, self.output_r)
    }
}
