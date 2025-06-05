// #![no_std]
// TODO: Use borrowedBuf or something similar to avoid std io (currently nightly only)

extern crate alloc;

mod apu;
mod bess;
mod bootrom;
mod cartridge;
mod cpu;
mod error;
mod interrupts;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod timing;

use crate::{
    bootrom::Bootrom,
    memory::{Hram, Wram},
    timing::TC_PER_FRAME,
};
use cartridge::Cartridge;
use interrupts::Interrupts;
use joypad::Joypad;
use memory::Key1;
use serial::Serial;
use std::io;
use {apu::Apu, ppu::Ppu};
pub use {
    apu::{AudioCallback, Sample},
    error::Error,
    joypad::Button,
    ppu::{PX_HEIGHT, PX_WIDTH, VRAM_PX_HEIGHT, VRAM_PX_WIDTH},
    timing::FRAME_DURATION,
};
use {
    cpu::Cpu,
    memory::{Dma, Hdma},
    timing::Clock,
};

#[derive(Debug)]
pub struct Gb<C: AudioCallback> {
    apu: Apu<C>,
    bootrom: Bootrom,
    cart: Cartridge,
    cgb_mode: CgbMode,
    clock: Clock,
    cpu: Cpu,
    dma: Dma,
    hdma: Hdma,
    hram: Hram,
    ints: Interrupts,
    joy: Joypad,
    key1: Key1,
    model: Model,
    ppu: Ppu,
    serial: Serial,
    t_cycles_run: i32,
    wram: Wram,
}

impl<C: AudioCallback> Gb<C> {
    #[must_use]
    fn new(model: Model, sample_rate: i32, cart: Cartridge, audio_callback: C) -> Self {
        Self {
            model,
            cgb_mode: model.into(),
            cart,
            bootrom: Bootrom::new(model),
            apu: Apu::new(sample_rate, audio_callback),
            clock: Clock::default(),
            cpu: Cpu::default(),
            dma: Dma::default(),
            t_cycles_run: Default::default(),
            hdma: Hdma::default(),
            hram: Hram::default(),
            ints: Interrupts::default(),
            joy: Joypad::default(),
            key1: Key1::default(),
            ppu: Ppu::default(),
            serial: Serial::default(),
            wram: Wram::default(),
        }
    }

    pub fn soft_reset(&mut self) {
        self.apu.reset();
        self.clock = Clock::default();
        self.cpu = Cpu::default();
        self.dma = Dma::default();
        self.hdma = Hdma::default();
        self.ints = Interrupts::default();
        self.key1 = Key1::default();
        self.ppu = Ppu::default();
        self.serial = Serial::default();
        self.bootrom.enable();
    }

    pub fn change_model_and_soft_reset(&mut self, model: Model) {
        self.model = model;
        self.cgb_mode = model.into();
        self.bootrom = Bootrom::new(model);
        self.soft_reset();
    }

    pub fn run_frame(&mut self) {
        while self.t_cycles_run < TC_PER_FRAME {
            self.run_cpu();
        }

        self.t_cycles_run -= TC_PER_FRAME;
    }

    #[must_use]
    pub const fn pixel_data_rgba(&self) -> &[u8] {
        self.ppu.pixel_data_rgba()
    }

    #[must_use]
    pub const fn vram_data_rgba(&self) -> &[u8] {
        self.ppu.vram_data_rgba()
    }

    pub const fn press(&mut self, button: Button) {
        self.joy.press(button, &mut self.ints);
    }

    pub const fn release(&mut self, button: Button) {
        self.joy.release(button);
    }

    pub fn save_data<W: io::Write + io::Seek>(&self, writer: &mut W) -> Result<(), io::Error> {
        bess::save_state(self, writer)
    }

    pub fn load_data<R: io::Read + io::Seek>(&mut self, reader: &mut R) -> Result<(), io::Error> {
        bess::load_state(self, reader)
    }

    pub const fn set_sample_rate(&mut self, sample_rate: i32) {
        self.apu.set_sample_rate(sample_rate);
    }

    pub fn cart_title(&self) -> &[u8] {
        self.cart.ascii_title()
    }

    pub const fn cart_header_checksum(&self) -> u8 {
        self.cart.header_checksum()
    }

    pub const fn cart_version(&self) -> u8 {
        self.cart.version()
    }

    pub const fn cart_has_battery(&self) -> bool {
        self.cart.has_battery()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum Model {
    Dmg,
    Mgb,
    #[default]
    Cgb,
}

#[derive(Clone, Copy, Debug, Default)]
enum CgbMode {
    Dmg,
    Compat,
    #[default]
    Cgb,
}

impl From<Model> for CgbMode {
    fn from(model: Model) -> Self {
        match model {
            Model::Dmg | Model::Mgb => Self::Dmg,
            Model::Cgb => Self::Cgb,
        }
    }
}

pub struct GbBuilder<C: AudioCallback> {
    model: Model,
    cart: Option<Cartridge>,
    sample_rate: i32,
    audio_callback: C,
}

impl<C: AudioCallback> GbBuilder<C> {
    pub fn new(sample_rate: i32, audio_callback: C) -> Self {
        Self {
            model: Model::default(),
            cart: None,
            sample_rate,
            audio_callback,
        }
    }

    #[must_use]
    pub const fn with_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }

    pub fn with_rom(mut self, rom: Box<[u8]>) -> Result<Self, Error> {
        self.cart = Some(Cartridge::new(rom)?);
        Ok(self)
    }

    pub fn can_load_save_data(&self) -> bool {
        self.cart
            .as_ref()
            .is_some_and(cartridge::Cartridge::has_battery)
    }

    pub fn build(self) -> Gb<C> {
        Gb::new(
            self.model,
            self.sample_rate,
            self.cart.unwrap_or_default(),
            self.audio_callback,
        )
    }
}
