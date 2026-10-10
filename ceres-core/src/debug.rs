//! What test harnesses and debuggers look at (the `debug` feature): the CPU
//! registers, the breakpoint opcodes test ROMs use and the serial output.

use crate::{AudioCallback, Gb};

impl<A: AudioCallback> Gb<A> {
    /// Whether `ld b, b` (0x40) ran since the last call. Test ROMs such as
    /// the acid tests and Mooneye's use it as a breakpoint.
    #[inline]
    pub fn take_ld_b_b_breakpoint(&mut self) -> bool {
        self.cpu.take_ld_b_b_breakpoint()
    }

    /// Whether an illegal opcode ran since the last call. Wilbertpol's
    /// Mooneye fork ends its tests with 0xED.
    #[inline]
    pub const fn take_illegal_opcode(&mut self) -> bool {
        self.cpu.take_illegal_opcode()
    }

    #[must_use]
    #[inline]
    pub const fn cpu_a(&self) -> u8 {
        self.cpu.a()
    }

    #[must_use]
    #[inline]
    pub const fn cpu_f(&self) -> u8 {
        self.cpu.af().to_le_bytes()[0]
    }

    #[must_use]
    #[inline]
    pub const fn cpu_b(&self) -> u8 {
        self.cpu.bc().to_le_bytes()[1]
    }

    #[must_use]
    #[inline]
    pub const fn cpu_c(&self) -> u8 {
        self.cpu.bc().to_le_bytes()[0]
    }

    #[must_use]
    #[inline]
    pub const fn cpu_d(&self) -> u8 {
        self.cpu.de().to_le_bytes()[1]
    }

    #[must_use]
    #[inline]
    pub const fn cpu_e(&self) -> u8 {
        self.cpu.de().to_le_bytes()[0]
    }

    #[must_use]
    #[inline]
    pub const fn cpu_h(&self) -> u8 {
        self.cpu.hl().to_le_bytes()[1]
    }

    #[must_use]
    #[inline]
    pub const fn cpu_l(&self) -> u8 {
        self.cpu.hl().to_le_bytes()[0]
    }

    #[must_use]
    #[inline]
    pub const fn cpu_sp(&self) -> u16 {
        self.cpu.sp()
    }

    #[must_use]
    #[inline]
    pub const fn cpu_pc(&self) -> u16 {
        self.cpu.pc()
    }

    #[must_use]
    #[inline]
    pub const fn cpu_is_halted(&self) -> bool {
        self.cpu.is_halted()
    }

    /// A VRAM byte, whatever the PPU mode (the CPU cannot read VRAM in
    /// mode 3).
    #[must_use]
    #[inline]
    pub const fn read_vram_direct(&self, addr: u16) -> u8 {
        self.ppu.vram().read(addr)
    }

    /// The printable bytes sent through the serial port (Blargg's tests print
    /// their results there).
    #[must_use]
    #[inline]
    pub fn serial_output(&self) -> &str {
        self.serial.output()
    }
}
