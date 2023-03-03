// #![no_std]
#![forbid(unsafe_code)]
#![warn(
  clippy::pedantic,
  clippy::nursery,
  // restriction
  clippy::alloc_instead_of_core,
  clippy::as_underscore,
  clippy::assertions_on_result_states,
  clippy::clone_on_ref_ptr,
  clippy::decimal_literal_representation,
  clippy::default_union_representation,
  clippy::deref_by_slicing,
  // clippy::else_if_without_else,
  clippy::empty_drop,
  clippy::empty_structs_with_brackets,
  clippy::exit,
  clippy::filetype_is_file,
  clippy::float_arithmetic,
  clippy::float_cmp_const,
  clippy::fn_to_numeric_cast_any,
  clippy::format_push_string,
  clippy::get_unwrap,
  clippy::if_then_some_else_none,
  clippy::let_underscore_must_use,
  clippy::lossy_float_literal,
  clippy::map_err_ignore,
  clippy::mem_forget,
  clippy::mixed_read_write_in_expression,
  clippy::modulo_arithmetic,
  clippy::non_ascii_literal,
  clippy::panic,
  clippy::partial_pub_fields,
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
  clippy::unneeded_field_pattern,
  clippy::unseparated_literal_suffix,
  // clippy::unwrap_used,
  clippy::verbose_file_reads,
)]
#![allow(
  clippy::struct_excessive_bools,
  clippy::verbose_bit_mask,
  clippy::missing_errors_doc,
  clippy::missing_panics_doc,
  clippy::missing_safety_doc,
  clippy::similar_names
)]
#![feature(error_in_core, negative_impls)]

extern crate alloc;

use {
  apu::Apu,
  core::{num::NonZeroU8, time::Duration},
  memory::HdmaState,
  ppu::{ColorPalette, Mode, RgbBuf, OAM_SIZE, VRAM_SIZE_CGB},
};
pub use {
  apu::Sample,
  cartridge::{Cartridge, Error},
  joypad::Button,
  ppu::{PX_HEIGHT, PX_WIDTH},
};

mod apu;
mod cartridge;
mod cpu;
mod joypad;
mod memory;
mod ppu;
mod timing;

const FRAME_NANOS: u64 = 16_750_418;
// frame duration in nanoseconds, the GameBoy framerate is 59.7 fps.
pub const FRAME_DUR: Duration = Duration::from_nanos(FRAME_NANOS);
// t-cycles per second
const TC_SEC: i32 = 0x0040_0000;

const IF_VBLANK_B: u8 = 1;
const IF_LCD_B: u8 = 2;
const IF_TIMER_B: u8 = 4;
// const IF_SERIAL_B: u8 = 8;
const IF_P1_B: u8 = 16;

const HRAM_SIZE: usize = 0x80;
const WRAM_SIZE: usize = 0x2000;
const WRAM_SIZE_CGB: usize = WRAM_SIZE * 4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Model {
  Dmg,
  Mgb,
  Cgb,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompatMode {
  Dmg,
  Compat,
  Cgb,
}

pub struct Gb {
  // general
  model:       Model,
  compat_mode: CompatMode,

  // double speed
  double_speed:         bool,
  double_speed_request: bool,

  // key1: u8,

  // cartridge
  cart:     Cartridge,
  boot_rom: Option<&'static [u8]>,

  // cpu
  af: u16,
  bc: u16,
  de: u16,
  hl: u16,
  sp: u16,
  pc: u16,

  cpu_ei_delay: bool,
  cpu_halted:   bool,

  // serial
  sb: u8,
  sc: u8,

  // joypad
  p1_btn:  u8,
  p1_dirs: bool,
  p1_acts: bool,

  // interrupts
  halt_bug: bool,
  ime:      bool,
  ifr:      u8,
  ie:       u8,

  // memory
  wram:      [u8; WRAM_SIZE_CGB],
  hram:      [u8; HRAM_SIZE],
  svbk:      u8,
  svbk_true: NonZeroU8, // true selected bank, between 1 and 7

  // -- dma
  dma:            u8,
  dma_on:         bool,
  dma_addr:       u16,
  dma_restarting: bool,
  dma_cycles:     i32,

  // -- hdma
  hdma5:      u8,
  hdma_src:   u16,
  hdma_dst:   u16,
  hdma_len:   u16,
  hdma_state: HdmaState,

  // ppu
  lcdc: u8,
  stat: u8,
  scy:  u8,
  scx:  u8,
  ly:   u8,
  lyc:  u8,
  bgp:  u8,
  obp0: u8,
  obp1: u8,
  wy:   u8,
  wx:   u8,
  opri: u8,
  vbk:  bool,
  bcp:  ColorPalette,
  ocp:  ColorPalette,

  frame_dots:       i32,
  lcdc_delay:       bool,
  vram:             [u8; VRAM_SIZE_CGB],
  oam:              [u8; OAM_SIZE],
  rgb_buf:          RgbBuf,
  rgb_buf_present:  RgbBuf,
  ppu_cycles:       i32,
  ppu_win_in_frame: bool,
  ppu_win_in_ly:    bool,
  ppu_win_skipped:  u8,

  // clock
  tima: u8,
  tma:  u8,
  tac:  u8,

  tac_enable: bool,
  system_clk: u16,

  apu: Apu,
}

impl Gb {
  #[allow(clippy::too_many_lines)]
  #[must_use]
  pub fn new(model: Model, sample_rate: i32, cart: Cartridge) -> Self {
    const DMG_BOOTROM: &[u8] =
      include_bytes!("../../../bootroms/bin/dmg_boot.bin");
    const MGB_BOOTROM: &[u8] =
      include_bytes!("../../../bootroms/bin/mgb_boot.bin");
    const CGB_BOOTROM: &[u8] =
      include_bytes!("../../../bootroms/bin/cgb_boot_fast.bin");

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

      apu: Apu::new(sample_rate),

      // Default
      double_speed: Default::default(),
      double_speed_request: Default::default(),
      af: Default::default(),
      bc: Default::default(),
      de: Default::default(),
      hl: Default::default(),
      sp: Default::default(),
      pc: Default::default(),
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
    }
  }

  pub fn run_samples(&mut self) -> (Sample, Sample) {
    while self.apu.samples_run() == 0 {
      self.run_cpu();
    }

    self.apu.reset_samples_run();

    self.apu.out()
  }

  #[must_use]
  pub fn cartridge(&mut self) -> &mut Cartridge { &mut self.cart }

  #[must_use]
  pub const fn pixel_data_rgb(&self) -> &[u8] {
    self.rgb_buf_present.pixel_data()
  }
}
