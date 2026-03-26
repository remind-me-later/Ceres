extern crate alloc;

mod apu;
mod bess;
mod bootrom;
mod cartridge;
#[cfg(feature = "game_genie")]
mod cheats;
mod error;
mod interrupts;
mod joypad;
mod memory;
mod ppu;
mod serial;
mod sm83;
mod timing;

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
    ppu::{LINES, PX_HEIGHT, PX_WIDTH, WIDTH},
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
    dma_write_start_dots: u64,
    dots_ran: i32,
    #[cfg(feature = "game_genie")]
    game_genie: GameGenie,
    hdma: Hdma,
    hram: Hram,
    pub(crate) ints: Interrupts,
    joy: Joypad,
    key1: Key1,
    ld_b_b_breakpoint: bool,
    model: Model,
    ppu: Ppu,
    serial: Serial,
    total_dots: u64,
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

    /// Initializes the system state to match exactly the state immediately
    /// after the bootrom finishes execution, skipping the boot sequence entirely.
    /// This is required to perfectly align timers with some integration tests (e.g., Gambatte).
    pub(crate) fn skip_bootrom(&mut self) {
        self.bootrom.disable();

        // CPU perfectly aligned post-bootrom
        self.cpu.set_pc(0x0100);
        self.cpu.set_sp(0xFFFE);

        if self.is_cgb() {
            // AGB F value might differ slightly, but we default to standard CGB
            self.cpu.set_af(0x11B0);
            self.cpu.set_bc(0x0013);
            self.cpu.set_de(0x00D8);
            self.cpu.set_hl(0x014D);
        } else {
            self.cpu.set_af(0x01B0);
            self.cpu.set_bc(0x0013);
            self.cpu.set_de(0x00D8);
            self.cpu.set_hl(0x014D);
        }

        // Initialize IO to standard post-boot values
        self.write_mem(0xFF11, 0xBF);
        self.write_mem(0xFF12, 0xF3);
        self.write_mem(0xFF24, 0x77);
        self.write_mem(0xFF25, 0xF3);
        self.write_mem(0xFF26, 0xF1);
        self.write_mem(0xFF40, 0x91);

        // DIV phase after boot ROM.  DMG and CGB boot ROMs leave DIV at
        // different phases due to different boot durations.
        // DMG: 0xABCC (from Gambatte's setPostBiosState with cycleCounter=0x18FCC)
        // CGB: 0x1DF0 (derived from Gambatte's cycleCounter=0x102A0 and
        //      divLastUpdate=-0x1C00, adjusted so that both div_start_inc and
        //      tc00_start gambatte test families pass)
        if self.is_cgb() {
            self.clock.div = 0x1DF0;
        } else {
            self.clock.div = 0xABCC;
        }
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

    /// Read the current value of CPU register A.
    #[must_use]
    #[inline]
    pub const fn cpu_a(&self) -> u8 {
        self.cpu.a()
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
        let cgb_mode = CgbMode::from(model);
        let clock = Clock::default();

        Self {
            model,
            cgb_mode,
            cart,
            bootrom: Bootrom::new(model),
            apu: Apu::new(sample_rate, audio_callback),
            clock,
            cpu: Sm83::default(),
            dma: Dma::default(),
            dma_write_start_dots: 0,
            dots_ran: Default::default(),
            total_dots: 0,
            hdma: Hdma::default(),
            hram: Hram::default(),
            ints: Interrupts::default(),
            joy: Joypad::default(),
            key1: Key1::default(),
            ld_b_b_breakpoint: false,
            ppu: Ppu::default(),
            serial: Serial::default(),
            wram: Wram::default(),
            #[cfg(feature = "game_genie")]
            game_genie: GameGenie::default(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn is_cgb(&self) -> bool {
        matches!(
            self.model,
            Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE
        )
    }

    #[must_use]
    #[inline]
    pub const fn pixel_data_rgba(&self) -> &[u8] {
        self.ppu.pixel_data_rgba()
    }

    /// Read a VRAM byte directly, bypassing PPU mode-accessibility checks.
    ///
    /// This is intended for test ROM completion checkers that need to inspect
    /// VRAM contents regardless of the current PPU rendering mode.  Normal
    /// emulated code must use `read_mem` so that mode-3 blocking is enforced.
    #[must_use]
    #[inline]
    pub const fn read_vram_direct(&self, addr: u16) -> u8 {
        self.ppu.vram().read(addr)
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
    pub fn set_sample_rate(&mut self, sample_rate: i32) {
        self.apu.set_sample_rate(sample_rate);
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
}

// FIXME: use all existing models
#[non_exhaustive]
#[derive(Clone, Copy, Default)]
pub enum Model {
    #[default]
    CgbE,
    Cgb0,
    CgbA,
    CgbB,
    CgbC,
    CgbD,
    DmgB,
    Mgb,
}

#[derive(Clone, Copy, Default)]
pub(crate) enum CgbMode {
    #[default]
    Cgb,
    Compat,
    Dmg,
}

impl From<Model> for CgbMode {
    fn from(model: Model) -> Self {
        match model {
            Model::DmgB | Model::Mgb => Self::Dmg,
            Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE => {
                Self::Cgb
            }
        }
    }
}

pub struct GbBuilder<A: AudioCallback> {
    audio_callback: A,
    cart: Option<Cartridge>,
    model: Model,
    sample_rate: i32,
    run_bootrom: bool,
}

impl<A: AudioCallback> GbBuilder<A> {
    #[inline]
    pub fn build(self) -> Gb<A> {
        let mut gb = Gb::new(
            self.model,
            self.sample_rate,
            self.cart.unwrap_or_default(),
            self.audio_callback,
        );

        if !self.run_bootrom {
            gb.skip_bootrom();
        }

        gb
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
            run_bootrom: true,
        }
    }

    #[must_use]
    #[inline]
    pub const fn with_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }

    #[must_use]
    #[inline]
    pub const fn with_run_bootrom(mut self, run_bootrom: bool) -> Self {
        self.run_bootrom = run_bootrom;
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

#[cfg(test)]
pub(crate) mod test_util {
    use super::*;

    pub struct DummyAudio;
    impl AudioCallback for DummyAudio {
        fn audio_sample(&self, _l: crate::Sample, _r: crate::Sample) {}
    }

    pub fn setup_gb() -> Gb<DummyAudio> {
        GbBuilder::new(44100, DummyAudio)
            .with_model(Model::DmgB)
            .build()
    }
}

#[cfg(test)]
impl<A: AudioCallback> Gb<A> {
    pub(crate) fn set_rom_byte(&mut self, addr: u16, val: u8) {
        self.cart.write_rom_byte_for_test(addr, val);
    }

    /// Set the CPU program counter.  Only available in test builds.
    pub(crate) fn set_cpu_pc(&mut self, pc: u16) {
        self.cpu.set_pc(pc);
    }

    pub(crate) fn set_cpu_bc(&mut self, bc: u16) {
        self.cpu.set_bc(bc);
    }

    pub(crate) fn set_cpu_de(&mut self, de: u16) {
        self.cpu.set_de(de);
    }

    pub(crate) fn set_cpu_hl(&mut self, hl: u16) {
        self.cpu.set_hl(hl);
    }

    pub(crate) fn set_cpu_sp(&mut self, sp: u16) {
        self.cpu.set_sp(sp);
    }

    pub(crate) const fn total_dots(&self) -> u64 {
        self.total_dots
    }

    pub(crate) const fn is_double_speed(&self) -> bool {
        self.key1.is_enabled()
    }
}
