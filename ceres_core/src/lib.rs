//! A library to make ``GameBoy`` Color emulators.
//! This library is pretty low level and uses unsafe to
//! avoid allocations.

//#![no_std]
#![feature(core_intrinsics)]
#![feature(const_maybe_uninit_zeroed)]
#![feature(negative_impls)]
#![warn(
    clippy::pedantic,
    clippy::as_underscore,
    clippy::clone_on_ref_ptr,
    clippy::decimal_literal_representation,
    clippy::deref_by_slicing,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::float_cmp_const,
    clippy::fn_to_numeric_cast_any,
    clippy::get_unwrap,
    clippy::if_then_some_else_none,
    clippy::let_underscore_must_use,
    clippy::lossy_float_literal,
    clippy::map_err_ignore,
    clippy::mem_forget,
    clippy::mixed_read_write_in_expression,
    clippy::modulo_arithmetic,
    clippy::non_ascii_literal,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_slice,
    clippy::string_to_string,
    clippy::try_err,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern
)]
#![allow(
    clippy::struct_excessive_bools,
    clippy::similar_names,
    clippy::verbose_bit_mask
)]

// For internal debugging purposes.
#[cfg(feature = "debugging_capability")]
extern crate std;

use {
    apu::{Noise, Square1, Square2, Wave},
    core::time::Duration,
    memory::HdmaState,
    ppu::{ColorPalette, Mode, RgbaBuf, OAM_SIZE, VRAM_SIZE_CGB},
};
pub use {
    cartridge::Cartridge,
    error::CartridgeInitError,
    joypad::Button,
    ppu::{PX_HEIGHT, PX_WIDTH},
};

mod apu;
mod cartridge;
mod cpu;
mod error;
mod joypad;
mod memory;
mod ppu;
mod timing;

const DMG_BOOTROM: &[u8] = include_bytes!("../bootroms/bin/dmg_boot.bin");
const MGB_BOOTROM: &[u8] = include_bytes!("../bootroms/bin/mgb_boot.bin");
const CGB_BOOTROM: &[u8] = include_bytes!("../bootroms/bin/cgb_boot_fast.bin");

const FRAME_NANOS: u64 = 16_750_418;
/// `GameBoy` frame duration in nanoseconds, the `GameBoy`
/// framerate is 59.7 fps.
pub const FRAME_DUR: Duration = Duration::from_nanos(FRAME_NANOS);
// t-cycles per second
const TC_SEC: u32 = 0x0040_0000;

const IF_VBLANK_B: u8 = 1;
const IF_LCD_B: u8 = 2;
const IF_TIMER_B: u8 = 4;
//const IF_SERIAL_B: u8 = 8;
const IF_P1_B: u8 = 0x10;

const KEY1_SPEED_B: u8 = 0x80;
const KEY1_SWITCH_B: u8 = 1;
const HRAM_SIZE: usize = 0x80;
const WRAM_SIZE: usize = 0x2000;
const WRAM_SIZE_CGB: usize = WRAM_SIZE * 4;

/// Audio sample type.
pub type Sample = f32;

/// ``GameBoy`` model to emulate.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Dmg,
    Mgb,
    Cgb,
}

#[derive(Clone, Copy)]
enum CompatMode {
    Dmg,
    Compat,
    Cgb,
}

/// The ``GameBoy`` struct is the main struct in the
/// library. The `run` method never returns and calls a PPU
/// "graphical" callback every frame and an APU "audio"
/// callback every sample. These callbacks are passed to
/// the `new` function, which returns a `GameBoy` struct.
pub struct Gb {
    // general
    cart: &'static mut Cartridge,
    model: Model,
    compat_mode: CompatMode,

    double_speed: bool,
    key1: u8,

    // boot rom
    boot_rom: &'static [u8],
    boot_rom_mapped: bool,

    // cpu
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    pc: u16,

    delay_cycles: i32,
    cpu_ei_delay: bool,
    cpu_halted: bool,

    // serial
    sb: u8,
    sc: u8,

    // joypad
    p1_btn: u8,
    p1_dirs: bool,
    p1_acts: bool,

    // interrupts
    halt_bug: bool,
    ime: bool,
    ifr: u8,
    ie: u8,

    // memory
    wram: [u8; WRAM_SIZE_CGB],
    hram: [u8; HRAM_SIZE],
    svbk: u8,
    svbk_true: u8, // true selected bank, between 1 and 7

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

    // ppu
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    opri: u8,
    vbk: u8,
    bcp: ColorPalette,
    ocp: ColorPalette,

    frame_dots: i32,
    lcdc_delay: bool,
    vram: [u8; VRAM_SIZE_CGB],
    oam: [u8; OAM_SIZE],
    rgba_buf: RgbaBuf,
    ppu_cycles: i32,
    ppu_win_in_frame: bool,
    ppu_win_in_ly: bool,
    ppu_win_skipped: u8,
    ppu_frame_callback: Option<fn(*const u8)>,

