//! OAM DMA.
//!
//! Port of SameBoy's `GB_dma_run` and the bus conflicts of `GB_read_memory` /
//! `GB_write_memory`: while a transfer runs, CPU accesses that share a bus with
//! the DMA source read (or write) whatever the DMA is currently accessing.

#[cfg(kani)]
mod netlist_proofs;

use {
    super::{CART_RAM_START, ECHO_B, ECHO_START, OAM_START, VRAM_START, WRAM_START, Wram},
    crate::{
        AudioCallback, Gb, Model,
        ppu::{Oam, unusable_index},
    },
};

/// Where an OAM DMA is. Each step takes an M-cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DmaPhase {
    #[default]
    Idle,
    /// The start-up delay after a write to the DMA register.
    StartUp,
    /// Copying: the next step writes OAM byte `next` (0 to 0x9F).
    Copy { next: u8 },
    /// The step after the last byte, which hands OAM back.
    Last,
}

impl DmaPhase {
    /// The phase after a step.
    const fn step(self) -> Self {
        match self {
            Self::StartUp => Self::Copy { next: 0 },
            Self::Copy { next } if next + 1 < Oam::SIZE => Self::Copy { next: next + 1 },
            Self::Copy { .. } => Self::Last,
            Self::Last | Self::Idle => Self::Idle,
        }
    }

    /// The bytes written so far, once there is one (1 to 0xA0).
    #[must_use]
    pub(crate) const fn written(self) -> Option<u8> {
        match self {
            Self::Copy { next } if next != 0 => Some(next),
            Self::Last => Some(Oam::SIZE),
            _ => None,
        }
    }
}

/// What a step of the transfer does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DmaStep {
    /// Copies the byte at `src` to OAM byte `dest`.
    Copy { src: u16, dest: u8 },
    /// The start-up delay or the last step, after which the run stops. A
    /// restarted transfer already copies, in its start-up delay, byte 0
    /// from the new source (`src` to OAM byte `dest`). `Dma::end_pause`
    /// ends the step.
    Pause { rewrite: Option<(u16, u8)> },
}

pub(crate) struct Dma {
    /// Cycles to process on the next `Gb::run_dma`.
    cycles: i32,
    /// Cycles left over from the previous run (a byte takes 4).
    cycles_modulo: i32,
    phase: DmaPhase,
    current_src: u16,
    reg: u8,
    /// A new transfer was started over one on its way or running: the DMA
    /// runs on through the start-up delay, which copies byte 0 (as the
    /// netlist's restart clears its count while it still runs).
    // @fact dma-restart-bytes
    restarting: bool,
}

