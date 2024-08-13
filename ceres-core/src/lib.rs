#![no_std]

use core::time::Duration;

use interrupts::Interrupts;
use joypad::Joypad;
use memory::{Key1, Svbk};
use serial::Serial;
use {apu::Apu, memory::HdmaState, ppu::Ppu, timing::TIMAState};
pub use {
    apu::{AudioCallback, Sample},
    cart::{Cart, Error},
    joypad::Button,
    ppu::{PX_HEIGHT, PX_WIDTH},
};

extern crate alloc;

mod apu;
mod cart;
mod cpu;
mod interrupts;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod timing;

pub const FPS: f32 = 59.7;
pub const FRAME_DURATION: Duration =
    Duration::new(0, (1_000_000_000_0u64 / ((FPS * 10.0) as u64)) as u32);

// t-cycles per second
pub const TC_SEC: i32 = 0x40_0000; // 2^22
pub const HRAM_SIZE: u8 = 0x80;
pub const WRAM_SIZE: u16 = 0x2000 * 4;

pub struct Gb<C: AudioCallback> {
    model: Model,
    cgb_mode: CgbMode,

    // cartridge
    cart: Cart,
    bootrom: Option<&'static [u8]>,

    // cpu
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    pc: u16,

    ei_delay: bool,
    cpu_halted: bool,
    halt_bug: bool,

    // memory
    wram: [u8; WRAM_SIZE as usize],
    hram: [u8; HRAM_SIZE as usize],
    svbk: Svbk,
    key1: Key1,

    // -- dma
    dma: u8,
    dma_on: bool,
    dma_addr: u16,
    dma_restarting: bool,
    dma_cycles: i32,

    // -- hdma
    hdma5: u8,
    hdma_src: u16,
    hdma_dst: u16,
    hdma_len: u16,
    hdma_state: HdmaState,

    // clock
    tima: u8,
    tma: u8,
    tac: u8,
    div: u16,
    tima_state: TIMAState,

    // peripherals
    ppu: Ppu,
    apu: Apu<C>,
    serial: Serial,
    ints: Interrupts,
    joy: Joypad,
}

impl<C: AudioCallback> Gb<C> {
    #[must_use]
    pub fn new(model: Model, sample_rate: i32, cart: Cart, audio_callback: C) -> Self {
        const DMG_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/dmg.bin");
        const MGB_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/mgb.bin");
        const CGB_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/cgb.bin");

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

            wram: [0; WRAM_SIZE as usize],
            hram: [0; HRAM_SIZE as usize],
            af: Default::default(),
            bc: Default::default(),
            cpu_halted: Default::default(),
            de: Default::default(),
            dma_addr: Default::default(),
            dma_cycles: Default::default(),
            dma_on: Default::default(),
            dma_restarting: Default::default(),
            dma: Default::default(),
            ei_delay: Default::default(),
            halt_bug: Default::default(),
            hdma_dst: Default::default(),
            hdma_len: Default::default(),
            hdma_src: Default::default(),
            hdma_state: HdmaState::default(),
            hdma5: Default::default(),
            hl: Default::default(),
            ints: Interrupts::default(),
            joy: Joypad::default(),
            key1: Key1::default(),
            pc: Default::default(),
            ppu: Ppu::default(),
            serial: Serial::default(),
            sp: Default::default(),
            svbk: Svbk::default(),
            tac: Default::default(),
            tima_state: TIMAState::default(),
            tima: Default::default(),
            tma: Default::default(),
            div: Default::default(),
        }
    }

    #[inline]
    pub fn run_frame(&mut self) {
        while !self.ppu.frame_done() {
            self.run_cpu();
        }

        self.ppu.reset_frame_done();
    }

    #[must_use]
    #[inline]
    pub fn cartridge(&mut self) -> &mut Cart {
        &mut self.cart
    }

    #[must_use]
    #[inline]
    pub const fn pixel_data_rgba(&self) -> &[u8] {
        self.ppu.pixel_data_rgb()
    }

    #[inline]
    pub fn press(&mut self, button: Button) {
        self.joy.press(button, &mut self.ints);
    }

    #[inline]
    pub fn release(&mut self, button: Button) {
        self.joy.release(button);
    }
}

#[derive(Clone, Copy)]
pub enum Model {
    Dmg,
    Mgb,
    Cgb,
}

enum CgbMode {
    Dmg,
    Compat,
    Cgb,
}
