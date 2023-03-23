#![forbid(unsafe_code)]
#![warn(
    clippy::pedantic,
    clippy::nursery,
    // restriction
    clippy::alloc_instead_of_core,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    clippy::clone_on_ref_ptr,
    clippy::create_dir,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::default_union_representation,
    clippy::deref_by_slicing,
    clippy::else_if_without_else,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::exit,
    clippy::expect_used,
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
    clippy::mutex_atomic,
    clippy::non_ascii_literal,
    clippy::panic,
    clippy::partial_pub_fields,
    clippy::print_stderr,
    clippy::print_stdout,
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
    clippy::todo,
    clippy::try_err,
    clippy::unimplemented,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern,
    clippy::unseparated_literal_suffix,
    clippy::use_debug,
    clippy::verbose_file_reads,
    // clippy::indexing_slicing,
    // clippy::unwrap_used,
    // clippy::integer_division,
)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::missing_safety_doc,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::verbose_bit_mask
)]

use interrupts::Interrupts;
use joypad::Joypad;
use memory::{Key1, Svbk};
use serial::Serial;
use {apu::Apu, memory::HdmaState, ppu::Ppu, timing::TIMAState};
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
mod interrupts;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod timing;

// t-cycles per second
const TC_SEC: i32 = 0x40_0000;
const HRAM_SIZE: u8 = 0x80;
const WRAM_SIZE: u16 = 0x2000 * 4;

pub struct Gb {
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
    tima_state: TIMAState,
    wide_div_counter: u16,

    // peripherals
    ppu: Ppu,
    apu: Apu,
    serial: Serial,
    ints: Interrupts,
    joy: Joypad,
}

impl Gb {
    #[must_use]
    pub fn new(model: Model, sample_rate: i32, cart: Cart) -> Self {
        const DMG_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/dmg_boot.bin");
        const MGB_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/mgb_boot.bin");
        const CGB_BOOTROM: &[u8] = include_bytes!("../../../bootroms/bin/cgb_boot_fast.bin");

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
            apu: Apu::new(sample_rate),

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
            wide_div_counter: Default::default(),
        }
    }

    #[inline]
    pub fn run_samples(&mut self) -> (Sample, Sample) {
        while self.apu.samples_run() == 0 {
            self.run_cpu();
        }

        self.apu.reset_samples_run();

        self.apu.out()
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
