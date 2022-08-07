//! A library to make ``GameBoy`` Color emulators.
//! This library is pretty low level and uses unsafe to
//! avoid allocations.

#![no_std]
#![feature(const_maybe_uninit_zeroed)]
#![warn(
    // unsafe_code,
    clippy::as_underscore,
    clippy::clone_on_ref_ptr,
    clippy::decimal_literal_representation,
    clippy::deref_by_slicing,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::float_arithmetic,
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
    clippy::pedantic,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_slice,
    clippy::string_to_string,
    clippy::try_err,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern
)]
#![allow(
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::verbose_bit_mask
)]

#[cfg(feature = "disassembler")]
extern crate std;

extern crate std;

pub use {
    apu::Sample,
    cartridge::InitializationError,
    joypad::Button,
    ppu::{PX_HEIGHT, PX_WIDTH},
};
use {
    apu::{Noise, Square1, Square2, Wave},
    cartridge::Cartridge,
    core::{mem::MaybeUninit, time::Duration},
    memory::HdmaState,
    ppu::{ColorPalette, Mode, RgbaBuf, OAM_SIZE, VRAM_SIZE_CGB},
};

mod apu;
mod cartridge;
mod cpu;
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

pub static mut GAME_BOY: MaybeUninit<Gb> = MaybeUninit::zeroed();

/// The ``GameBoy`` struct is the main struct in the
/// library. The `run` method never returns and calls a PPU
/// "graphical" callback every frame and an APU "audio"
/// callback every sample. These callbacks are passed to
/// the `new` function, which returns a `GameBoy` struct.
pub struct Gb {
    // general
    model: Model,
    compat_mode: CompatMode,
    running_frame: bool,

    // double speed
    double_speed: bool,
    key1: u8,

    // cartridge
    cart: Cartridge,
    boot_rom: Option<&'static [u8]>,

    // cpu
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    pc: u16,

    stolen_cycles: i32,
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

    // clock
    tima: u8,
    tma: u8,
    tac: u8,

    tac_enable: bool,
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

    apu_timer: u16,
    apu_render_timer: u32,
    apu_ext_sample_period: u32,
    apu_callback: Option<fn(Sample, Sample)>,
    apu_seq_step: u8,
}

impl Gb {
    #[must_use]
    pub fn new(
        model: Model,
        apu_callback: fn(Sample, Sample),
        sample_rate: u32,
    ) -> &'static mut Self {
        let mut gb = unsafe { GAME_BOY.assume_init_mut() };

        // custom initilization
        gb.model = model;

        gb.compat_mode = match model {
            Model::Dmg | Model::Mgb => CompatMode::Dmg,
            Model::Cgb => CompatMode::Cgb,
        };

        gb.boot_rom = Some(match model {
            Model::Dmg => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::Cgb => CGB_BOOTROM,
        });

        gb.svbk_true = 1;
        gb.ppu_cycles = Mode::HBlank.cycles(0);

        gb.set_apu_callback(apu_callback);
        gb.set_sample_rate(sample_rate);

        // Default like
        gb.rgba_buf = RgbaBuf::default();
        gb.bcp = ColorPalette::default();
        gb.ocp = ColorPalette::default();
        gb.apu_ch1 = Square1::default();
        gb.apu_ch2 = Square2::default();
        gb.apu_ch3 = Wave::default();
        gb.apu_ch4 = Noise::default();
        gb.hdma_state = HdmaState::default();

        gb
    }

    /// # Errors
    ///
    /// Will return `Err` if the ROM header contains some
    /// unsupported MBC value. This can happen if the ROM is
    /// corrupt, has not been initialized or we simply don't
    /// support its MBC yet.
    pub fn init(&mut self) -> Result<(), InitializationError> {
        self.cart.init()
    }

    fn set_apu_callback(&mut self, apu_callback: fn(Sample, Sample)) {
        self.apu_callback = Some(apu_callback);
    }

    fn set_sample_rate(&mut self, sample_rate: u32) {
        // maybe account for difference between 59.7 and 60 Hz?
        let x = (600 * TC_SEC) / 597;
        self.apu_ext_sample_period = x / sample_rate;
    }

    /// Runs 1 frame
    #[inline]
    pub fn run_frame(&mut self) {
        self.running_frame = true;

        while self.running_frame {
            self.run_cpu();
        }
    }

    #[must_use]
    pub fn pixel_data(&self) -> &[u8] {
        self.rgba_buf.pixel_data()
    }

    /// Returns true if cartridge has battery, false
    /// otherwise
    #[must_use]
    pub fn cartridge_has_battery(&self) -> bool {
        self.cart.has_battery()
    }

    /// Returns reference to static RAM slice.
    #[must_use]
    pub fn cartridge_ram(&self) -> &[u8] {
        self.cart.ram()
    }

    /// Returns mutable reference to static RAM slice.
    ///
    /// Modifying the RAM contents while the Gb is running
    /// could lead to undesirable results.
    #[must_use]
    pub fn cartridge_ram_mut(&mut self) -> &mut [u8] {
        self.cart.mut_ram()
    }

    /// Returns mutable reference to static ROM slice.
    ///
    /// Modifying the ROM contents while the Gb is running
    /// could lead to undesirable results.
    #[must_use]
    pub fn cartridge_rom_mut(&mut self) -> &mut [u8] {
        self.cart.mut_rom()
    }

    // This is used for the test suite
    // The test is considered to be running
    // until registers have the following values
    // B = 3, C = 5, D = 8, E = 13, H = 21
    // L = 0 if test suceeded and non-zero otherwise
    pub fn test_running(&mut self) -> bool {
        // TODO: Make this understandable by humans
        self.bc != 773 || self.de != 2061 || (self.hl & 0xFF00) != 5376
    }

    pub fn get_test_result(&mut self) -> u16 {
        // l is the lower 8 bits of 16-bit register hl
        self.hl & 255
    }
}
