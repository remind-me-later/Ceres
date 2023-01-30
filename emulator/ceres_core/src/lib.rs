#![no_std]
#![forbid(unsafe_code)]
#![warn(
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
    clippy::pedantic,
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
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::verbose_bit_mask
)]

use core::num::NonZeroU8;

#[cfg(feature = "disassembler")]
extern crate std;

extern crate alloc;

pub use {
    apu::Sample,
    cartridge::{Cartridge, InitializationError},
    joypad::Button,
    ppu::{PX_HEIGHT, PX_WIDTH},
};
use {
    apu::{Noise, Square1, Square2, Wave},
    core::time::Duration,
    memory::HdmaState,
    ppu::{ColorPalette, Mode, RgbBuf, OAM_SIZE, VRAM_SIZE_CGB},
};

mod apu;
mod cartridge;
mod cpu;
mod joypad;
mod memory;
mod ppu;
mod timing;

const DMG_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/dmg_boot.bin");
const MGB_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/mgb_boot.bin");
const CGB_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/cgb_boot_fast.bin");

const FRAME_NANOS: u64 = 16_750_418;
/// `GameBoy` frame duration in nanoseconds, the `GameBoy`
/// framerate is 59.7 fps.
pub const FRAME_DUR: Duration = Duration::from_nanos(FRAME_NANOS);
// t-cycles per second
const TC_SEC: i32 = 0x0040_0000;

const IF_VBLANK_B: u8 = 1;
const IF_LCD_B: u8 = 2;
const IF_TIMER_B: u8 = 4;
//const IF_SERIAL_B: u8 = 8;
const IF_P1_B: u8 = 16;

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

pub struct Gb {
    // general
    model: Model,
    compat_mode: CompatMode,
    // running_frame: bool,
    samples_run: usize,

    // double speed
    double_speed: bool,
    double_speed_request: bool,
    // key1: u8,

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
    svbk_true: NonZeroU8, // true selected bank, between 1 and 7

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
    vbk: bool,
    bcp: ColorPalette,
    ocp: ColorPalette,

    frame_dots: i32,
    lcdc_delay: bool,
    vram: [u8; VRAM_SIZE_CGB],
    oam: [u8; OAM_SIZE],
    rgb_buf: RgbBuf,
    rgb_buf_present: RgbBuf,
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

    apu_timer: i32,
    apu_render_timer: i32,
    apu_ext_sample_period: i32,
    apu_seq_step: u8,

    apu_l_out: Sample,
    apu_r_out: Sample,

    apu_cap: Sample,
}

impl Gb {
    #[allow(clippy::too_many_lines)]
    #[must_use]
    /// # Panics
    pub fn new(model: Model, sample_rate: i32, cart: Cartridge) -> Self {
        let compat_mode = match model {
            Model::Dmg | Model::Mgb => CompatMode::Dmg,
            Model::Cgb => CompatMode::Cgb,
        };

        let boot_rom = Some(match model {
            Model::Dmg => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::Cgb => CGB_BOOTROM,
        });

        Self {
            model,
            compat_mode,
            cart,
            boot_rom,

            // Custom
            svbk_true: NonZeroU8::new(1).unwrap(),
            ppu_cycles: Mode::HBlank.cycles(0),

            // Slices
            wram: [0; WRAM_SIZE_CGB],
            hram: [0; HRAM_SIZE],
            vram: [0; VRAM_SIZE_CGB],
            oam: [0; OAM_SIZE],

            // Sound
            apu_ext_sample_period: Self::sample_period_from_rate(sample_rate),

            // Default
            // running_frame: Default::default(),
            apu_cap: Default::default(),
            double_speed: Default::default(),
            double_speed_request: Default::default(),
            af: Default::default(),
            bc: Default::default(),
            de: Default::default(),
            hl: Default::default(),
            sp: Default::default(),
            pc: Default::default(),
            stolen_cycles: Default::default(),
            cpu_ei_delay: Default::default(),
            cpu_halted: Default::default(),
            sb: Default::default(),
            sc: Default::default(),
            p1_btn: Default::default(),
            p1_dirs: Default::default(),
            p1_acts: Default::default(),
            halt_bug: Default::default(),
            ime: Default::default(),
            ifr: Default::default(),
            ie: Default::default(),
            svbk: Default::default(),
            dma: Default::default(),
            dma_on: Default::default(),
            dma_addr: Default::default(),
            dma_restarting: Default::default(),
            dma_cycles: Default::default(),
            hdma5: Default::default(),
            hdma_src: Default::default(),
            hdma_dst: Default::default(),
            hdma_len: Default::default(),
            hdma_state: HdmaState::default(),
            lcdc: Default::default(),
            stat: Default::default(),
            scy: Default::default(),
            scx: Default::default(),
            ly: Default::default(),
            lyc: Default::default(),
            bgp: Default::default(),
            obp0: Default::default(),
            obp1: Default::default(),
            wy: Default::default(),
            wx: Default::default(),
            opri: Default::default(),
            vbk: Default::default(),
            bcp: ColorPalette::default(),
            ocp: ColorPalette::default(),
            frame_dots: Default::default(),
            lcdc_delay: Default::default(),
            rgb_buf: RgbBuf::default(),
            rgb_buf_present: RgbBuf::default(),
            ppu_win_in_frame: Default::default(),
            ppu_win_in_ly: Default::default(),
            ppu_win_skipped: Default::default(),
            tima: Default::default(),
            tma: Default::default(),
            tac: Default::default(),
            tac_enable: Default::default(),
            system_clk: Default::default(),
            nr51: Default::default(),
            apu_on: Default::default(),
            apu_r_vol: Default::default(),
            apu_l_vol: Default::default(),
            apu_r_vin: Default::default(),
            apu_l_vin: Default::default(),
            apu_ch1: Square1::default(),
            apu_ch2: Square2::default(),
            apu_ch3: Wave::default(),
            apu_ch4: Noise::default(),
            apu_timer: Default::default(),
            apu_render_timer: Default::default(),
            apu_seq_step: Default::default(),
            samples_run: Default::default(),
            apu_l_out: Default::default(),
            apu_r_out: Default::default(),
        }
    }

    const fn sample_period_from_rate(sample_rate: i32) -> i32 {
        // maybe account for difference between 59.7 and target Hz?
        TC_SEC / sample_rate
    }

    /// Runs 1 frame
    // #[inline]
    // pub fn run_frame(&mut self) {
    //     self.running_frame = true;

    //     while self.running_frame {
    //         self.run_cpu();
    //     }
    // }

    /// Runs samples
    #[inline]
    pub fn run_samples(&mut self) -> (Sample, Sample) {
        while self.samples_run == 0 {
            self.run_cpu();
        }

        self.samples_run = 0;

        (self.apu_l_out, self.apu_r_out)
    }

    #[must_use]
    pub fn cartridge_ram(&self) -> &[u8] {
        self.cart.ram()
    }

    #[must_use]
    pub fn pixel_data_rgb(&self) -> &[u8] {
        self.rgb_buf_present.pixel_data()
    }

    /// Returns true if cartridge has battery, false
    /// otherwise
    #[must_use]
    pub fn cartridge_has_battery(&self) -> bool {
        self.cart.has_battery()
    }
}
