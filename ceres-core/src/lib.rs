// #![no_std]
// TODO: Use borrowedBuf or something similar to avoid std io (currently nightly only)

extern crate alloc;

mod apu;
mod bess;
mod cartridge;
mod cpu;
mod error;
mod interrupts;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod timing;

use cartridge::Cartridge;
use core::time::Duration;
use interrupts::Interrupts;
use joypad::Joypad;
use memory::{Key1, Svbk};
use serial::Serial;
use std::io;
use {apu::Apu, ppu::Ppu};
use {
    cpu::Cpu,
    memory::{Dma, Hdma},
    timing::Clock,
};

pub use {
    error::Error,
    joypad::Button,
    ppu::{PX_HEIGHT, PX_WIDTH, VRAM_PX_HEIGHT, VRAM_PX_WIDTH},
};

pub const FRAME_DURATION: Duration = Duration::new(0, 16_742_706);
pub const TC_PER_FRAME: i32 = 70224; // t-cycles per frame

// t-cycles per second
pub const TC_SEC: i32 = 0x40_0000; // 2^22
pub const HRAM_SIZE: u8 = 0x7F;
pub const WRAM_SIZE_GB: u16 = 0x2000;
pub const WRAM_SIZE_CGB: u16 = WRAM_SIZE_GB * 4;

// Boot ROMs
const DMG_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/dmg.bin");
const MGB_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/mgb.bin");
const CGB_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/cgb.bin");

pub type Sample = i16;

pub trait AudioCallback {
    fn audio_sample(&self, l: Sample, r: Sample);
}

#[derive(Debug)]
pub struct Gb<C: AudioCallback> {
    model: Model,
    cgb_mode: CgbMode,
    dot_accumulator: i32,

    cart: Cartridge,
    bootrom: Option<&'static [u8]>,

    cpu: Cpu,
    clock: Clock,

    wram: Box<[u8; WRAM_SIZE_CGB as usize]>,
    hram: [u8; HRAM_SIZE as usize],
    svbk: Svbk,
    key1: Key1,
    dma: Dma,
    hdma: Hdma,

    ppu: Ppu,
    apu: Apu<C>,
    serial: Serial,
    ints: Interrupts,
    joy: Joypad,
}

impl<C: AudioCallback> Gb<C> {
    #[must_use]
    fn new(model: Model, sample_rate: i32, cart: Cartridge, audio_callback: C) -> Self {
        let cgb_mode = match model {
            Model::Dmg | Model::Mgb => CgbMode::Dmg,
            Model::Cgb => CgbMode::Cgb,
        };

        let bootrom = Some(match model {
            Model::Dmg => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::Cgb => CGB_BOOTROM,
        });

        Self {
            model,
            cgb_mode,
            cart,
            bootrom,
            apu: Apu::new(sample_rate, audio_callback),

            wram: {
                #[expect(
                    clippy::unwrap_used,
                    reason = "RGB_BUF_SIZE is a constant, so this will never panic."
                )]
                vec![0; WRAM_SIZE_CGB as usize]
                    .into_boxed_slice()
                    .try_into()
                    .unwrap()
            },
            hram: [0; HRAM_SIZE as usize],
            cpu: Cpu::default(),
            dma: Dma::default(),
            hdma: Hdma::default(),
            ints: Interrupts::default(),
            joy: Joypad::default(),
            key1: Key1::default(),
            ppu: Ppu::default(),
            serial: Serial::default(),
            svbk: Svbk::default(),
            clock: Clock::default(),
            dot_accumulator: Default::default(),
        }
    }

    pub fn soft_reset(&mut self) {
        self.bootrom = match self.model {
            Model::Dmg => Some(DMG_BOOTROM),
            Model::Mgb => Some(MGB_BOOTROM),
            Model::Cgb => Some(CGB_BOOTROM),
        };

        self.cpu = Cpu::default();
        self.dma = Dma::default();
        self.hdma = Hdma::default();
        self.ints = Interrupts::default();
        self.joy = Joypad::default();
        self.key1 = Key1::default();
        self.ppu = Ppu::default();
        self.serial = Serial::default();
        self.svbk = Svbk::default();
        self.clock = Clock::default();
        self.dot_accumulator = Default::default();
        self.ppu = Ppu::default();
        self.apu.reset();
        self.serial = Serial::default();
        self.ints = Interrupts::default();
        self.joy = Joypad::default();
        self.svbk = Svbk::default();
        self.key1 = Key1::default();
    }

    pub fn change_model_and_soft_reset(&mut self, model: Model) {
        self.model = model;
        self.cgb_mode = match model {
            Model::Dmg | Model::Mgb => CgbMode::Dmg,
            Model::Cgb => CgbMode::Cgb,
        };
        self.soft_reset();
    }

    pub fn run_frame(&mut self) {
        while self.dot_accumulator < TC_PER_FRAME {
            self.run_cpu();
        }

        self.dot_accumulator -= TC_PER_FRAME;
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

    pub const fn set_sample_rate(&mut self, sample_rate: i32) {
        self.apu.set_sample_rate(sample_rate);
    }

    // Cartridge functions
    pub fn title(&self) -> &[u8] {
        self.cart.ascii_title()
    }

    pub const fn header_checksum(&self) -> u8 {
        self.cart.header_checksum()
    }

    pub const fn version(&self) -> u8 {
        self.cart.version()
    }

    pub const fn has_battery(&self) -> bool {
        self.cart.has_battery()
    }

    pub const fn cgb_regs_available(&self) -> bool {
        // The bootrom writes the color palettes for compatibility mode, so we must allow it to write to those registers,
        // since it's not modifiable by the user there should be no issues.
        matches!(self.cgb_mode, CgbMode::Cgb) || self.bootrom.is_some()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Model {
    Dmg,
    Mgb,
    Cgb,
}

#[derive(Clone, Copy, Debug)]
enum CgbMode {
    Dmg,
    Compat,
    Cgb,
}

pub struct GbBuilder<C: AudioCallback> {
    gb: Gb<C>,
}

impl<C: AudioCallback> GbBuilder<C> {
    pub fn new(
        model: Model,
        sample_rate: i32,
        rom: Option<Box<[u8]>>,
        audio_callback: C,
    ) -> Result<Self, Error> {
        let cart = if let Some(rom) = rom {
            Cartridge::new(rom)?
        } else {
            Cartridge::default()
        };

        Ok(Self {
            gb: Gb::new(model, sample_rate, cart, audio_callback),
        })
    }

    pub const fn can_load_save_data(&self) -> bool {
        self.gb.cart.has_battery()
    }

    pub fn load_save_data<R: io::Read + io::Seek>(
        mut self,
        reader: &mut R,
    ) -> Result<Self, io::Error> {
        bess::load_state(&mut self.gb, reader)?;
        Ok(self)
    }

    pub fn build(self) -> Gb<C> {
        self.gb
    }
}
