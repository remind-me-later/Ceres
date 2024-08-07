#![allow(clippy::module_name_repetitions)]
use {
    super::envelope::Envelope,
    crate::apu::{LengthTimer, PHalf, WaveLength},
};

#[derive(Default)]
struct AbstractSquare<S: super::SweepTrait> {
    ltim: LengthTimer<0x3F>,
    wl: WaveLength<4, S>,
    env: Envelope,

    on: bool,
    dac_on: bool,
    out: u8,

    duty: u8,
    duty_bit: u8,
}

impl<S: super::SweepTrait> AbstractSquare<S> {
    fn read_nrx0(&self) -> u8 {
        self.wl.read_sweep()
    }

    const fn read_nrx1(&self) -> u8 {
        0x3F | (self.duty << 6)
    }

    const fn read_nrx2(&self) -> u8 {
        self.env.read()
    }

    fn read_nrx4(&self) -> u8 {
        0xBF | self.ltim.read_on()
    }

    fn write_nrx0(&mut self, val: u8) {
        self.wl.write_sweep(val);
    }

    fn write_nrx1(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.ltim.write_len(val);
    }

    fn write_nrx2(&mut self, val: u8) {
        if val & 0xF8 == 0 {
            self.on = false;
            self.dac_on = false;
        } else {
            self.dac_on = true;
        }

        self.env.write(val);
    }

    fn write_nrx3(&mut self, val: u8) {
        self.wl.write_low(val);
    }

    fn write_nrx4(&mut self, val: u8) {
        self.wl.write_high(val);
        self.ltim.write_on(val, &mut self.on);

        // trigger
        if val & 0x80 != 0 {
            if self.dac_on {
                self.on = true;
            }

            self.env.trigger();
            self.wl.trigger(&mut self.on);
            self.ltim.trigger(&mut self.on);
        }
    }

    fn step_sample(&mut self, cycles: i32) {
        // Shape of the duty waveform for a certain duty
        const DUTY_WAV: [u8; 4] = [
            // _______- : 12.5%
            0b0000_0001,
            // -______- : 25%
            0b1000_0001,
            // -____--- : 50%
            0b1000_0111,
            // _------_ : 75%
            0b0111_1110,
        ];

        if !self.on() {
            return;
        }

        if self.wl.step(cycles) {
            self.duty_bit = (self.duty_bit + 1) & 7;
            self.out = u8::from((DUTY_WAV[self.duty as usize] & (1 << self.duty_bit)) != 0);
        }
    }

    fn step_sweep(&mut self) {
        if self.on {
            self.wl.step_sweep(&mut self.on);
        }
    }

    fn step_env(&mut self) {
        if self.on {
            self.env.step();
        }
    }

    const fn out(&self) -> u8 {
        self.out * self.env.vol()
    }

    const fn true_on(&self) -> bool {
        self.on && self.dac_on
    }

    fn step_len(&mut self) {
        self.ltim.step(&mut self.on);
    }

    fn set_period_half(&mut self, p_half: PHalf) {
        self.ltim.set_phalf(p_half);
    }

    const fn on(&self) -> bool {
        self.on
    }
}

#[derive(Default)]
pub(super) struct Square1 {
    ab: AbstractSquare<super::Sweep>,
}

impl Square1 {
    pub(super) fn read_nr10(&self) -> u8 {
        self.ab.read_nrx0()
    }

    pub(super) const fn read_nr11(&self) -> u8 {
        self.ab.read_nrx1()
    }

    pub(super) const fn read_nr12(&self) -> u8 {
        self.ab.read_nrx2()
    }

    pub(super) fn read_nr14(&self) -> u8 {
        self.ab.read_nrx4()
    }

    pub(super) fn write_nr10(&mut self, val: u8) {
        self.ab.write_nrx0(val);
    }

    pub(super) fn write_nr11(&mut self, val: u8) {
        self.ab.write_nrx1(val);
    }

    pub(super) fn write_nr12(&mut self, val: u8) {
        self.ab.write_nrx2(val);
    }

    pub(super) fn write_nr13(&mut self, val: u8) {
        self.ab.write_nrx3(val);
    }

    pub(super) fn write_nr14(&mut self, val: u8) {
        self.ab.write_nrx4(val);
    }

    pub(super) const fn out(&self) -> u8 {
        self.ab.out()
    }

    pub(super) fn step_sample(&mut self, cycles: i32) {
        self.ab.step_sample(cycles);
    }

    pub(super) fn step_sweep(&mut self) {
        self.ab.step_sweep();
    }

    pub(super) fn step_env(&mut self) {
        self.ab.step_env();
    }

    pub(super) const fn true_on(&self) -> bool {
        self.ab.true_on()
    }

    pub(super) fn step_len(&mut self) {
        self.ab.step_len();
    }

    pub(super) fn set_period_half(&mut self, p_half: PHalf) {
        self.ab.set_period_half(p_half);
    }

    pub(super) const fn on(&self) -> bool {
        self.ab.on()
    }
}

#[derive(Default)]
pub(super) struct Square2 {
    ab: AbstractSquare<()>,
}

impl Square2 {
    pub(super) const fn read_nr21(&self) -> u8 {
        self.ab.read_nrx1()
    }

    pub(super) const fn read_nr22(&self) -> u8 {
        self.ab.read_nrx2()
    }

    pub(super) fn read_nr24(&self) -> u8 {
        self.ab.read_nrx4()
    }

    pub(super) fn write_nr21(&mut self, val: u8) {
        self.ab.write_nrx1(val);
    }

    pub(super) fn write_nr22(&mut self, val: u8) {
        self.ab.write_nrx2(val);
    }

    pub(super) fn write_nr23(&mut self, val: u8) {
        self.ab.write_nrx3(val);
    }

    pub(super) fn write_nr24(&mut self, val: u8) {
        self.ab.write_nrx4(val);
    }

    pub(super) const fn out(&self) -> u8 {
        self.ab.out()
    }

    pub(super) fn step_sample(&mut self, cycles: i32) {
        self.ab.step_sample(cycles);
    }

    pub(super) fn step_env(&mut self) {
        self.ab.step_env();
    }

    pub(super) const fn true_on(&self) -> bool {
        self.ab.true_on()
    }

    pub(super) fn step_len(&mut self) {
        self.ab.step_len();
    }

    pub(super) fn set_period_half(&mut self, p_half: PHalf) {
        self.ab.set_period_half(p_half);
    }

    pub(super) const fn on(&self) -> bool {
        self.ab.on()
    }
}
