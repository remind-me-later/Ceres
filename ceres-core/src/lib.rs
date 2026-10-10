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
    timing::UNITS_PER_FRAME,
};
use alloc::{boxed::Box, vec::Vec};
use cartridge::{Cartridge, HEADER_CGB_B, HEADER_CGB_FLAG};
#[cfg(feature = "game_genie")]
use cheats::GameGenie;
#[cfg(feature = "game_genie")]
pub use cheats::GameGenieCode;
use interrupts::Interrupts;
use joypad::Joypad;
use memory::{
    BCPS, BGP, IE, IF, Key1, LCDC, LYC, OBP0, OBP1, OCPS, P1, SCX, SCY, SpeedSwitch, WX, WY,
    io_addr,
};
use serial::Serial;
use {
    apu::{Apu, PostBoot},
    ppu::Ppu,
};
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
    /// Address the CPU last put on the bus (the APU's wave channel reads it).
    address_bus: u16,
    bootrom: Bootrom,
    cart: Cartridge,
    cgb_mode: CgbMode,
    clock: Clock,
    cpu: Sm83,
    dma: Dma,
    /// 8 MHz units run in the current frame.
    units_ran: i32,
    #[cfg(feature = "game_genie")]
    game_genie: GameGenie,
    hdma: Hdma,
    hram: Hram,
    ints: Interrupts,
    joy: Joypad,
    key1: Key1,
    speed_switch: SpeedSwitch,
    model: Model,
    ppu: Ppu,
    serial: Serial,
    wram: Wram,
    /// Bus time deferred by the CPU but not yet consumed by the machine
    /// (see the `Bus` trait docs in `sm83`). Only nonzero mid-step and for
    /// the interrupt dispatch's 2-T-cycle tail; never serialized.
    time_deferred: i32,
    /// HALT prefetched the next opcode for a pending HBlank transfer.
    hdma_halt_prefetch: bool,
    /// Undocumented CGB register at $FF72 (full R/W, init $00).
    /// Pan Docs "FF72-FF73 — Bits 0-7 (CGB Mode only)".
    undoc_ff72: u8,
    /// Undocumented CGB register at $FF73 (full R/W, init $00).
    undoc_ff73: u8,
    /// Undocumented CGB register at $FF75 (bits 0-3, 7 read as 1; bits 4-6 R/W).
    /// Pan Docs "FF75 — Bits 4-6 (CGB Mode only)".
    undoc_ff75: u8,
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
    #[cfg(feature = "game_genie")]
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
        self.apu.set_model(model);
        self.joy = Joypad::new(matches!(model, Model::Sgb | Model::Sgb2));
        self.soft_reset();
    }

    /// Initializes the system state to match exactly the state immediately
    /// after the bootrom finishes execution, skipping the boot sequence entirely.
    /// This is required to perfectly align timers with some integration tests (e.g., Gambatte).
    ///
    /// Per-model register values come from the mooneye-test-suite source
    /// (`acceptance/boot_regs-*.s` and `misc/boot_regs-*.s`, committed in
    /// `external/test-sources/`), which were measured on real hardware by Joonas
    /// Javanainen. The values are also cross-checked against SameBoy's own
    /// post-boot state in `gb.c::GB_reset_internal`.
    #[expect(
        clippy::missing_inline_in_public_items,
        clippy::too_many_lines,
        reason = "Called once; it lists the state the boot ROM leaves behind"
    )]
    pub fn skip_bootrom(&mut self) {
        if matches!(self.model, Model::Sgb | Model::Sgb2) {
            // The SGB boot ROM transmits the cartridge header to the SGB, so
            // how long it takes (and with it DIV at the hand-off) depends on
            // the header: run it (it takes a fraction of a second).
            self.bootrom.enable();
            while self.bootrom.is_enabled() {
                self.run_cpu();
            }
            return;
        }

        self.bootrom.disable();

        // CPU perfectly aligned post-bootrom. The test ROM starts at $0100
        // with a `jp $0150` (16 T-cycles), then runs the test code. We keep
        // PC at $0100 so the existing per-model clock.div values, calibrated
        // to this entry point, work without modification.
        self.cpu.set_pc(0x0100);
        self.cpu.set_sp(0xFFFE);

        let cgb_cart =
            self.model.is_cgb_hardware() && self.cart.read_rom(HEADER_CGB_FLAG) & HEADER_CGB_B != 0;
        if self.model.is_cgb_hardware() {
            self.cgb_mode = if cgb_cart {
                CgbMode::Cgb
            } else {
                CgbMode::Compat
            };
        }

        // Per-model post-boot register values. Each line documents the source:
        //   DMG-0     → acceptance/boot_regs-dmg0.s   (pass: DMG 0)
        //   DMG-ABC   → acceptance/boot_regs-dmgABC.s (pass: DMG ABC)
        //   MGB       → acceptance/boot_regs-mgb.s    (pass: MGB)
        //   SGB/SGB2  → acceptance/boot_regs-sgb{,2}.s (pass: SGB{,2})
        //   CGB       → misc/boot_regs-cgb.s          (pass: CGB, every revision)
        //   AGB       → misc/boot_regs-A.s            (pass: AGB, AGS)
        //   SGB/SGB2  → acceptance/boot_regs-sgb{,2}.s (A differs: SGB=$01, SGB2=$FF)
        let (af, bc, de, hl) = match self.model {
            Model::Dmg0 => (0x0100, 0xFF13, 0x00C1, 0x8403), // DMG-0
            Model::DmgB => (0x01B0, 0x0013, 0x00D8, 0x014D), // DMG-ABC
            Model::Mgb => (0xFFB0, 0x0013, 0x00D8, 0x014D),  // MGB
            Model::Sgb => (0x0100, 0x0014, 0x0000, 0xC060),  // SGB (A=$01)
            Model::Sgb2 => (0xFF00, 0x0014, 0x0000, 0xC060), // SGB2 (A=$FF)
            // The CGB boot ROMs (measured): DMG-only cartridges get the
            // compatibility values, CGB cartridges DE = $FF56 and HL = $000D.
            Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE => {
                if cgb_cart {
                    (0x1180, 0x0000, 0xFF56, 0x000D)
                } else {
                    (0x1180, 0x0000, 0x0008, 0x007C)
                }
            }
            Model::Agb => {
                if cgb_cart {
                    (0x1100, 0x0100, 0xFF56, 0x000D)
                } else {
                    (0x1100, 0x0100, 0x0008, 0x007C)
                }
            }
        };
        self.cpu.set_af(af);
        self.cpu.set_bc(bc);
        self.cpu.set_de(de);
        self.cpu.set_hl(hl);

        // The boot ROM leaves the sound registers in a state that depends on
        // the model and the boot time (see `PostBoot`).
        self.apu.post_boot(PostBoot::new(self.model, cgb_cart));
        // P1, OBP0/OBP1, LCDC, STAT, LY, LYC, BGP, IF, IE per-model.
        // P1: $CF on DMG/DMG0/MGB, $FF on CGB/SGB.
        self.write_mem(
            io_addr(P1),
            if matches!(self.model, Model::Dmg0 | Model::DmgB | Model::Mgb) {
                0xCF
            } else {
                0xFF
            },
        );
        self.write_mem(io_addr(SCY), 0x00);
        self.write_mem(io_addr(SCX), 0x00);
        // OBP0/OBP1: $00 on CGB, $FF on DMG.
        self.write_mem(
            io_addr(OBP0),
            if self.model.is_cgb_hardware() {
                0x00
            } else {
                0xFF
            },
        );
        self.write_mem(
            io_addr(OBP1),
            if self.model.is_cgb_hardware() {
                0x00
            } else {
                0xFF
            },
        );
        self.write_mem(io_addr(WY), 0x00);
        self.write_mem(io_addr(WX), 0x00);
        // LCDC: $91 on all models.
        self.write_mem(io_addr(LCDC), 0x91);
        self.write_mem(io_addr(LYC), 0x00);
        // DMA: $00 on CGB, $FF on DMG.
        self.dma.set_reg(if self.model.is_cgb_hardware() {
            0x00
        } else {
            0xFF
        });
        // BGP: $FC on all models.
        self.write_mem(io_addr(BGP), 0xFC);
        // Where the boot ROM leaves the PPU: measured by running the real boot
        // ROMs (the hand-off write plus the M-cycle that follows it). DMG and
        // SGB hand off in the tail of line 153 (LY already reads 0); the
        // others in VBlank.
        // CGB hardware runs a longer boot sequence for DMG-only cartridges
        // (compatibility palettes), so they hand off in VBlank line 148; CGB
        // cartridges in line 144.
        let (line, dot) = match self.model {
            Model::Dmg0 => (145, 101),
            Model::DmgB | Model::Mgb => (153, 405),
            // The SGB boot ROM transmits the cartridge header, so its length
            // varies a little with the cartridge (values for the mooneye ROMs).
            Model::Sgb => (153, 173),
            Model::Sgb2 => (153, 161),
            Model::Agb if cgb_cart => (144, 177),
            Model::Agb => (148, 365),
            _ if cgb_cart => (144, 173),
            _ => (148, 361),
        };
        self.ppu.set_position(line, dot);
        if self.model.is_cgb_hardware() {
            // Auto-increment on, at the index the boot ROM stopped at.
            self.write_mem(io_addr(BCPS), 0xC8);
            self.write_mem(io_addr(OCPS), 0xD0);
            self.undoc_ff72 = 0x00;
            self.undoc_ff73 = 0x00;
            self.undoc_ff75 = 0x00;
        }
        // IF: $E1 (VBlank pending) on all models.
        self.write_mem(io_addr(IF), 0xE1);
        // IE: $00 on all models.
        self.write_mem(io_addr(IE), 0x00);

        // DIV phase after boot ROM.  DMG and CGB boot ROMs leave DIV at
        // different phases due to different boot durations.
        // Derived from Gambatte's setPostBiosState:
        //   divLastUpdate = -0x1C00 for both models
        //   cycleCounter = 0x102A0 (CGB) or 0x18FCC (DMG)
        //   internal_counter = cycleCounter - divLastUpdate
        //   DIV = internal_counter & 0xFFFF
        if self.model.is_cgb_hardware() {
            // DIV at the first cartridge instruction, measured by running the
            // real boot ROMs. The boot time depends a little on the header
            // (the compatibility palette lookup hashes the title): these
            // match the mooneye boot_div ROMs for DMG-only cartridges and
            // Gambatte's start_inc for CGB cartridges.
            self.clock.div = match (self.model, cgb_cart) {
                (Model::Cgb0, false) => 0x2884,
                (Model::Cgb0, true) => 0x20AC,
                (Model::Agb, false) => 0x267C,
                (Model::Agb, true) => 0x1EA4,
                (_, false) => 0x2678,
                (_, true) => 0x1EA0,
            };
        } else {
            // DIV at the first cartridge instruction, measured by running the
            // real boot ROMs (mooneye boot_div-*). The SGB value depends on
            // the cartridge header; these match the mooneye ROMs.
            self.clock.div = match self.model {
                Model::Dmg0 => 0x1830,
                Model::Sgb => 0xD860,
                Model::Sgb2 => 0xD850,
                _ => 0xABCC,
            };
        }

        self.clock.div_cycles = 0;
        // The DIV state machine is running.
        self.clock.div_state = 2;

        self.serial.set_master_clock(self.clock.div & 0x100 != 0);
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
    pub fn check_and_reset_ld_b_b_breakpoint(&mut self) -> bool {
        self.cpu.take_ld_b_b_breakpoint()
    }

    /// Read the current value of CPU register A.
    #[must_use]
    #[inline]
    pub const fn cpu_a(&self) -> u8 {
        self.cpu.a()
    }

    /// Read the current value of the CPU flags register F.
    #[must_use]
    #[inline]
    pub const fn cpu_f(&self) -> u8 {
        (self.cpu.af() & 0xFF) as u8
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

        let mut apu = Apu::new(sample_rate, audio_callback);
        apu.set_model(model);

        Self {
            cgb_mode,
            cart,
            bootrom: Bootrom::new(model),
            apu,
            address_bus: 0,
            clock,
            cpu: Sm83::default(),
            dma: Dma::new(model),
            units_ran: Default::default(),
            hdma: Hdma::default(),
            hram: Hram::default(),
            ints: Interrupts::default(),
            joy: Joypad::new(matches!(model, Model::Sgb | Model::Sgb2)),
            key1: Key1::default(),
            speed_switch: SpeedSwitch::default(),
            model,
            ppu: Ppu::new(model),
            serial: Serial::default(),
            wram: Wram::power_on(model.is_cgb_hardware()),
            time_deferred: 0,
            hdma_halt_prefetch: false,
            undoc_ff72: 0,
            undoc_ff73: 0,
            undoc_ff75: 0,
            #[cfg(feature = "game_genie")]
            game_genie: GameGenie::default(),
        }
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
        while self.units_ran < UNITS_PER_FRAME {
            self.run_cpu();
        }

        self.units_ran -= UNITS_PER_FRAME;
    }

    #[inline]
    #[must_use]
    pub const fn cpu_pc(&self) -> u16 {
        self.cpu.pc()
    }

    /// Returns whether the CPU is currently halted.
    ///
    /// This is used by test ROMs (such as Wilbertpol's Mooneye Test Suite
    /// fork) that signal test completion by executing the undefined opcode
    /// `0xED`, which the SM83 implements by entering the HALT state.
    #[inline]
    #[must_use]
    pub const fn cpu_is_halted(&self) -> bool {
        self.cpu.is_halted()
    }

    #[inline]
    pub const fn check_and_reset_illegal_opcode_breakpoint(&mut self) -> bool {
        self.cpu.take_illegal_opcode()
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
        self.dma = Dma::new(self.model);
        self.hdma = Hdma::default();
        self.ints = Interrupts::default();
        self.key1 = Key1::default();
        self.speed_switch = SpeedSwitch::default();
        self.time_deferred = 0;
        self.ppu = Ppu::new(self.model);
        self.serial = Serial::default();
        self.bootrom.enable();
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Model {
    #[default]
    CgbE,
    Cgb0,
    CgbA,
    CgbB,
    CgbC,
    CgbD,
    Dmg0,
    DmgB,
    Mgb,
    Sgb,
    Sgb2,
    Agb,
}

impl Model {
    #[must_use]
    #[inline]
    pub const fn is_cgb_hardware(self) -> bool {
        matches!(
            self,
            Self::Cgb0 | Self::CgbA | Self::CgbB | Self::CgbC | Self::CgbD | Self::CgbE | Self::Agb
        )
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CgbMode {
    #[default]
    Cgb,
    Compat,
    Dmg,
}

impl From<Model> for CgbMode {
    #[inline]
    fn from(model: Model) -> Self {
        match model {
            Model::Dmg0 | Model::DmgB | Model::Mgb | Model::Sgb | Model::Sgb2 => Self::Dmg,
            Model::Cgb0
            | Model::CgbA
            | Model::CgbB
            | Model::CgbC
            | Model::CgbD
            | Model::CgbE
            | Model::Agb => Self::Cgb,
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
