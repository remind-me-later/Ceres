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
#![allow(clippy::too_many_lines)]

extern crate alloc;

pub use {
    apu::{AudioCallbacks, Sample},
    cart::Cartridge,
    error::Error,
    joypad::Button,
    ppu::{VideoCallbacks, PX_HEIGHT, PX_WIDTH},
};
use {
    apu::{Ch1, Ch2, Noise, Wave, APU_TIMER_RES},
    bootrom::BootRom,
    core::time::Duration,
    cpu::Regs,
    mem::{HdmaState, HIGH_RAM_SIZE, WRAM_SIZE_CGB},
    ppu::{ColorPalette, Mode, RgbaBuf, OAM_SIZE, VRAM_SIZE_CGB},
    serial::Serial,
};

mod apu;
mod bootrom;
mod cart;
mod cpu;
mod error;
mod joypad;
mod mem;
mod ppu;
mod serial;
mod timer;

const FRAME_NANOS: u64 = 16_750_418;
// 59.7 fps
pub const FRAME_DUR: Duration = Duration::from_nanos(FRAME_NANOS);
// t-cycles per second
const TC_SEC: u32 = 4_194_304;

const IF_VBLANK_B: u8 = 1;
const IF_LCD_B: u8 = 2;
const IF_TIMER_B: u8 = 4;
const IF_SERIAL_B: u8 = 8;
const IF_P1_B: u8 = 0x10;

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

// IO addresses
const IO_TIMA: u8 = 0x05;
const IO_TMA: u8 = 0x06;
const IO_TAC: u8 = 0x07;

const IO_LCDC: u8 = 0x40;
const IO_STAT: u8 = 0x41;
const IO_SCY: u8 = 0x42;
const IO_SCX: u8 = 0x43;
const IO_LY: u8 = 0x44;
const IO_LYC: u8 = 0x45;
const IO_DMA: u8 = 0x46;
const IO_BGP: u8 = 0x47;
const IO_OBP0: u8 = 0x48;
const IO_OBP1: u8 = 0x49;
const IO_WY: u8 = 0x4a;
const IO_WX: u8 = 0x4b;

const IO_OPRI: u8 = 0x6c;

const IO_IF: u8 = 0x0f;

const IO_HDMA5: u8 = 0x55;

const IO_NR51: u8 = 0x25;

pub struct Gb {
    // general
    exit_run: bool,
    cart: Cartridge,
    brom: BootRom,
    model: Model,
    function_mode: FunctionMode,

    double_speed: bool,
    key1: u8,

    // cpu
    reg: Regs,
    cpu_ei_delay: bool,
    cpu_halted: bool,
    cpu_halt_bug: bool,

    // serial
    serial: Serial,

    // joypad
    p1_btn: u8,
    p1_dirs: bool,
    p1_acts: bool,

    // interrupts
    ime: bool,
    ie: u8,

    // memory
    wram: [u8; WRAM_SIZE_CGB],
    hram: [u8; HIGH_RAM_SIZE],
    // TODO: move everything here
    io: [u8; 0x70],
    svbk: u8,
    svbk_true: u8, // true selected bank, between 1 and 7

    // -- dma
    dma_on: bool,
    dma_addr: u16,
    dma_restarting: bool,
    dma_cycles: i8,

    // -- hdma
    hdma_src: u16,
    hdma_dst: u16,
    hdma_len: u16,
    hdma_state: HdmaState,

    // ppu
    vram: [u8; VRAM_SIZE_CGB],
    oam: [u8; OAM_SIZE],
    rgba_buf: RgbaBuf,
    ppu_cycles: i16,
    ppu_win_in_frame: bool,
    ppu_win_in_ly: bool,
    ppu_win_skipped: u16,
    ppu_callbacks: *mut dyn VideoCallbacks,
    bcp: ColorPalette,
    ocp: ColorPalette,
    vbk: u8,

    // clock
    clk_on: bool,
    clk_overflow: bool,
    clk_wide: u16,

    // apu
    apu_on: bool,
    apu_r_vol: u8,
    apu_l_vol: u8,
    apu_r_vin: bool,
    apu_l_vin: bool,

    // -- channels
    apu_ch1: Ch1,
    apu_ch2: Ch2,
    apu_ch3: Wave,
    apu_ch4: Noise,

    // -- sequencer
    apu_timer: u16,
    apu_counter: u8,

    apu_render_period: u32,
    apu_cycles_until_render: u32,
    apu_callbacks: *mut dyn AudioCallbacks,
    apu_cap: f32,
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
            io: [0; 0x70],
            reg: Regs::new(),
            cpu_ei_delay: false,
            ime: false,
            cpu_halted: false,
            cpu_halt_bug: false,
            cart,
            hram: [0; HIGH_RAM_SIZE],
            wram: [0; WRAM_SIZE_CGB],
            vbk: 0,
            svbk: 0,
            svbk_true: 1,
            serial: Serial::new(),
            brom: BootRom::new(model),
            model,
            double_speed: false,
            key1: 0,
            function_mode,
            vram: [0; VRAM_SIZE_CGB],
            oam: [0; OAM_SIZE],
            rgba_buf: RgbaBuf::new(),
            ppu_cycles: Mode::HBlank.cycles(0),
            ppu_win_in_frame: false,
            ppu_win_skipped: 0,
            ppu_win_in_ly: false,
            ppu_callbacks: video_callbacks,
            bcp: ColorPalette::new(),
            ocp: ColorPalette::new(),
            exit_run: false,
            clk_on: false,
            clk_wide: 0,
            clk_overflow: false,
            p1_btn: 0,
            p1_dirs: false,
            p1_acts: false,
            apu_ch1: Ch1::new(),
            apu_ch2: Ch2::new(),
            apu_ch3: Wave::new(),
            apu_ch4: Noise::new(),
            apu_render_period: cycles_to_render,
            apu_cycles_until_render: cycles_to_render,
            apu_callbacks: audio_callbacks,
            apu_on: false,
            apu_r_vol: 0,
            apu_l_vol: 0,
            apu_r_vin: false,
            apu_l_vin: false,
            apu_timer: APU_TIMER_RES,
            apu_counter: 0,
            dma_on: false,
            dma_addr: 0,
            dma_restarting: false,
            dma_cycles: 0,
            hdma_src: 0,
            hdma_dst: 0,
            hdma_len: 0,
            hdma_state: HdmaState::Sleep,
            apu_cap: 0.0,
            ie: 0,
        }
    }

    pub fn run_frame(&mut self) {
        self.exit_run = false;

        while !self.exit_run {
            self.run();
        }
    }

    #[must_use]
    pub fn save_data(&self) -> Option<&[u8]> {
        self.cart.has_battery().then_some(self.cart.ram())
    }
}
