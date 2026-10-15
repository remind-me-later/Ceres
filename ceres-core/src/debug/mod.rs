//! What test harnesses and debuggers look at (the `debug` feature): the CPU
//! registers, the breakpoint opcodes test ROMs use, the serial output, and
//! what a debugger needs to stop and inspect the machine (single steps,
//! memory access without side effects, watchpoints).

mod disasm;
mod io_log;
mod state;

pub use disasm::{Instruction, Operand, disassemble};
pub use io_log::IoAccess;
pub(crate) use io_log::IoLog;
pub use state::{HdmaKind, HdmaState, OamDmaPhase, OamDmaState, PpuState, TimerState};

use {
    crate::{AudioCallback, Gb, Model, PpuEvent, memory::Wram, timing::UNITS_PER_FRAME},
    alloc::{vec, vec::Vec},
};

/// The ROM banks the CPU sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RomBanks {
    /// The bank at 0x0000-0x3FFF (not always 0 on an MBC1).
    pub rom0: u32,
    /// The bank at 0x4000-0x7FFF.
    pub romx: u32,
}

/// A CPU memory access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MemoryAccess {
    Read,
    Write,
}

/// A watched address the CPU accessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct WatchHit {
    pub addr: u16,
    pub access: MemoryAccess,
    /// The byte read or written.
    pub value: u8,
    /// What the byte was before a write (the value, for a read).
    pub old: u8,
}

/// The addresses whose CPU reads or writes stop a debugger, as one bit per
/// address (allocated on the first watchpoint).
#[derive(Default)]
pub(crate) struct Watchpoints {
    read: Vec<u64>,
    write: Vec<u64>,
    /// Whether any address is watched, so unwatched machines skip the
    /// lookup.
    any: bool,
    /// The first watched access since the last `take_hit`.
    hit: Option<WatchHit>,
}

impl Watchpoints {
    const WORDS: usize = 0x1_0000 / 64;

    const fn bits(&self, access: MemoryAccess) -> &Vec<u64> {
        match access {
            MemoryAccess::Read => &self.read,
            MemoryAccess::Write => &self.write,
        }
    }

    /// Whether a CPU access to `addr` is watched (and the first since the
    /// last `take_hit`).
    #[inline]
    pub(crate) fn is_watched(&self, addr: u16, access: MemoryAccess) -> bool {
        self.any
            && self.hit.is_none()
            && self.bits(access)[usize::from(addr) / 64] & (1 << (addr % 64)) != 0
    }

    /// Notes a watched access.
    pub(crate) const fn hit(&mut self, hit: WatchHit) {
        self.hit = Some(hit);
    }

