#![no_std]
#![feature(core_intrinsics)]
// clippy
#![warn(clippy::pedantic)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::similar_names)]

use {
    apu::{Ch1, Ch2, HighPassFilter, Noise, Wave, TIMER_RESET_VALUE},
    mem::HdmaState,
};

extern crate alloc;

pub use {
    apu::{AudioCallbacks, Frame, Sample},
    cart::{Cartridge, Header},
    error::Error,
    joypad::Button,
    ppu::{VideoCallbacks, PX_HEIGHT, PX_WIDTH},
};
use {
    bootrom::BootRom,
    core::time::Duration,
    cpu::Regs,
    mem::{HIGH_RAM_SIZE, WRAM_SIZE_CGB},
    ppu::{ColorPalette, Mode, RgbaBuf, OAM_SIZE, VRAM_SIZE_CGB},
    serial::Serial,
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
    halted: bool,
    halt_bug: bool,
    key1: u8,
    cart: Cartridge,
    serial: Serial,
    boot_rom: BootRom,
    model: Model,
    double_speed: bool,
    function_mode: FunctionMode,

    // joypad
    joy_btns: u8,
    joy_dirs: bool,
    joy_acts: bool,

    // interrupts
    ime: bool,
    interrupt_flag: u8,
    interrupt_enable: u8,

    // memory
    wram: [u8; WRAM_SIZE_CGB],
    hram: [u8; HIGH_RAM_SIZE],
    svbk: u8,
    svbk_bank: u8, // true selected bank, between 1 and 7

    hdma_src: u16,
    hdma_dst: u16,
    hdma_len: u16,
    hdma_state: HdmaState,
    hdma5: u8, // stores only low 7 bits

    // ppu
    vram: [u8; VRAM_SIZE_CGB],
    oam: [u8; OAM_SIZE],
    cycles: i16,
    rgba_buf: RgbaBuf,
    win_in_frame: bool,
    win_in_ly: bool,
    win_lines_skipped: u16,
    video_callbacks: *mut dyn VideoCallbacks,
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
    bcp: ColorPalette,
    ocp: ColorPalette,
    vbk: u8, // 0 or 1

    // clock
    clock_on: bool,
    internal_counter: u16,

    // clock registers
    counter: u8,
    modulo: u8,
    tac: u8,
    overflow: bool,

    // apu
    ch1: Ch1,
    ch2: Ch2,
    ch3: Wave,
    ch4: Noise,

    // sequencer
    timer: u16,
    apu_counter: u8,

    apu_render_period: u32,
    apu_cycles_until_render: u32,
    audio_callbacks: *mut dyn AudioCallbacks,
    high_pass_filter: HighPassFilter,

    apu_on: bool,
    nr51: u8,
    right_volume: u8,
    left_volume: u8,
    right_vin_on: bool,
    left_vin_on: bool,

    // dma
    dma_on: bool,
    dma: u8,
    dma_addr: u16,
    dma_restarting: bool,
    dma_cycles: i8,
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

        let cycles_to_render = (*audio_callbacks).cycles_to_render();

        Self {
            reg: Regs::new(),
            ei_delay: false,
            ime: false,
            halted: false,
            halt_bug: false,
            cart,
            hram: [0; HIGH_RAM_SIZE],
            wram: [0; WRAM_SIZE_CGB],
            svbk: 0,
            svbk_bank: 1,
            serial: Serial::new(),
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
            win_lines_skipped: 0,
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
            bcp: ColorPalette::new(),
            ocp: ColorPalette::new(),
            opri: 0,
            vbk: 0,
            exit: false,
            interrupt_enable: 0,
            interrupt_flag: 0,
            clock_on: false,
            internal_counter: 0,
            counter: 0,
            modulo: 0,
            tac: 0,
            overflow: false,
            joy_btns: 0,
            joy_dirs: false,
            joy_acts: false,
            ch1: Ch1::new(),
            ch2: Ch2::new(),
            ch3: Wave::new(),
            ch4: Noise::new(),
            apu_render_period: cycles_to_render,
            apu_cycles_until_render: cycles_to_render,
            audio_callbacks,
            high_pass_filter: HighPassFilter::new(),
            apu_on: false,
            nr51: 0,
            right_volume: 0,
            left_volume: 0,
            right_vin_on: false,
            left_vin_on: false,
            timer: TIMER_RESET_VALUE,
            apu_counter: 0,
            dma_on: false,
            dma: 0,
            dma_addr: 0,
            dma_restarting: false,
            dma_cycles: 0,
            hdma_src: 0,
            hdma_dst: 0,
            hdma_len: 0,
            hdma_state: HdmaState::Sleep,
            hdma5: 0,
        }
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
