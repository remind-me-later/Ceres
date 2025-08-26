// #![no_std]
// TODO: Use borrowedBuf or something similar to avoid std io (currently nightly only)

extern crate alloc;

mod apu;
mod bess;
mod bootrom;
mod cartridge;
#[cfg(feature = "game_genie")]
mod cheats;
mod error;
mod interrupts;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod sm83;
mod timing;

use crate::{
    bootrom::Bootrom,
    memory::{Hram, Wram},
    timing::DOTS_PER_FRAME,
};
use cartridge::Cartridge;
#[cfg(feature = "game_genie")]
use cheats::GameGenie;
#[cfg(feature = "game_genie")]
pub use cheats::GameGenieCode;
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
    ppu::{PX_HEIGHT, PX_WIDTH},
    timing::FRAME_DURATION,
};
use {
    memory::{Dma, Hdma},
    sm83::Sm83,
    timing::Clock,
};

pub struct Gb<A: AudioCallback> {
    apu: Apu<A>,
    bootrom: Bootrom,
    cart: Cartridge,
    cgb_mode: CgbMode,
    clock: Clock,
    cpu: Sm83,
    dma: Dma,
    dots_ran: i32,
    #[cfg(feature = "game_genie")]
    game_genie: GameGenie,
    hdma: Hdma,
    hram: Hram,
    ints: Interrupts,
    joy: Joypad,
    key1: Key1,
    model: Model,
    ppu: Ppu,
    serial: Serial,
    wram: Wram,
}

impl<A: AudioCallback> Gb<A> {
    /// Activates a Game Genie code.
    ///
    /// # Errors
    ///
    /// Returns an error if too many codes are activated.
    #[cfg(feature = "game_genie")]
    pub const fn activate_game_genie(&mut self, code: GameGenieCode) -> Result<(), Error> {
        self.game_genie.activate_code(code)
    }

    pub const fn cart_has_battery(&self) -> bool {
        self.cart.has_battery()
    }

    pub const fn cart_header_checksum(&self) -> u8 {
        self.cart.header_checksum()
    }

    pub fn cart_title(&self) -> &[u8] {
        self.cart.ascii_title()
    }

    pub const fn cart_version(&self) -> u8 {
        self.cart.version()
    }

    pub fn change_model_and_soft_reset(&mut self, model: Model) {
        self.model = model;
        self.cgb_mode = model.into();
        self.bootrom = Bootrom::new(model);
        self.soft_reset();
    }

    #[cfg(feature = "game_genie")]
    pub fn deactivate_game_genie(&mut self, code: GameGenieCode) {
        self.game_genie.deactivate_code(code);
    }

    /// Loads the state from the provided reader.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from or seeking within the reader fails.
    pub fn load_data<R: io::Read + io::Seek>(&mut self, reader: &mut R) -> Result<(), io::Error> {
        bess::load_state(self, reader)
    }

    #[must_use]
    fn new(model: Model, sample_rate: i32, cart: Cartridge, audio_callback: A) -> Self {
        Self {
            model,
            cgb_mode: model.into(),
            cart,
            bootrom: Bootrom::new(model),
            apu: Apu::new(sample_rate, audio_callback),
            clock: Clock::default(),
            cpu: Sm83::default(),
            dma: Dma::default(),
            dots_ran: Default::default(),
            hdma: Hdma::default(),
            hram: Hram::default(),
            ints: Interrupts::default(),
            joy: Joypad::default(),
            key1: Key1::default(),
            ppu: Ppu::default(),
            serial: Serial::default(),
            wram: Wram::default(),
            #[cfg(feature = "game_genie")]
            game_genie: GameGenie::default(),
        }
    }

    #[must_use]
    pub const fn pixel_data_rgba(&self) -> &[u8] {
        self.ppu.pixel_data_rgba()
    }

    pub const fn press(&mut self, button: Button) {
        self.joy.press(button, &mut self.ints);
    }

    pub const fn release(&mut self, button: Button) {
        self.joy.release(button);
    }

    pub fn run_frame(&mut self) {
        while self.dots_ran < DOTS_PER_FRAME {
            self.run_cpu();
        }

        self.dots_ran -= DOTS_PER_FRAME;
    }

    /// Saves the current state to the provided writer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing or seeking to the writer fails.
    pub fn save_data<W: io::Write + io::Seek>(&self, writer: &mut W) -> Result<(), io::Error> {
        bess::save_state(self, writer)
    }

    pub const fn set_sample_rate(&mut self, sample_rate: i32) {
        self.apu.set_sample_rate(sample_rate);
    }

    pub fn soft_reset(&mut self) {
        self.apu.reset();
        self.clock = Clock::default();
        self.cpu = Sm83::default();
        self.dma = Dma::default();
        self.hdma = Hdma::default();
        self.ints = Interrupts::default();
        self.key1 = Key1::default();
        self.ppu = Ppu::default();
        self.serial = Serial::default();
        self.bootrom.enable();
    }
}

#[derive(Clone, Copy, Default)]
pub enum Model {
    #[default]
    Cgb,
    Dmg,
    Mgb,
}

#[derive(Clone, Copy, Default)]
enum CgbMode {
    #[default]
    Cgb,
    Compat,
    Dmg,
}

impl From<Model> for CgbMode {
    fn from(model: Model) -> Self {
        match model {
            Model::Dmg | Model::Mgb => Self::Dmg,
            Model::Cgb => Self::Cgb,
        }
    }
}

pub struct GbBuilder<A: AudioCallback> {
    audio_callback: A,
    cart: Option<Cartridge>,
    model: Model,
    sample_rate: i32,
}

impl<A: AudioCallback> GbBuilder<A> {
    pub fn build(self) -> Gb<A> {
        Gb::new(
            self.model,
            self.sample_rate,
            self.cart.unwrap_or_default(),
            self.audio_callback,
        )
    }

    pub fn can_load_save_data(&self) -> bool {
        self.cart
            .as_ref()
            .is_some_and(cartridge::Cartridge::has_battery)
    }

    pub fn new(sample_rate: i32, audio_callback: A) -> Self {
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

    /// Loads a ROM into the builder.
    ///
    /// # Errors
    ///
    /// Returns an error if the ROM data is invalid or cannot be parsed as a cartridge.
    pub fn with_rom(mut self, rom: Box<[u8]>) -> Result<Self, Error> {
        self.cart = Some(Cartridge::new(rom)?);
        Ok(self)
    }
}