    fn set(&mut self, addr: u16, access: MemoryAccess, watched: bool) {
        if self.read.is_empty() {
            self.read = vec![0; Self::WORDS];
            self.write = vec![0; Self::WORDS];
        }
        let bits = match access {
            MemoryAccess::Read => &mut self.read,
            MemoryAccess::Write => &mut self.write,
        };
        let word = &mut bits[usize::from(addr) / 64];
        if watched {
            *word |= 1 << (addr % 64);
        } else {
            *word &= !(1 << (addr % 64));
        }
        self.any = self.read.iter().chain(&self.write).any(|&w| w != 0);
    }
}

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

    /// The interrupt master enable flag.
    #[must_use]
    #[inline]
    pub const fn cpu_ime(&self) -> bool {
        self.cpu.ime()
    }

    #[must_use]
    #[inline]
    pub const fn cpu_af(&self) -> u16 {
        self.cpu.af()
    }

    #[must_use]
    #[inline]
    pub const fn cpu_bc(&self) -> u16 {
        self.cpu.bc()
    }

    #[must_use]
    #[inline]
    pub const fn cpu_de(&self) -> u16 {
        self.cpu.de()
    }

    #[must_use]
    #[inline]
    pub const fn cpu_hl(&self) -> u16 {
        self.cpu.hl()
    }

    /// Sets AF (the low nibble of F always reads 0).
    #[inline]
    pub const fn set_cpu_af(&mut self, af: u16) {
        self.cpu.set_af(af);
    }

    #[inline]
    pub const fn set_cpu_bc(&mut self, bc: u16) {
        self.cpu.set_bc(bc);
    }

    #[inline]
    pub const fn set_cpu_de(&mut self, de: u16) {
        self.cpu.set_de(de);
    }

    #[inline]
    pub const fn set_cpu_hl(&mut self, hl: u16) {
        self.cpu.set_hl(hl);
    }

    #[inline]
    pub const fn set_cpu_sp(&mut self, sp: u16) {
        self.cpu.set_sp(sp);
    }

    #[inline]
    pub const fn set_cpu_pc(&mut self, pc: u16) {
        self.cpu.set_pc(pc);
    }

    /// Runs one CPU step: an instruction, an interrupt dispatch or, while
    /// halted, one M-cycle. Returns whether that ended a frame, as
    /// `run_frame` counts them, so steps and frames can be mixed.
    #[inline]
    pub fn step(&mut self) -> bool {
        self.run_cpu();
        if self.units_ran >= UNITS_PER_FRAME {
            self.units_ran -= UNITS_PER_FRAME;
            true
        } else {
            false
        }
    }

    /// Runs the rest of the frame, or until `stop` (asked after every step,
    /// the frame's last one included) says to stop. Returns whether the
    /// frame ended.
    #[inline]
    pub fn run_frame_until<F: FnMut(&mut Self) -> bool>(&mut self, mut stop: F) -> bool {
        loop {
            let ended = self.step();
            if stop(self) || ended {
                return ended;
            }
        }
    }

    /// The byte at `addr`, as the CPU would read it but whatever the PPU or
    /// a DMA is doing, and without side effects.
    #[must_use]
    #[inline]
    pub fn peek_mem(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9FFF => self.ppu.vram().read(addr),
            0xFE00..=0xFE9F => self.ppu.oam().read(addr),
            _ => self.read_mem(addr),
        }
    }

    /// Writes `val` at `addr` for a debugger: whatever the PPU or a DMA is
    /// doing, and in 0x0000-0x7FFF into the ROM bank mapped there instead of
    /// to the MBC. Other writes (the I/O registers) have their usual effects.
    #[inline]
    pub fn poke_mem(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.cart.patch_rom(addr, val),
            0x8000..=0x9FFF => self.ppu.vram_mut().write(addr, val),
            0xFE00..=0xFE9F => self.ppu.oam_mut().write(addr, val),
            _ => self.write_mem(addr, val),
        }
    }

    /// Starts or stops watching CPU accesses to `addr` (OAM DMA and HDMA
    /// transfers are not watched).
    #[inline]
    pub fn set_watchpoint(&mut self, addr: u16, access: MemoryAccess, watched: bool) {
        self.watch.set(addr, access, watched);
    }

    #[inline]
    pub fn clear_watchpoints(&mut self) {
        self.watch = Watchpoints::default();
    }

    /// The first watched access since the last call.
    #[inline]
    pub const fn take_watch_hit(&mut self) -> Option<WatchHit> {
        self.watch.hit.take()
    }

    #[must_use]
    #[inline]
    pub const fn model(&self) -> Model {
        self.model
    }

    /// Whether the CPU reads the boot ROM at `addr`, not the cartridge.
    #[must_use]
    #[inline]
    pub fn boot_rom_at(&self, addr: u16) -> bool {
        let boot_area = match addr {
            0x0000..=0x00FF => true,
            0x0200..=0x08FF => self.model.is_cgb_hardware(),
            _ => false,
        };
        boot_area && self.bootrom.read(addr).is_some()
    }

    /// The byte at `addr` (0x0000-0x7FFF) of the cartridge's ROM bank
    /// `bank`, whether it's mapped or not.
    #[must_use]
    #[inline]
    pub fn peek_rom(&self, bank: u32, addr: u16) -> u8 {
        self.cart.read_rom_bank(bank, addr)
    }

    /// The ROM banks mapped at 0x0000-0x3FFF and 0x4000-0x7FFF.
    #[must_use]
    #[inline]
    pub const fn rom_banks(&self) -> RomBanks {
        self.cart.rom_banks()
    }

    /// The WRAM bank mapped at 0xD000-0xDFFF (1 but on a CGB in CGB mode).
    #[must_use]
    #[inline]
    pub const fn wram_bank(&self) -> u8 {
        (self.wram.svbk().bank_offset() / Wram::BANK_SIZE) as u8
    }

    /// The VRAM bank mapped at 0x8000-0x9FFF.
    #[must_use]
    #[inline]
    pub const fn vram_bank(&self) -> u8 {
        self.ppu.vram().bank()
    }

    /// The cartridge RAM bank mapped at 0xA000-0xBFFF.
    #[must_use]
    #[inline]
    pub const fn ram_bank(&self) -> u32 {
        self.cart.ram_bank()
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

    /// Starts recording what the CPU sees of the PPU (its LCD interrupt
    /// requests and STAT and LY reads), or stops and forgets it.
    #[inline]
    pub fn set_ppu_trace(&mut self, enabled: bool) {
        self.ppu.set_trace(enabled, &self.ints);
    }

    /// The PPU events recorded since the last call.
    #[inline]
    pub fn take_ppu_trace(&mut self) -> Vec<PpuEvent> {
        self.ppu.take_trace()
    }
}

#[cfg(test)]
mod tests;
