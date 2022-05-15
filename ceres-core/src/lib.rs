#![no_std]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::similar_names)]

use {
    mem::{HIGH_RAM_SIZE, WRAM_SIZE_CGB},
    ppu::{ColorPalette, Mode, RgbaBuf, OAM_SIZE, VRAM_SIZE_CGB},
};

extern crate alloc;

use {
    apu::Apu,
    bootrom::BootRom,
    core::time::Duration,
    cpu::Regs,
    interrupts::Interrupts,
    joypad::Joypad,
    mem::{Dma, Hdma},
    serial::Serial,
    timer::Timer,
};
pub use {
    apu::{AudioCallbacks, Frame, Sample},
    cart::{Cartridge, Header},
    error::Error,
    joypad::Button,
    ppu::{VideoCallbacks, PX_HEIGHT, PX_WIDTH},
};

mod apu;
mod bootrom;
mod cart;
mod cpu;
mod error;
mod interrupts;
mod joypad;
mod mem;
mod ppu;
mod serial;
mod timer;

// 59.7 fps
pub const FRAME_DURATION: Duration = Duration::from_nanos(NANOS_PER_FRAME);
const NANOS_PER_FRAME: u64 = 16_750_418;
const T_CYCLES_PER_SECOND: u32 = 4_194_304;

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
    exit: bool,
    reg: Regs,
    ei_delay: bool,
    ime: bool,
    halted: bool,
    halt_bug: bool,
    key1: u8,
    ints: Interrupts,
    joypad: Joypad,
    cart: Cartridge,
    timer: Timer,
    hram: [u8; HIGH_RAM_SIZE],
    apu: Apu,
    serial: Serial,
    dma: Dma,
    hdma: Hdma,
    boot_rom: BootRom,
    model: Model,
    double_speed: bool,
    function_mode: FunctionMode,
    wram: [u8; WRAM_SIZE_CGB],
    svbk: u8,
    svbk_bank: u8, // true selected bank, between 1 and 7
    // ppu
    vram: [u8; VRAM_SIZE_CGB],
    oam: [u8; OAM_SIZE],
    cycles: i16,
    rgba_buf: RgbaBuf,
    win_in_frame: bool,
    win_in_ly: bool,
    window_lines_skipped: u16,
    video_callbacks: *mut dyn VideoCallbacks,
    // ppu registers
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    wy: u8,
    wx: u8,
    opri: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    cgb_bg_palette: ColorPalette,
    cgb_obj_palette: ColorPalette,
    vbk: u8, // 0 or 1
}

impl Gb {
    /// # Safety
    ///
    /// `audio_callbacks` and `video_callbacks` should not be dropped before the struct
    /// and be in the heap, so that their position doesn't change
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
            hram: [0; HIGH_RAM_SIZE],
            wram: [0; WRAM_SIZE_CGB],
            svbk: 0,
            svbk_bank: 1,
            joypad: Joypad::new(),
            apu: Apu::new(audio_callbacks),
            serial: Serial::new(),
            dma: Dma::new(),
            hdma: Hdma::new(),
            boot_rom: BootRom::new(model),
            model,
            double_speed: false,
            key1: 0,
            function_mode,
            vram: [0; VRAM_SIZE_CGB],
            oam: [0; OAM_SIZE],
            rgba_buf: RgbaBuf::new(),
            cycles: Mode::HBlank.cycles(0),
            win_in_frame: false,
            window_lines_skipped: 0,
            win_in_ly: false,
            video_callbacks,
            lcdc: 0,
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            wy: 0,
            wx: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            cgb_bg_palette: ColorPalette::new(),
            cgb_obj_palette: ColorPalette::new(),
            opri: 0,
            vbk: 0,
            exit: false,
        }
    }

    pub fn press(&mut self, button: Button) {
        self.joypad.press(&mut self.ints, button);
    }

    pub fn release(&mut self, button: Button) {
        self.joypad.release(button);
    }

    pub fn run_frame(&mut self) {
        self.exit = false;

        while !self.exit {
            self.run();
        }
    }

    #[must_use]
    pub fn save_data(&self) -> Option<&[u8]> {
        self.cart.has_battery().then_some(self.cart.ram())
    }
}
