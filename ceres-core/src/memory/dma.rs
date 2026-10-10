//! OAM DMA.
//!
//! Port of SameBoy's `GB_dma_run` and the bus conflicts of `GB_read_memory` /
//! `GB_write_memory`: while a transfer runs, CPU accesses that share a bus with
//! the DMA source read (or write) whatever the DMA is currently accessing.

use {
    super::{CART_RAM_START, ECHO_B, ECHO_START, OAM_START, VRAM_START, WRAM_START, Wram},
    crate::{
        AudioCallback, Gb, Model,
        ppu::{Oam, unusable_index},
    },
};

pub(crate) struct Dma {
    /// Cycles to process on the next `Gb::run_dma`.
    cycles: i32,
    /// Cycles left over from the previous run (a byte takes 4).
    cycles_modulo: i32,
    /// OAM index being written; `DEST_START_UP` during the start-up delay,
    /// `Oam::SIZE` in the last cycle, `DEST_IDLE` when idle.
    current_dest: u8,
    current_src: u16,
    reg: u8,
    /// A new transfer was started while another one was running.
    restarting: bool,
    /// The OAM index the interrupted transfer was about to write when it
    /// was restarted: it still writes it, from the new source, during the
    /// new transfer's start-up delay (gambatte's `oamDmaStartPos_`).
    restart_at: Option<u8>,
}

impl Dma {
    /// `current_dest` while no transfer is running.
    pub(crate) const DEST_IDLE: u8 = 0xA1;
    /// `current_dest` during the start-up delay of a transfer.
    pub(crate) const DEST_START_UP: u8 = 0xFF;
}

