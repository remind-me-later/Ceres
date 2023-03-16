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

use serial::Serial;

use {apu::Apu, core::num::NonZeroU8, memory::HdmaState, ppu::Ppu, timing::TIMAState};
pub use {
    apu::Sample,
    cart::{Cart, Error},
    joypad::Button,
    ppu::{PX_HEIGHT, PX_WIDTH},
};

extern crate alloc;

mod apu;
mod cart;
mod cpu;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod timing;

// t-cycles per second
const TC_SEC: i32 = 0x40_0000;

const IF_VBLANK_B: u8 = 1;
const IF_LCD_B: u8 = 2;
const IF_TIMER_B: u8 = 4;
const IF_SERIAL_B: u8 = 8;
const IF_P1_B: u8 = 16;

const HRAM_SIZE: u8 = 0x80;
const WRAM_SIZE: u16 = 0x2000;
const WRAM_SIZE_CGB: u16 = WRAM_SIZE * 4;

#[derive(Clone, Copy)]
pub enum Model {
    Dmg,
    Mgb,
    Cgb,
}

#[derive(Clone, Copy)]
enum CMode {
    Dmg,
    Compat,
    Cgb,
}

pub struct Gb {
    // general
    model: Model,
    // CGB compatibility mode
    cmode: CMode,

    // double speed
    key1_ena: bool,
    key1_req: bool,

    // key1: u8,

    // cartridge
    cart: Cart,
    boot_rom: Option<&'static [u8]>,

    // cpu
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    pc: u16,

    ei_delay: bool,
    cpu_halted: bool,

    // serial
    serial: Serial,

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
    wram: [u8; WRAM_SIZE_CGB as usize],
    hram: [u8; HRAM_SIZE as usize],
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

    // clock
    tima: u8,
    tma: u8,
    tac: u8,

    tima_state: TIMAState,

    tac_enable: bool,
    wide_div_counter: u16,

    // ppu
    ppu: Ppu,

    // apu
    apu: Apu,
}

impl Gb {
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn new(model: Model, sample_rate: i32, cart: Cart) -> Self {
        const DMG_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/dmg_boot.bin");
        const MGB_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/mgb_boot.bin");
        const CGB_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/cgb_boot_fast.bin");

        let compat_mode = match model {
            Model::Dmg | Model::Mgb => CMode::Dmg,
            Model::Cgb => CMode::Cgb,
        };

        let boot_rom = Some(match model {
            Model::Dmg => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::Cgb => CGB_BOOTROM,
        });

        Self {
            model,
            cmode: compat_mode,
            cart,
            boot_rom,

            // Custom
            svbk_true: NonZeroU8::new(1).unwrap(),

            // Slices
            wram: [0; WRAM_SIZE_CGB as usize],
            hram: [0; HRAM_SIZE as usize],

            apu: Apu::new(sample_rate),

            // Default
            serial: Serial::default(),
            ppu: Ppu::default(),
            tima_state: TIMAState::default(),
            key1_ena: Default::default(),
            key1_req: Default::default(),
            af: Default::default(),
            bc: Default::default(),
            de: Default::default(),
            hl: Default::default(),
            sp: Default::default(),
            pc: Default::default(),
            ei_delay: Default::default(),
            cpu_halted: Default::default(),
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

            tima: Default::default(),
            tma: Default::default(),
            tac: Default::default(),
            tac_enable: Default::default(),
            wide_div_counter: Default::default(),
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
    pub fn cartridge(&mut self) -> &mut Cart {
        &mut self.cart
    }

    #[must_use]
    pub const fn pixel_data_rgba(&self) -> &[u8] {
        self.ppu.pixel_data_rgb()
    }
}
