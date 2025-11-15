//! # Ceres Game Boy and Game Boy Color Emulator Core
//!
//! Ceres is an experimental Game Boy and Game Boy Color emulator written in Rust.
//! This crate contains the core emulation logic including CPU, PPU, APU, memory management,
//! and cartridge handling.
//!
//! ## Tracing and Debugging
//!
//! This crate uses the standard Rust `tracing` crate for execution tracing.
//! To enable tracing, configure a tracing subscriber in your application:
//!
//! ```rust,ignore
//! // Example of how to configure tracing in your application:
//! use tracing_subscriber::{fmt, EnvFilter};
//!
//! tracing::subscriber::with_default(
//!     fmt::Subscriber::builder()
//!         .with_env_filter(EnvFilter::from_default_env())
//!         .json() // For JSON output format
//!         .finish(),
//!     || {
//!         // Your emulator code here
//!     }
//! );
//! ```
//!
//! With tracing enabled and the `cpu_execution` target filtered, you will receive
//! detailed logs of each executed instruction.

// #![no_std]
// FIXME: https://github.com/rust-lang/rust/issues/137578

extern crate alloc;

mod apu;
mod bess;
mod bootrom;
mod cartridge;
#[cfg(feature = "game_genie")]
mod cheats;
pub mod disasm;
mod error;
mod interrupts;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod sm83;
mod timing;
pub mod trace;

use crate::{
    bootrom::Bootrom,
    memory::{Hram, Wram},
    timing::DOTS_PER_FRAME,
};
use alloc::{boxed::Box, vec::Vec};
use cartridge::Cartridge;
#[cfg(feature = "game_genie")]
use cheats::GameGenie;
#[cfg(feature = "game_genie")]
pub use cheats::GameGenieCode;
use interrupts::Interrupts;
use joypad::Joypad;
use memory::Key1;
use serial::Serial;
use {apu::Apu, ppu::Ppu};
pub use {
    apu::{AudioCallback, Sample},
    error::Error,
    joypad::Button,
    ppu::ColorCorrectionMode,
    ppu::{PX_HEIGHT, PX_WIDTH},
    timing::FRAME_DURATION,
};
use {
    memory::{Dma, Hdma},
    sm83::Sm83,
    timing::Clock,
};

pub struct Gb<A: AudioCallback> {
    apu: Apu<A>,
    bootrom: Bootrom,
    cart: Cartridge,
    cgb_mode: CgbMode,
    clock: Clock,
    cpu: Sm83,
    dma: Dma,
    dots_ran: i32,
    #[cfg(feature = "game_genie")]
    game_genie: GameGenie,
    hdma: Hdma,
    hram: Hram,
    ints: Interrupts,
    joy: Joypad,
    key1: Key1,
    ld_b_b_breakpoint: bool,
    model: Model,
    ppu: Ppu,
    serial: Serial,
    trace_enabled: bool,
    trace_end_pc: Option<u16>,
    trace_start_pc: Option<u16>,
    wram: Wram,
}

impl<A: AudioCallback> Gb<A> {
    /// Activates a Game Genie code.
    ///
    /// # Errors
    ///
    /// Returns an error if too many codes are activated.
    #[inline]
    #[cfg(feature = "game_genie")]
    pub const fn activate_game_genie(&mut self, code: GameGenieCode) -> Result<(), Error> {
        self.game_genie.activate_code(code)
    }

    #[inline]
    pub fn active_game_genie_codes(&self) -> &[GameGenieCode] {
        self.game_genie.active_codes()
    }

    #[inline]
    pub const fn cart_has_battery(&self) -> bool {
        self.cart.has_battery()
    }

    #[inline]
    pub const fn cart_header_checksum(&self) -> u8 {
        self.cart.header_checksum()
    }

    #[inline]
    pub fn cart_title(&self) -> &[u8] {
        self.cart.ascii_title()
    }

    #[inline]
    pub const fn cart_version(&self) -> u8 {
        self.cart.version()
    }

    #[inline]
    pub fn change_model_and_soft_reset(&mut self, model: Model) {
        self.model = model;
        self.cgb_mode = model.into();
        self.bootrom = Bootrom::new(model);
        self.soft_reset();
    }

    /// Check if the `ld b, b` debug breakpoint instruction was executed and reset the flag.
    ///
    /// Some test ROMs (like cgb-acid2 and dmg-acid2) use the `ld b, b` instruction (opcode 0x40)
    /// as a debug breakpoint to signal test completion. This method returns `true` if the
    /// instruction has been executed since the last check, then automatically resets the flag.
    ///
    /// # Returns
    ///
    /// `true` if `ld b, b` was executed since the last check, `false` otherwise.
    #[inline]
    pub const fn check_and_reset_ld_b_b_breakpoint(&mut self) -> bool {
        let was_set = self.ld_b_b_breakpoint;
        self.ld_b_b_breakpoint = false;
        was_set
    }