impl Default for Dma {
    fn default() -> Self {
        Self {
            cycles: 0,
            cycles_modulo: 0,
            current_dest: Self::DEST_IDLE,
            current_src: 0,
            reg: 0xFF,
            restarting: false,
            restart_at: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DmaBus {
    Main,
    Ram,
    Vram,
}

impl Dma {
    /// The DMA register powers on as 00 on CGB and FF on DMG hardware.
    #[must_use]
    pub(crate) fn new(model: Model) -> Self {
        Self {
            reg: if model.is_cgb_hardware() { 0x00 } else { 0xFF },
            ..Self::default()
        }
    }

    #[must_use]
    pub(crate) const fn is_active(&self) -> bool {
        self.current_dest != Self::DEST_IDLE
    }

    /// The next step of the transfer is its last one, which hands OAM back.
    #[must_use]
    pub(crate) const fn is_in_last_step(&self) -> bool {
        self.current_dest == Oam::SIZE
    }

    /// CPU reads of OAM return 0xFF while the DMA owns it.
    #[must_use]
    pub(crate) const fn blocks_oam_read(&self) -> bool {
        self.is_active() && (self.current_dest != 0 || self.restarting)
    }

    /// CPU writes to OAM are dropped while a transfer is active.
    #[must_use]
    pub(crate) const fn blocks_oam_write(&self) -> bool {
        self.is_active()
    }

    #[must_use]
    pub(crate) const fn read(&self) -> u8 {
        self.reg
    }

    pub(crate) const fn set_reg(&mut self, val: u8) {
        self.reg = val;
    }

    /// The HDMA copies bytes into OAM while an OAM DMA is running, but only
    /// on the M-cycle phase where the DMA has the bus free.
    #[must_use]
    pub(crate) const fn hdma_can_write_oam(&self, double_speed: bool) -> bool {
        self.is_active() && (self.cycles_modulo == 2 || double_speed)
    }

    /// The T-cycles the next `run_dma` runs (SameBoy's `dma_cycles`).
    pub(crate) const fn set_cycles(&mut self, cycles: i32) {
        self.cycles = cycles;
    }

    pub(crate) fn write(&mut self, val: u8) {
        self.restarting = self.current_dest != Self::DEST_IDLE && self.current_dest != Oam::SIZE;
        self.restart_at = (1..Oam::SIZE)
            .contains(&self.current_dest)
            .then_some(self.current_dest);
        self.cycles = 0;
        self.cycles_modulo = 2;
        self.current_dest = Self::DEST_START_UP;
        self.current_src = u16::from(val) << 8;
        self.reg = val;
    }
}

const fn bus_for_addr(cgb: bool, addr: u16) -> DmaBus {
    if addr < VRAM_START {
        DmaBus::Main
    } else if addr < CART_RAM_START {
        DmaBus::Vram
    } else if addr < WRAM_START || !cgb {
        DmaBus::Main
    } else {
        DmaBus::Ram
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Whether a CPU access to `addr` conflicts with the running transfer.
    fn is_addr_in_dma_use(&self, addr: u16) -> bool {
        let dma = &self.dma;
        if !dma.is_active() || addr >= OAM_START || self.hdma.is_transferring() {
            return false;
        }
        // Warm-up, unless a restarted transfer is still writing.
        if dma.current_dest == Dma::DEST_START_UP
            || (dma.current_dest == 0 && dma.restart_at.is_none())
        {
            return false;
        }
        // Shortcut for the DMA's own access flow.
        if dma.current_src == addr {
            return false;
        }
        if dma.current_src >= ECHO_START && (dma.current_src & !ECHO_B) == addr {
            return false;
        }
        let cgb = self.model.is_cgb_hardware();
        if cgb {
            if addr >= WRAM_START {
                return bus_for_addr(cgb, dma.current_src) != DmaBus::Vram;
            }
            if dma.current_src >= ECHO_START {
                return bus_for_addr(cgb, addr) != DmaBus::Vram;
            }
        }
        bus_for_addr(cgb, addr) == bus_for_addr(cgb, dma.current_src)
    }

    /// The address a CPU read of `addr` really reaches during a transfer, or
    /// `None` if the bus is driven by the DMA (open bus: 0xFF).
    pub(crate) fn dma_read_redirect(&self, addr: u16) -> Option<u16> {
        if !self.is_addr_in_dma_use(addr) {
            return Some(addr);
        }
        let cgb = self.model.is_cgb_hardware();
        let src = self.dma.current_src;
        if let Some(at) = self.dma.restart_at
            && self.dma.current_dest == 0
        {
            // The byte the interrupted transfer just wrote.
            return Some(src.wrapping_add(u16::from(at)));
        }
        if cgb && bus_for_addr(cgb, addr) == DmaBus::Main && src >= ECHO_START {
            // Cart specific.
            return None;
        }
        if cgb && addr >= WRAM_START && (bus_for_addr(cgb, src) != DmaBus::Ram || src >= ECHO_START) {
            return Some(
                (src.wrapping_sub(1) & Wram::BANK_SIZE)
                    | (addr & (Wram::BANK_SIZE - 1))
                    | WRAM_START,
            );
        }
        let current = src.wrapping_sub(1);
        // The DMG's DMA reads the work RAM through its echo.
        Some(if !cgb && current >= ECHO_START {
            current & !ECHO_B
        } else {
            current
        })
    }

    /// What a CPU read that conflicted with a VRAM-sourced transfer leaves
    /// behind on the CPU-CGB-C: the OAM byte being copied is cleared.
    pub(crate) fn dma_after_read(&mut self, addr: u16) {
        let src = self.dma.current_src.wrapping_sub(1);
        if self.model == Model::CgbC
            && (VRAM_START..CART_RAM_START).contains(&src)
            && self.is_addr_in_dma_use(addr)
        {
            let index = usize::from(self.dma.current_dest.wrapping_sub(1));
            if let Some(byte) = self.ppu.oam_mut().bytes_mut().get_mut(index) {
                *byte = 0;
            }
        }
    }

    /// Handles a CPU write to `addr` during a transfer. Returns the address
    /// the write still has to be performed on, if any.
    pub(crate) fn dma_write_redirect(&mut self, addr: u16, value: u8) -> Option<u16> {
        if !self.is_addr_in_dma_use(addr) {
            return Some(addr);
        }
        let cgb = self.model.is_cgb_hardware();
        let model = self.model;
        let src = self.dma.current_src;
        if !cgb {
            // The write only reaches the OAM byte the DMA is writing (ANDed
            // with it when the DMA reads the work RAM).
            let current = src.wrapping_sub(1);
            let oam = self.ppu.oam_mut().bytes_mut();
            if let Some(byte) = oam.get_mut(usize::from(self.dma.current_dest.wrapping_sub(1))) {
                *byte = if current >= WRAM_START {
                    *byte & value
                } else {
                    value
                };
            }
            return None;
        }
        if model == Model::CgbC && addr < WRAM_START {
            // CPU-CGB-C (measured by Gambatte's tests): the write lands in the
            // OAM byte being copied, or clears it when the DMA reads the VRAM.
            let current = src.wrapping_sub(1);
            let oam = self.ppu.oam_mut().bytes_mut();
            if let Some(byte) = oam.get_mut(usize::from(self.dma.current_dest.wrapping_sub(1))) {
                *byte = if (VRAM_START..CART_RAM_START).contains(&current) {
                    0
                } else {
                    value
                };
            }
            return None;
        }
        if bus_for_addr(cgb, addr) == DmaBus::Main && src >= ECHO_START {
            // Cart specific.
            return None;
        }
        if !(WRAM_START..ECHO_START).contains(&src) && addr >= WRAM_START {
            return Some(
                (src.wrapping_sub(1) & Wram::BANK_SIZE)
                    | (addr & (Wram::BANK_SIZE - 1))
                    | WRAM_START,
            );
        }

        let current = src.wrapping_sub(1);
        let oam_index = usize::from(self.dma.current_dest.wrapping_sub(1));
        let before_cgb_c = matches!(model, Model::Cgb0 | Model::CgbA | Model::CgbB);
        let before_cgb_e = before_cgb_c || matches!(model, Model::CgbC | Model::CgbD);
        let oam = self.ppu.oam_mut().bytes_mut();
        if oam_index < oam.len() {
            if current < CART_RAM_START {
                oam[oam_index] = 0;
            } else if model == Model::CgbB {
                oam[oam_index] &= value;
            } else if before_cgb_c || model == Model::Agb {
                oam[oam_index] = value;
            } else {
                // CGB-C, D and E: the OAM is left alone.
            }
        }
        if before_cgb_e || current >= CART_RAM_START {
            return None;
        }
        Some(current)
    }

    /// SameBoy's `write_oam`: a byte the HDMA drops into OAM (only the low
    /// byte of the source address is used). Past the 160 bytes it goes to the
    /// memory behind the unusable area on some revisions.
    pub(crate) fn hdma_write_oam(&mut self, addr: u8, value: u8) {
        let oam = self.ppu.oam_mut();
        if addr < Oam::SIZE {
            oam.bytes_mut()[usize::from(addr)] = value;
            return;
        }
        if let Some(i) = unusable_index(self.model, addr) {
            oam.extra_mut()[i] = value;
        }
    }

    /// DMA source read: unlike the CPU it is not blocked by the PPU.
    fn dma_read(&self, addr: u16) -> u8 {
        if (VRAM_START..CART_RAM_START).contains(&addr) {
            self.ppu.vram().read(addr)
        } else {
            self.read_mem(addr)
        }
    }

    /// The OAM index `run_dma` will have reached after the next `cycles`.
    pub(crate) const fn dma_dest_after(&self, cycles: i32) -> u8 {
        let mut dest = self.dma.current_dest;
        if !self.dma.is_active() || self.hdma.cpu_halted() || self.clock.stopped {
            return dest;
        }
        let mut due = (self.dma.cycles_modulo + cycles) / 4;
        while due > 0 {
            due -= 1;
            let last = dest >= Oam::SIZE;
            dest = dest.wrapping_add(1);
            if last {
                break;
            }
        }
        dest
    }

    /// Port of `GB_dma_run`: transfers the bytes that became due.
    pub(crate) fn run_dma(&mut self) {
        if !self.dma.is_active() || self.hdma.cpu_halted() || self.clock.stopped {
            return;
        }
        // The halt of a speed switch holds the OAM DMA until the CPU resumes.
        if self.speed_switch.unhalt {
            return;
        }
        let cgb = self.model.is_cgb_hardware();
        let mut cycles = self.dma.cycles + self.dma.cycles_modulo;
        while cycles >= 4 {
            cycles -= 4;
            if self.dma.current_dest >= Oam::SIZE {
                if self.dma.current_dest == Dma::DEST_START_UP
                    && let Some(at) = self.dma.restart_at
                {
                    let value = self.dma_read(self.dma.current_src.wrapping_add(u16::from(at)));
                    self.ppu.write_oam_by_dma(OAM_START | u16::from(at), value);
                }
                self.dma.current_dest = self.dma.current_dest.wrapping_add(1);
                self.ppu.dma_finished(&mut self.ints);
                break;
            }
            let dest = self.dma.current_dest;
            self.dma.restart_at = None;
            self.dma.current_dest = self.dma.current_dest.wrapping_add(1);
            let src = self.dma.current_src;
            if self.hdma.is_transferring()
                && (self.hdma.has_multiple_steps_left() || !self.hdma.is_at_block_end())
            {
                // The HDMA owns the bus: nothing is copied.
            } else if src < ECHO_START {
                let value = self.dma_read(src);
                self.ppu
                    .write_oam_by_dma(OAM_START | u16::from(dest), value);
            } else if cgb {
                self.ppu.write_oam_by_dma(OAM_START | u16::from(dest), 0xFF);
            } else {
                let value = self.dma_read(src & !ECHO_B);
                self.ppu
                    .write_oam_by_dma(OAM_START | u16::from(dest), value);
            }
            self.dma.current_src = src.wrapping_add(1);
            self.ppu.clear_dma_vram_conflict();
        }
        self.dma.cycles_modulo = cycles;
        self.dma.cycles = 0;
        self.ppu.set_dma_state(
            self.dma.current_dest,
            self.dma.current_src,
            self.dma.cycles_modulo != 0,
        );
    }
}
