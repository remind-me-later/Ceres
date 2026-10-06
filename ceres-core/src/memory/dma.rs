//! OAM DMA.
//!
//! Port of SameBoy's `GB_dma_run` and the bus conflicts of `GB_read_memory` /
//! `GB_write_memory`: while a transfer runs, CPU accesses that share a bus with
//! the DMA source read (or write) whatever the DMA is currently accessing.

use crate::{AudioCallback, Gb, Model};

/// `current_dest` value while no transfer is running.
const INACTIVE: u8 = 0xA1;

pub struct Dma {
    /// Cycles to process on the next `Gb::run_dma`.
    cycles: i32,
    /// Cycles left over from the previous run (a byte takes 4).
    cycles_modulo: i32,
    /// OAM index being written; `0xFF` during the start-up delay, `0xA0` in
    /// the last cycle, `INACTIVE` when idle.
    current_dest: u8,
    current_src: u16,
    reg: u8,
    /// A new transfer was started while another one was running.
    restarting: bool,
}

impl Default for Dma {
    fn default() -> Self {
        Self {
            cycles: 0,
            cycles_modulo: 0,
            current_dest: INACTIVE,
            current_src: 0,
            reg: 0xFF,
            restarting: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bus {
    Main,
    Ram,
    Vram,
}

impl Dma {
    /// The DMA register powers on as 00 on CGB and FF on DMG hardware.
    #[must_use]
    pub fn new(model: Model) -> Self {
        Self {
            reg: if model.is_cgb_hardware() { 0x00 } else { 0xFF },
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.current_dest != INACTIVE
    }

    /// CPU reads of OAM return 0xFF while the DMA owns it.
    #[must_use]
    pub const fn blocks_oam_read(&self) -> bool {
        self.is_active() && (self.current_dest != 0 || self.restarting)
    }

    /// CPU writes to OAM are dropped while a transfer is active.
    #[must_use]
    pub const fn blocks_oam_write(&self) -> bool {
        self.is_active()
    }

    #[must_use]
    pub const fn read(&self) -> u8 {
        self.reg
    }

    pub const fn set_reg(&mut self, val: u8) {
        self.reg = val;
    }

    /// The HDMA copies bytes into OAM while an OAM DMA is running, but only
    /// on the M-cycle phase where the DMA has the bus free.
    #[must_use]
    pub const fn hdma_can_write_oam(&self, double_speed: bool) -> bool {
        self.is_active() && (self.cycles_modulo == 2 || double_speed)
    }

    pub const fn add_cycles(&mut self, cycles: i32) {
        self.cycles = cycles;
    }

    pub fn write(&mut self, val: u8) {
        self.restarting = self.current_dest != INACTIVE && self.current_dest != 0xA0;
        self.cycles = 0;
        self.cycles_modulo = 2;
        self.current_dest = 0xFF;
        self.current_src = u16::from(val) << 8;
        self.reg = val;
    }
}

const fn bus_for_addr(cgb: bool, addr: u16) -> Bus {
    if addr < 0x8000 {
        Bus::Main
    } else if addr < 0xA000 {
        Bus::Vram
    } else if addr < 0xC000 || !cgb {
        Bus::Main
    } else {
        Bus::Ram
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Whether a CPU access to `addr` conflicts with the running transfer.
    fn is_addr_in_dma_use(&self, addr: u16) -> bool {
        let dma = &self.dma;
        if !dma.is_active() || addr >= 0xFE00 || self.hdma.is_transferring() {
            return false;
        }
        // Warm-up.
        if dma.current_dest == 0xFF || dma.current_dest == 0 {
            return false;
        }
        // Shortcut for the DMA's own access flow.
        if dma.current_src == addr {
            return false;
        }
        if dma.current_src >= 0xE000 && (dma.current_src & !0x2000) == addr {
            return false;
        }
        let cgb = self.model.is_cgb_hardware();
        if cgb {
            if addr >= 0xC000 {
                return bus_for_addr(cgb, dma.current_src) != Bus::Vram;
            }
            if dma.current_src >= 0xE000 {
                return bus_for_addr(cgb, addr) != Bus::Vram;
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
        if cgb && bus_for_addr(cgb, addr) == Bus::Main && src >= 0xE000 {
            // Cart specific.
            return None;
        }
        if cgb && addr >= 0xC000 && (bus_for_addr(cgb, src) != Bus::Ram || src >= 0xE000) {
            return Some((src.wrapping_sub(1) & 0x1000) | (addr & 0xFFF) | 0xC000);
        }
        Some(src.wrapping_sub(1))
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
        let mut addr = addr;
        if !cgb {
            // The write only reaches the OAM byte the DMA is writing (ANDed
            // with it when the DMA reads the work RAM).
            let current = src.wrapping_sub(1);
            let oam = self.ppu.oam_mut().bytes_mut();
            if let Some(byte) = oam.get_mut(usize::from(self.dma.current_dest.wrapping_sub(1))) {
                *byte = if current >= 0xC000 {
                    *byte & value
                } else {
                    value
                };
            }
            return None;
        }
        if cgb && bus_for_addr(cgb, addr) == Bus::Main && src >= 0xE000 {
            // Cart specific.
            return None;
        }

        if cgb && !(0xC000..0xE000).contains(&src) && addr >= 0xC000 {
            return Some((src.wrapping_sub(1) & 0x1000) | (addr & 0xFFF) | 0xC000);
        } else if cgb && src >= 0xE000 && addr >= 0xC000 {
            addr = (src.wrapping_sub(1) & 0x1000) | (addr & 0xFFF) | 0xC000;
        } else {
            addr = src.wrapping_sub(1);
        }

        if cgb || addr >= 0xA000 {
            let oam_index = usize::from(self.dma.current_dest.wrapping_sub(1));
            let before_cgb_c = !cgb || matches!(model, Model::Cgb0 | Model::CgbA | Model::CgbB);
            let before_cgb_e = before_cgb_c || matches!(model, Model::CgbC | Model::CgbD);
            let oam = self.ppu.oam_mut().bytes_mut();
            if oam_index < oam.len() {
                if addr < 0xA000 {
                    oam[oam_index] = 0;
                } else if !cgb || model == Model::CgbB {
                    oam[oam_index] &= value;
                } else if before_cgb_c || model == Model::Agb {
                    oam[oam_index] = value;
                } else {
                    // CGB-C, D and E: the OAM is left alone.
                }
            }
            if before_cgb_e || addr >= 0xA000 {
                return None;
            }
        }
        Some(addr)
    }

    /// SameBoy's `write_oam`: a byte the HDMA drops into OAM (only the low
    /// byte of the source address is used). Past the 160 bytes it goes to the
    /// memory behind the unusable area on some revisions.
    pub(crate) fn hdma_write_oam(&mut self, addr: u8, value: u8) {
        let oam = self.ppu.oam_mut();
        if addr < 0xA0 {
            oam.bytes_mut()[usize::from(addr)] = value;
            return;
        }
        let addr = match self.model {
            Model::CgbD => {
                if addr >= 0xC0 {
                    addr | 0xF0
                } else {
                    addr
                }
            }
            Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC => addr & !0x18,
            _ => return,
        };
        oam.extra_mut()[usize::from(addr - 0xA0)] = value;
    }

    /// DMA source read: unlike the CPU it is not blocked by the PPU.
    fn dma_read(&self, addr: u16) -> u8 {
        if (0x8000..0xA000).contains(&addr) {
            self.ppu.vram().read(addr)
        } else {
            self.read_mem(addr)
        }
    }

    /// Port of `GB_dma_run`: transfers the bytes that became due.
    pub(crate) fn run_dma(&mut self) {
        if !self.dma.is_active() || self.hdma.cpu_halted() || self.clock.stopped {
            return;
        }
        let cgb = self.model.is_cgb_hardware();
        let mut cycles = self.dma.cycles + self.dma.cycles_modulo;
        while cycles >= 4 {
            cycles -= 4;
            if self.dma.current_dest >= 0xA0 {
                self.dma.current_dest = self.dma.current_dest.wrapping_add(1);
                self.ppu.dma_finished(&mut self.ints);
                break;
            }
            let dest = self.dma.current_dest;
            self.dma.current_dest = self.dma.current_dest.wrapping_add(1);
            let src = self.dma.current_src;
            if self.hdma.is_transferring()
                && (self.hdma.has_multiple_steps_left() || !self.hdma.is_at_block_end())
            {
                // The HDMA owns the bus: nothing is copied.
            } else if src < 0xE000 {
                let value = self.dma_read(src);
                self.ppu.write_oam_by_dma(u16::from(dest) | 0xFE00, value);
            } else if cgb {
                self.ppu.write_oam_by_dma(u16::from(dest) | 0xFE00, 0xFF);
            } else {
                let value = self.dma_read(src & !0x2000);
                self.ppu.write_oam_by_dma(u16::from(dest) | 0xFE00, value);
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
