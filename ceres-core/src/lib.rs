#![no_std]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::struct_excessive_bools)]

extern crate alloc;

use {
    apu::Apu,
    bootrom::BootRom,
    core::time::Duration,
    cpu::Regs,
    interrupts::Interrupts,
    joypad::Joypad,
    memory::{Dma, Hdma, Hram, Key1, Wram},
    ppu::Ppu,
    serial::Serial,
    timer::Timer,
};
pub use {
    apu::{AudioCallbacks, Frame, Sample},
    cartridge::{Cartridge, Header},
    error::Error,
    joypad::Button,
    ppu::{VideoCallbacks, PX_HEIGHT, PX_WIDTH},
};

mod apu;
mod bootrom;
mod cartridge;
mod cpu;
mod error;
mod interrupts;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod timer;

// 59.7 fps
pub const NANOSECONDS_PER_FRAME: u64 = 16_750_418;
pub const FRAME_DURATION: Duration = Duration::from_nanos(NANOSECONDS_PER_FRAME);
pub const T_CYCLES_PER_SECOND: u32 = 4_194_304;
// 2^22
pub const M_CYCLES_PER_SECOND: u32 = T_CYCLES_PER_SECOND / 4;
pub const T_CYCLES_PER_FRAME: u32 = 70224;
pub const M_CYCLES_PER_FRAME: u32 = T_CYCLES_PER_FRAME / 4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Dmg,
    Mgb,
    Cgb,
}

#[derive(Clone, Copy)]
enum FunctionMode {
    Dmg,
    Compat,
    Cgb,
}

pub struct Gb {
    reg: Regs,
    ei_delay: bool,
    ime: bool,
    halted: bool,
    halt_bug: bool,
    key1: Key1,
    ints: Interrupts,
    ppu: Ppu,
    joypad: Joypad,
    cart: Cartridge,
    timer: Timer,
    hram: Hram,
    wram: Wram,
    apu: Apu,
    serial: Serial,
    dma: Dma,
    hdma: Hdma,
    boot_rom: BootRom,
    model: Model,
    in_double_speed: bool,
    function_mode: FunctionMode,
}

impl Gb {
    /// # Safety
    ///
    /// `audio_callbacks` and `video_callbacks` should not be dropped before the struct 
    /// and be in the heap, so that it's position doesn't change
    pub unsafe fn new(
        model: Model,
        cart: Cartridge,
        audio_callbacks: *mut dyn AudioCallbacks,
        video_callbacks: *mut dyn VideoCallbacks,
    ) -> Self {
        let function_mode = match model {
            Model::Dmg | Model::Mgb => FunctionMode::Dmg,
            Model::Cgb => FunctionMode::Cgb,
        };

        Self {
            reg: Regs::new(),
            ei_delay: false,
            ime: false,
            halted: false,
            halt_bug: false,
            ints: Interrupts::new(),
            timer: Timer::new(),
            cart,
            hram: Hram::new(),
            wram: Wram::new(),
            ppu: Ppu::new(video_callbacks),
            joypad: Joypad::new(),
            apu: Apu::new(audio_callbacks),
            serial: Serial::new(),
            dma: Dma::new(),
            hdma: Hdma::new(),
            boot_rom: BootRom::new(model),
            model,
            in_double_speed: false,
            key1: Key1::new(),
            function_mode,
        }
    }

    pub fn press(&mut self, button: Button) {
        self.joypad.press(&mut self.ints, button);
    }

    pub fn release(&mut self, button: Button) {
        self.joypad.release(button);
    }

    pub fn run_frame(&mut self) {
        for _ in 0..M_CYCLES_PER_FRAME {
            self.run();
        }
    }

    #[must_use]
    pub fn save_data(&self) -> Option<&[u8]> {
        self.cart.has_battery().then_some(self.cart.ram())
    }
}
