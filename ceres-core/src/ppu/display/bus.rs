//! The PPU's own memory accesses, which fight the OAM DMA and the HDMA for
//! the bus, and the locks that keep the CPU out of the memory the PPU uses.

use crate::ppu::Ppu;

/// What the PPU sees of the DMA and HDMA, and of STOP mode.
#[expect(clippy::struct_excessive_bools, reason = "Independent bus states")]
#[derive(Clone)]
pub struct PpuBus {
    /// OAM index the DMA is writing (`0xA1`: no transfer).
    pub dma_dest: u8,
    /// Where `dma_dest` will be once the T-cycles the PPU is running are
    /// accounted for (the DMA catches up after the PPU).
    pub dma_dest_next: u8,
    /// T-cycles of that chunk still to run after the current one.
    pub chunk_left: i32,
    /// Address the OAM DMA reads next.
    pub dma_src: u16,
    /// The OAM DMA is part-way through a byte (`dma_cycles_modulo != 0`).
    pub dma_modulo: bool,
    /// An HDMA burst is running and the byte it reads next.
    pub hdma_in_progress: bool,
    pub hdma_src: u16,
    /// VRAM address the PPU read while an HDMA burst ran (0xFFFF: none).
    pub addr_for_hdma_conflict: u16,
    /// The PPU and an OAM DMA from VRAM fought over the bus in this byte.
    pub dma_ppu_vram_conflict: bool,
    pub dma_ppu_vram_conflict_addr: u16,
    /// In STOP mode the PPU's own accesses are blocked (SameBoy's `*_ppu_blocked`).
    pub oam_ppu_blocked: bool,
    pub vram_ppu_blocked: bool,
    pub cgb_palettes_ppu_blocked: bool,
    /// The CPU is halted or stopped.
    pub cpu_idle: bool,
}

impl Default for PpuBus {
    fn default() -> Self {
        Self {
            dma_dest: 0xA1,
            dma_dest_next: 0xA1,
            chunk_left: 0,
            dma_src: 0,
            dma_modulo: false,
            hdma_in_progress: false,
            hdma_src: 0,
            addr_for_hdma_conflict: 0xFFFF,
            dma_ppu_vram_conflict: false,
            dma_ppu_vram_conflict_addr: 0,
            oam_ppu_blocked: false,
            vram_ppu_blocked: false,
            cgb_palettes_ppu_blocked: false,
            cpu_idle: false,
        }
    }
}

/// CPU-visible access blocking, set at the same points as SameBoy.
#[expect(
    clippy::struct_excessive_bools,
    reason = "One lock per memory and direction"
)]
#[derive(Clone, Default)]
pub struct CpuAccess {
    pub oam_read_blocked: bool,
    pub oam_write_blocked: bool,
    pub vram_read_blocked: bool,
    pub vram_write_blocked: bool,
    pub cgb_palettes_blocked: bool,
}

impl CpuAccess {
    /// The CPU can access the OAM and the VRAM.
    pub const fn unlock_oam_vram(&mut self) {
        self.oam_read_blocked = false;
        self.oam_write_blocked = false;
        self.vram_read_blocked = false;
        self.vram_write_blocked = false;
    }

    /// The CPU can access the OAM, the VRAM and the CGB palettes.
    pub const fn unlock_all(&mut self) {
        self.unlock_oam_vram();
        self.cgb_palettes_blocked = false;
    }
}

impl Ppu {
    /// VRAM at `address` (0x2000.. is bank 1), no bus conflicts.
    pub(super) const fn vram_raw(&self, address: u16) -> u8 {
        if address >= 0x2000 {
            self.vram.vram_at_bank(address - 0x2000, 1)
        } else {
            self.vram.vram_at_bank(address, 0)
        }
    }