    /// Read the current value of CPU register B.
    ///
    /// This is primarily used for test validation in test ROMs like the Mooneye Test Suite,
    /// which use specific register values to signal pass/fail status.
    #[must_use]
    #[inline]
    pub const fn cpu_b(&self) -> u8 {
        (self.cpu.bc() >> 8) as u8
    }

    /// Read the current value of CPU register C.
    ///
    /// This is primarily used for test validation in test ROMs like the Mooneye Test Suite,
    /// which use specific register values to signal pass/fail status.
    #[must_use]
    #[inline]
    pub const fn cpu_c(&self) -> u8 {
        (self.cpu.bc() & 0xFF) as u8
    }

    /// Read the current value of CPU register D.
    ///
    /// This is primarily used for test validation in test ROMs like the Mooneye Test Suite,
    /// which use specific register values to signal pass/fail status.
    #[must_use]
    #[inline]
    pub const fn cpu_d(&self) -> u8 {
        (self.cpu.de() >> 8) as u8
    }

    /// Read the current value of CPU register E.
    ///
    /// This is primarily used for test validation in test ROMs like the Mooneye Test Suite,
    /// which use specific register values to signal pass/fail status.
    #[must_use]
    #[inline]
    pub const fn cpu_e(&self) -> u8 {
        (self.cpu.de() & 0xFF) as u8
    }

    /// Read the current value of CPU register H.
    ///
    /// This is primarily used for test validation in test ROMs like the Mooneye Test Suite,
    /// which use specific register values to signal pass/fail status.
    #[must_use]
    #[inline]
    pub const fn cpu_h(&self) -> u8 {
        (self.cpu.hl() >> 8) as u8
    }

    /// Read the current value of CPU register L.
    ///
    /// This is primarily used for test validation in test ROMs like the Mooneye Test Suite,
    /// which use specific register values to signal pass/fail status.
    #[must_use]
    #[inline]
    pub const fn cpu_l(&self) -> u8 {
        (self.cpu.hl() & 0xFF) as u8
    }

    #[inline]
    #[cfg(feature = "game_genie")]
    pub fn deactivate_game_genie(&mut self, code: &GameGenieCode) {
        self.game_genie.deactivate_code(code);
    }

    /// Disassemble the instruction at the specified address.
    ///
    /// Reads up to 3 bytes from memory at the given address and returns
    /// the disassembled instruction as a structured `Instruction` and its length.
    ///
    /// # Arguments
    ///
    /// * `addr` - The memory address to disassemble
    ///
    /// # Returns
    ///
    /// An `Option` containing a tuple of the `Instruction` and instruction length,
    /// or `None` if the input is empty
    #[must_use]
    #[inline]
    pub fn disasm_at(&self, addr: u16) -> Option<(disasm::Instruction, u8)> {
        // Read up to 3 bytes for the instruction
        let b0 = self.read_mem(addr);
        let b1 = self.read_mem(addr.wrapping_add(1));
        let b2 = self.read_mem(addr.wrapping_add(2));

        disasm::disassemble(&[b0, b1, b2])
    }

    /// Loads the state from the provided reader.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from or seeking within the reader fails.
    #[inline]
    pub fn load_data(&mut self, buf: &[u8], secs_since_unix_epoch: u64) -> Result<(), Error> {
        bess::Reader::new(buf).load_state(self, secs_since_unix_epoch)
    }

