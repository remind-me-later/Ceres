use super::{Apu, NOISE, SQUARE_1};
use crate::{AudioCallback, Model};

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
    ch1_duty_step: u8,
    ch1_countdown: u16,
    ch1_length: u16,
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
            ch1_duty_step: 6,
            ch1_countdown: 79,
            ch1_length: 64,
            other_volume_countdown: 1,
            noise_alignment: 2,
        };
        match model {
            Model::Dmg0 => {
                pb.div_divider = 188;
                pb.sweep_countdown = 5;
                pb.ch1_duty_step = 4;
                pb.ch1_countdown = 31;
                pb.noise_alignment = 248;
            }
            Model::DmgB | Model::Mgb => {
                pb.div_divider = 17;
                pb.sweep_countdown = 2;
                pb.ch1_volume_countdown = 1;
                pb.ch1_duty_step = 2;
                pb.ch1_countdown = 11;
                pb.other_volume_countdown = 6;
                pb.noise_alignment = 198;
            }
            Model::Sgb | Model::Sgb2 => {
                pb.div_divider = 210;
                pb.sweep_countdown = 4;
                pb.ch1_active = false;
                pb.ch1_volume_countdown = 6;
                pb.ch1_duty_step = 0;
                pb.ch1_countdown = 0xFFFF;
                pb.other_volume_countdown = 6;
                pb.noise_alignment = 4;
            }
            Model::Cgb0 => {
                pb.ch1_length = 62;
                if cgb_cart {
                    pb.ch1_countdown = 73;
                    pb.noise_alignment = 28;
                } else {
                    pb.noise_alignment = 8;
                }
            }
            Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE => {
                if cgb_cart {
                    pb.div_divider = 56;
                    pb.ch1_countdown = 73;
                    pb.noise_alignment = 22;
                }
            }
            Model::Agb => {
                if cgb_cart {
                    pb.div_divider = 56;
                    pb.ch1_countdown = 71;
                    pb.noise_alignment = 24;
                } else {
                    pb.ch1_countdown = 77;
                    pb.noise_alignment = 4;
                }
            }
        }
        pb
    }
}

impl<A: AudioCallback> Apu<A> {
    /// Sets the state the boot ROM leaves behind (see [`PostBoot`]).
    pub fn post_boot(&mut self, pb: PostBoot) {
        self.reset();
        self.enabled = true;
        self.nr50 = 0x77;
        self.nr51 = 0xF3;
        self.lf_div = 1;
        self.div_divider = pb.div_divider;
        self.sweep.countdown = pb.sweep_countdown;

        let [ch1, ch2] = &mut self.squares;
        ch1.write_nrx1(0x80);
        ch1.envelope.nrx2 = 0xF3;
        ch1.write_nrx3(0xC1);
        ch1.restore_nrx4(if pb.ch1_active { 0x87 } else { 0x07 });
        ch1.countdown = pb.ch1_countdown;
        ch1.length.counter = pb.ch1_length;
        ch1.envelope.countdown = pb.ch1_volume_countdown;
        ch1.duty_step = pb.ch1_duty_step;
        if pb.ch1_active {
            ch1.out.active = true;
            ch1.did_tick = true;
            ch1.envelope.clock.locked = true;
            ch1.envelope.clock.should_lock = true;
        }
        ch2.countdown = 0xFFFF;
        ch2.envelope.countdown = pb.other_volume_countdown;
        self.noise.envelope.countdown = pb.other_volume_countdown;
        self.noise.alignment = pb.noise_alignment;

        let c = self.ctx(&super::ApuCtx::default());
        if c.rev.is_agb() {
            for i in 0..super::N_CHANNELS {
                self.update_sample(i, 0, &c);
            }
        } else {
            if pb.ch1_active {
                // The decayed note leaves its DC level behind.
                self.squares[SQUARE_1].update_sample(1, &c);
                self.squares[SQUARE_1].update_sample(0, &c);
            }
            for i in SQUARE_1 + 1..=NOISE {
                self.output_mut(i).sample = 0x10;
            }
        }
    }
}