    /// The PPU's VRAM read (SameBoy's `vram_read`): blocked in STOP mode, and
    /// it fights the HDMA and an OAM DMA that reads VRAM for the bus.
    pub(super) fn vram_read(&mut self, address: u16) -> u8 {
        if self.d.bus.vram_ppu_blocked {
            return 0xFF;
        }
        let mut address = address;
        if self.d.bus.hdma_in_progress {
            self.d.bus.addr_for_hdma_conflict = address;
            return 0;
        }
        let dest = self.d.bus.dma_dest;
        if (1..=0xA0).contains(&dest) && self.d.bus.dma_src & 0xE000 == 0x8000 {
            // DMAing from VRAM!
            let offset = 1 - u16::from(self.d.bus.cpu_idle);
            if self.hw_cgb() {
                if self.d.bus.dma_ppu_vram_conflict {
                    address = (self.d.bus.dma_ppu_vram_conflict_addr & 0x1FFF) | (address & 0x2000);
                } else if self.d.bus.dma_modulo && !self.d.bus.cpu_idle {
                    address &= 0x2000;
                    address |= self.d.bus.dma_src.wrapping_sub(offset) & 0x1FFF;
                } else {
                    address &= 0x2000 | (self.d.bus.dma_src.wrapping_sub(offset) & 0x1FFF);
                    self.d.bus.dma_ppu_vram_conflict_addr = address;
                    self.d.bus.dma_ppu_vram_conflict = !self.d.bus.cpu_idle;
                }
            } else {
                address |= self.d.bus.dma_src.wrapping_sub(offset) & 0x1FFF;
            }
            let bank = u16::from(self.vram.read_vbk() & 1) * 0x2000;
            let value = self.vram_raw((address & 0x1FFF) | bank);
            let index = usize::from(dest.wrapping_sub(u8::from(!self.d.bus.cpu_idle)));
            if let Some(byte) = self.oam.bytes_mut().get_mut(index) {
                *byte = value;
            }
        }
        self.vram_raw(address)
    }

    /// The DMA copies `dest` (an OAM index, `0xA1` when idle) next.
    /// The OAM DMA's state, as the PPU sees it.
    pub const fn set_dma_state(&mut self, dest: u8, src: u16, modulo: bool) {
        self.d.bus.dma_dest = dest;
        self.d.bus.dma_src = src;
        self.d.bus.dma_modulo = modulo;
    }

    /// The PPU is about to run `cycles` T-cycles, after which the OAM DMA
    /// will have reached `dest_next`.
    pub const fn set_dma_lookahead(&mut self, dest_next: u8, cycles: i32) {
        self.d.bus.dma_dest_next = dest_next;
        self.d.bus.chunk_left = cycles;
    }

    /// One T-cycle of the chunk set by `set_dma_lookahead` is about to run.
    pub(in crate::ppu) const fn count_chunk_cycle(&mut self) {
        self.d.bus.chunk_left -= 1;
    }

    /// A new DMA byte starts: the bus fight of the last one is over.
    pub const fn clear_dma_vram_conflict(&mut self) {
        self.d.bus.dma_ppu_vram_conflict = false;
    }

    /// An HDMA burst starts (`true`) or ends; `src` is the byte it reads next.
    pub const fn set_hdma_state(&mut self, in_progress: bool, src: u16) {
        self.d.bus.hdma_in_progress = in_progress;
        self.d.bus.hdma_src = src;
        if in_progress {
            self.d.bus.addr_for_hdma_conflict = 0xFFFF;
        }
    }

    /// The VRAM address the PPU read during the last HDMA byte, if it did.
    pub const fn take_hdma_conflict_addr(&mut self) -> Option<u16> {
        let addr = self.d.bus.addr_for_hdma_conflict;
        self.d.bus.addr_for_hdma_conflict = 0xFFFF;
        if addr == 0xFFFF { None } else { Some(addr) }
    }

    /// STOP: the PPU's accesses are blocked (unless the CPU's already were).
    pub const fn block_ppu_accesses(&mut self, blocked: bool) {
        self.d.bus.oam_ppu_blocked = blocked && !self.d.cpu.oam_read_blocked;
        self.d.bus.vram_ppu_blocked = blocked && !self.d.cpu.vram_read_blocked;
        self.d.bus.cgb_palettes_ppu_blocked = blocked && !self.d.cpu.cgb_palettes_blocked;
    }

    pub const fn set_cpu_idle(&mut self, idle: bool) {
        self.d.bus.cpu_idle = idle;
    }

    // CPU-visible memory access, gated by the flags above.

    #[must_use]
    pub fn vram_read_blocked(&self) -> bool {
        self.gstat_mode3_lock(79)
            .unwrap_or(self.d.cpu.vram_read_blocked)
    }

    #[must_use]
    pub fn vram_write_blocked(&self) -> bool {
        self.gstat_mode3_lock(79)
            .unwrap_or(self.d.cpu.vram_write_blocked)
    }

    #[must_use]
    pub const fn oam_read_blocked(&self) -> bool {
        self.d.cpu.oam_read_blocked
    }
}