    #[must_use]
    fn new(model: Model, sample_rate: i32, cart: Cartridge, audio_callback: A) -> Self {
        Self {
            model,
            cgb_mode: model.into(),
            cart,
            bootrom: Bootrom::new(model),
            apu: Apu::new(sample_rate, audio_callback),
            clock: Clock::default(),
            cpu: Sm83::default(),
            dma: Dma::default(),
            dots_ran: Default::default(),
            hdma: Hdma::default(),
            hram: Hram::default(),
            ints: Interrupts::default(),
            joy: Joypad::default(),
            key1: Key1::default(),
            ld_b_b_breakpoint: false,
            ppu: Ppu::default(),
            serial: Serial::default(),
            trace_enabled: false,
            trace_start_pc: None,
            trace_end_pc: None,
            wram: Wram::default(),
            #[cfg(feature = "game_genie")]
            game_genie: GameGenie::default(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn pixel_data_rgba(&self) -> &[u8] {
        self.ppu.pixel_data_rgba()
    }

    #[inline]
    pub const fn press(&mut self, button: Button) {
        self.joy.press(button, &mut self.ints);
    }

    #[inline]
    pub const fn release(&mut self, button: Button) {
        self.joy.release(button);
    }

    #[inline]
    pub fn run_frame(&mut self) {
        while self.dots_ran < DOTS_PER_FRAME {
            self.run_cpu();
        }

        self.dots_ran -= DOTS_PER_FRAME;
    }

    #[inline]
    pub fn save_data(&self, buf: &mut Vec<u8>, secs_since_unix_epoch: u64) {
        bess::Writer::new(buf).save_state(self, secs_since_unix_epoch);
    }

    /// Get the serial output buffer (used by test ROMs like Blargg's tests)
    #[must_use]
    #[inline]
    pub fn serial_output(&self) -> &str {
        self.serial.output()
    }

    #[inline]
    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.ppu.set_color_correction_mode(mode);
    }

    #[inline]
    pub const fn set_sample_rate(&mut self, sample_rate: i32) {
        self.apu.set_sample_rate(sample_rate);
    }

    /// Enable or disable execution tracing.
    ///
    /// When enabled, the emulator will print disassembled instructions and register
    /// state to stderr during execution.
    #[inline]
    pub const fn set_trace_enabled(&mut self, enabled: bool) {
        self.trace_enabled = enabled;
    }

    #[inline]
    pub const fn set_trace_pc_range(&mut self, start: u16, end: u16) {
        self.trace_start_pc = Some(start);
        self.trace_end_pc = Some(end);
    }

    #[inline]
    pub fn soft_reset(&mut self) {
        self.apu.reset();
        self.clock = Clock::default();
        self.cpu = Sm83::default();
        self.dma = Dma::default();
        self.hdma = Hdma::default();
        self.ints = Interrupts::default();
        self.key1 = Key1::default();
        self.ld_b_b_breakpoint = false;
        self.ppu = Ppu::default();
        self.serial = Serial::default();
        self.bootrom.enable();
    }

    /// Disable trace collection.
    ///
    /// For structured logging, use the Rust `tracing` crate with a configured subscriber.
    #[inline]
    pub const fn trace_disable(&mut self) {
        self.trace_enabled = false;
    }

    /// Enable trace collection.
    ///
    /// This method controls whether detailed execution traces are logged.
    /// For structured logging, use the Rust `tracing` crate with a configured subscriber.
    #[inline]
    pub const fn trace_enable(&mut self) {
        self.trace_enabled = true;
    }

    /// Check if execution tracing is enabled.
    #[must_use]
    #[inline]
    pub const fn trace_enabled(&self) -> bool {
        self.trace_enabled
    }

    /// Check if trace collection is enabled.
    #[must_use]
    #[inline]
    pub const fn trace_is_enabled(&self) -> bool {
        self.trace_enabled
    }
}

// FIXME: use all existing models
#[non_exhaustive]
#[derive(Clone, Copy, Default)]
pub enum Model {
    #[default]
    Cgb,
    Dmg,
    Mgb,
}

#[derive(Clone, Copy, Default)]
enum CgbMode {
    #[default]
    Cgb,
    Compat,
    Dmg,
}

impl From<Model> for CgbMode {
    fn from(model: Model) -> Self {
        match model {
            Model::Dmg | Model::Mgb => Self::Dmg,
            Model::Cgb => Self::Cgb,
        }
    }
}

pub struct GbBuilder<A: AudioCallback> {
    audio_callback: A,
    cart: Option<Cartridge>,
    model: Model,
    sample_rate: i32,
}

impl<A: AudioCallback> GbBuilder<A> {
    #[inline]
    pub fn build(self) -> Gb<A> {
        Gb::new(
            self.model,
            self.sample_rate,
            self.cart.unwrap_or_default(),
            self.audio_callback,
        )
    }

    #[inline]
    pub fn can_load_save_data(&self) -> bool {
        self.cart
            .as_ref()
            .is_some_and(cartridge::Cartridge::has_battery)
    }

    #[inline]
    pub fn new(sample_rate: i32, audio_callback: A) -> Self {
        Self {
            model: Model::default(),
            cart: None,
            sample_rate,
            audio_callback,
        }
    }

    #[must_use]
    #[inline]
    pub const fn with_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }

    /// Loads a ROM into the builder.
    ///
    /// # Errors
    ///
    /// Returns an error if the ROM data is invalid or cannot be parsed as a cartridge.
    #[inline]
    pub fn with_rom(mut self, rom: Box<[u8]>) -> Result<Self, Error> {
        self.cart = Some(Cartridge::new(rom)?);
        Ok(self)
    }
}