    // clock
    tima: u8,
    tma: u8,
    tac: u8,

    tac_enable: bool,
    tima_overflow: bool,
    system_clk: u16,

    // apu
    nr51: u8,

    apu_on: bool,
    apu_r_vol: u8,
    apu_l_vol: u8,
    apu_r_vin: bool,
    apu_l_vin: bool,

    apu_ch1: Square1,
    apu_ch2: Square2,
    apu_ch3: Wave,
    apu_ch4: Noise,

    apu_cap: f32,
    apu_timer: u16,
    apu_render_timer: u32,
    apu_ext_sample_period: u32,
    apu_frame_callback: Option<fn(Sample, Sample)>,
    apu_seq_step: u8,
}

impl Gb {
    /// Create a new ``GameBoy`` instance, specifying a PPU
    /// callback, an APU callback and a sampling rate. Of
    /// course, a cartridge is also needed.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(
        model: Model,
        cart: &'static mut Cartridge,
        ppu_frame_callback: fn(*const u8),
        apu_frame_callback: fn(Sample, Sample),
        sample_rate: u32,
    ) -> Self {
        let function_mode = match model {
            Model::Dmg | Model::Mgb => CompatMode::Dmg,
            Model::Cgb => CompatMode::Cgb,
        };

        let boot_rom = match model {
            Model::Dmg => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::Cgb => CGB_BOOTROM,
        };

        let mut gb = Self {
            halt_bug: false,
            apu_cap: 0.0,
            lcdc_delay: false,
            frame_dots: 0,
            af: 0,
            bc: 0,
            de: 0,
            hl: 0,
            delay_cycles: 0,
            sb: 0,
            sc: 0,
            opri: 0,
            nr51: 0,
            hdma5: 0,
            ifr: 0,
            cpu_ei_delay: false,
            ime: false,
            cpu_halted: false,
            cart,
            hram: [0; HRAM_SIZE],
            wram: [0; WRAM_SIZE_CGB],
            vbk: 0,
            svbk: 0,
            svbk_true: 1,
            model,
            double_speed: false,
            key1: 0,
            compat_mode: function_mode,
            vram: [0; VRAM_SIZE_CGB],
            oam: [0; OAM_SIZE],
            rgba_buf: RgbaBuf::new(),
            ppu_cycles: Mode::HBlank.cycles(0),
            ppu_win_in_frame: false,
            ppu_win_skipped: 0,
            ppu_win_in_ly: false,
            ppu_frame_callback: None,
            bcp: ColorPalette::new(),
            ocp: ColorPalette::new(),
            tac_enable: false,
            system_clk: 0,
            tima_overflow: false,
            p1_btn: 0,
            p1_dirs: false,
            p1_acts: false,
            apu_ch1: Square1::new(),
            apu_ch2: Square2::new(),
            apu_ch3: Wave::new(),
            apu_ch4: Noise::new(),
            apu_render_timer: 0,
            apu_on: false,
            apu_r_vol: 0,
            apu_l_vol: 0,
            apu_r_vin: false,
            apu_l_vin: false,
            apu_timer: 0,
            apu_seq_step: 0,
            dma_on: false,
            dma_addr: 0,
            dma_restarting: false,
            dma_cycles: 0,
            hdma_src: 0,
            hdma_dst: 0,
            hdma_len: 0,
            hdma_state: HdmaState::Sleep,
            ie: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            lcdc: 0,
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            dma: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            sp: 0,
            pc: 0,
            boot_rom,
            boot_rom_mapped: true,
            apu_ext_sample_period: 0,
            apu_frame_callback: None,
        };

        gb.set_ppu_frame_callback(ppu_frame_callback);
        gb.set_apu_frame_callback(apu_frame_callback);
        gb.set_sample_rate(sample_rate);

        gb
    }

    fn set_ppu_frame_callback(&mut self, ppu_frame_callback: fn(*const u8)) {
        self.ppu_frame_callback = Some(ppu_frame_callback);
    }

    fn set_apu_frame_callback(&mut self, apu_frame_callback: fn(Sample, Sample)) {
        self.apu_frame_callback = Some(apu_frame_callback);
    }

    fn set_sample_rate(&mut self, sample_rate: u32) {
        // maybe account for difference between 59.7 and 60 Hz?
        self.apu_ext_sample_period = TC_SEC / sample_rate;
    }

    /// Initiates the main emulation loop, never returns.
    #[inline]
    pub fn run(&mut self) -> ! {
        self.run_cpu();
    }

    /// Returns cartridge RAM if cartridge has battery, null
    /// otherwise
    #[must_use]
    pub fn save_data(&self) -> Option<&[u8]> {
        self.cart.has_battery().then_some(self.cart.ram())
    }
}