impl Default for Dma {
    fn default() -> Self {
        Self {
            cycles: 0,
            cycles_modulo: 0,
            phase: DmaPhase::Idle,
            current_src: 0,
            reg: 0xFF,
            restarting: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DmaBus {
    Main,
    Ram,
    Vram,
}

#[cfg(feature = "debug")]
impl Dma {
    pub(crate) const fn debug_state(&self) -> crate::OamDmaState {
        crate::OamDmaState {
            phase: match self.phase {
                DmaPhase::Idle => crate::OamDmaPhase::Idle,
                DmaPhase::StartUp => crate::OamDmaPhase::StartUp,
                DmaPhase::Copy { next } => crate::OamDmaPhase::Copy { next },
                DmaPhase::Last => crate::OamDmaPhase::Last,
            },
            source: self.current_src,
        }
    }
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
        !matches!(self.phase, DmaPhase::Idle)
    }

    /// The next step of the transfer is its last one, which hands OAM back.
    #[must_use]
    pub(crate) const fn is_in_last_step(&self) -> bool {
        matches!(self.phase, DmaPhase::Last)
    }

    /// The first byte is next, and the DMA does not run yet.
    const fn is_warming_up(&self) -> bool {
        matches!(self.phase, DmaPhase::StartUp)
            || (matches!(self.phase, DmaPhase::Copy { next: 0 }) && !self.restarting)
    }

    /// The DMA owns OAM: CPU reads of it return 0xFF and CPU writes to it
    /// are dropped. It does not yet in the M-cycle after a write to FF46,
    /// unless a transfer was running.
    #[must_use]
    // @fact dma-oam-write-after-start
    pub(crate) const fn blocks_oam(&self) -> bool {
        self.is_active() && (!matches!(self.phase, DmaPhase::Copy { next: 0 }) || self.restarting)
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

    /// The M-cycle after a restart, in which the hardware writes byte 0, as
    /// the start-up delay copied it.
    const fn is_after_restart(&self) -> bool {
        matches!(self.phase, DmaPhase::Copy { next: 0 }) && self.restarting
    }

    /// The OAM byte copied last, which the hardware writes in this M-cycle,
    /// once one was.
    const fn oam_byte(&self) -> Option<u8> {
        if self.is_after_restart() {
            return Some(0);
        }
        match self.phase.written() {
            Some(written) => Some(written - 1),
            None => None,
        }
    }

    /// The OAM index of the byte being copied, once one was.
    const fn oam_index(&self) -> Option<usize> {
        match self.oam_byte() {
            Some(byte) => Some(byte as usize),
            None => None,
        }
    }

    /// The source address of the byte being copied.
    const fn copied_src(&self) -> u16 {
        if self.is_after_restart() {
            self.current_src
        } else {
            self.current_src.wrapping_sub(1)
        }
    }

    /// The T-cycles the next `run_dma` runs (SameBoy's `dma_cycles`).
    pub(crate) const fn set_cycles(&mut self, cycles: i32) {
        self.cycles = cycles;
    }

    /// Starts a run of the T-cycles that `set_cycles` gave, with those left
    /// from the last run.
    pub(crate) const fn begin_run(&mut self) {
        self.cycles_modulo += self.cycles;
        self.cycles = 0;
    }

    /// The run's next step, while a whole one (4 T-cycles) is left. A copy
    /// moves on at once (the source read already sees the phase after it);
    /// a pause moves on with `end_pause`, after its rewrite (whose read still
    /// sees the start-up delay), and ends the run.
    pub(crate) const fn due_step(&mut self) -> Option<DmaStep> {
        if self.cycles_modulo < 4 {
            return None;
        }
        self.cycles_modulo -= 4;
        Some(self.step())
    }

    const fn step(&mut self) -> DmaStep {
        let DmaPhase::Copy { next: dest } = self.phase else {
            let rewrite = if matches!(self.phase, DmaPhase::StartUp) && self.restarting {
                Some((self.current_src, 0))
            } else {
                None
            };
            return DmaStep::Pause { rewrite };
        };
        self.phase = self.phase.step();
        let src = self.current_src;
        self.current_src = src.wrapping_add(1);
        DmaStep::Copy { src, dest }
    }

    /// Ends a pause (see `step`).
    pub(crate) const fn end_pause(&mut self) {
        self.phase = self.phase.step();
    }

    /// A write to FF46. Returns the byte the hardware writes to OAM in this
    /// M-cycle, read again: FF46 is a latch, open while the CPU writes it, so
    /// the byte copied last comes from the new source (`src` to OAM byte
    /// `dest`). A write in the M-cycle after another one only changes the
    /// source: the start it asks for is the one already on its way.
    // @fact dma-restart-bytes
    pub(crate) fn write(&mut self, val: u8) -> Option<(u16, u8)> {
        let last = self.oam_byte();
        if !matches!(self.phase, DmaPhase::StartUp | DmaPhase::Copy { next: 0 }) {
            self.restarting = !matches!(self.phase, DmaPhase::Idle | DmaPhase::Last);
            self.cycles = 0;
            self.cycles_modulo = 2;
            self.phase = DmaPhase::StartUp;
        }
        self.current_src = u16::from(val) << 8;
        self.reg = val;
        last.map(|dest| (self.current_src | u16::from(dest), dest))
    }
}

/// The work RAM address a CPU access that conflicts with the transfer
/// reaches on the CGB: the bank half the DMA reads, at the CPU's offset.
const fn wram_conflict_addr(src: u16, addr: u16) -> u16 {
    (src.wrapping_sub(1) & Wram::BANK_SIZE) | (addr & (Wram::BANK_SIZE - 1)) | WRAM_START
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
        if dma.is_warming_up() {
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
        if self.dma.is_after_restart() {
            // Byte 0 of the new source.
            return Some(src);
        }
        if cgb && bus_for_addr(cgb, addr) == DmaBus::Main && src >= ECHO_START {
            // Cart specific.
            return None;
        }
        if cgb && addr >= WRAM_START && (bus_for_addr(cgb, src) != DmaBus::Ram || src >= ECHO_START)
        {
            return Some(wram_conflict_addr(src, addr));
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
        let src = self.dma.copied_src();
        if self.model == Model::CgbC
            && (VRAM_START..CART_RAM_START).contains(&src)
            && self.is_addr_in_dma_use(addr)
            && let Some(index) = self.dma.oam_index()
            && let Some(byte) = self.ppu.oam_mut().bytes_mut().get_mut(index)
        {
            *byte = 0;
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
            let current = self.dma.copied_src();
            let index = self.dma.oam_index();
            let oam = self.ppu.oam_mut().bytes_mut();
            if let Some(byte) = index.and_then(|i| oam.get_mut(i)) {
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
            let current = self.dma.copied_src();
            let index = self.dma.oam_index();
            let oam = self.ppu.oam_mut().bytes_mut();
            if let Some(byte) = index.and_then(|i| oam.get_mut(i)) {
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
            return Some(wram_conflict_addr(src, addr));
        }

        let current = self.dma.copied_src();
        let index = self.dma.oam_index();
        let before_cgb_c = matches!(model, Model::Cgb0 | Model::CgbA | Model::CgbB);
        let before_cgb_e = before_cgb_c || matches!(model, Model::CgbC | Model::CgbD);
        let oam = self.ppu.oam_mut().bytes_mut();
        if let Some(byte) = index.and_then(|i| oam.get_mut(i)) {
            if current < CART_RAM_START {
                *byte = 0;
            } else if model == Model::CgbB {
                *byte &= value;
            } else if before_cgb_c || model == Model::Agb {
                *byte = value;
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

    /// The CPU writes FF46.
    pub(crate) fn write_dma(&mut self, val: u8) {
        if let Some((src, dest)) = self.dma.write(val) {
            self.dma_copy(src, dest);
        }
        self.ppu
            .set_dma_state(self.dma.phase, self.dma.current_src, true);
    }

    /// Copies the byte at `src` to OAM byte `dest`. The CGB's DMA reads FF
    /// from E000 up, the DMG's reads the work RAM through its echo.
    fn dma_copy(&mut self, src: u16, dest: u8) {
        let value = if src < ECHO_START {
            self.dma_read(src)
        } else if self.model.is_cgb_hardware() {
            0xFF
        } else {
            self.dma_read(src & !ECHO_B)
        };
        self.ppu
            .write_oam_by_dma(OAM_START | u16::from(dest), value);
    }

    /// The phase `run_dma` will have reached after the next `cycles`.
    pub(crate) const fn dma_phase_after(&self, cycles: i32) -> DmaPhase {
        let mut phase = self.dma.phase;
        if !self.dma.is_active() || self.hdma.cpu_halted() || self.clock.stopped {
            return phase;
        }
        let mut due = (self.dma.cycles_modulo + cycles) / 4;
        while due > 0 {
            due -= 1;
            // A run stops after the start-up delay and after the last step.
            let stop = matches!(phase, DmaPhase::StartUp | DmaPhase::Last);
            phase = phase.step();
            if stop {
                break;
            }
        }
        phase
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
        self.dma.begin_run();
        while let Some(step) = self.dma.due_step() {
            let (src, dest) = match step {
                DmaStep::Copy { src, dest } => (src, dest),
                DmaStep::Pause { rewrite } => {
                    if let Some((src, dest)) = rewrite {
                        self.dma_copy(src, dest);
                    }
                    self.dma.end_pause();
                    self.ppu.dma_finished();
                    break;
                }
            };
            if self.hdma.is_transferring()
                && (self.hdma.has_multiple_steps_left() || !self.hdma.is_at_block_end())
            {
                // The HDMA owns the bus: nothing is copied.
            } else {
                self.dma_copy(src, dest);
            }
            self.ppu.clear_dma_vram_conflict();
        }
        self.ppu.set_dma_state(
            self.dma.phase,
            self.dma.current_src,
            self.dma.cycles_modulo != 0,
        );
    }
}
