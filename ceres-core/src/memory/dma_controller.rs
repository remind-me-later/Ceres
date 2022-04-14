mod oam_dma;
mod vram_dma;

use self::oam_dma::OamDma;
pub use self::vram_dma::VramDMATransfer;
use self::vram_dma::VramDma;
use crate::video::ppu::Ppu;

#[derive(Clone, Copy)]
pub enum DmaRegister {
    Dma,
    HDMA1,
    HDMA2,
    HDMA3,
    HDMA4,
    HDMA5,
}

pub struct DmaController {
    oam_dma_controller: OamDma,
    hdma_controller: VramDma,
}

impl DmaController {
    pub const fn new() -> Self {
        Self {
            oam_dma_controller: OamDma::new(),
            hdma_controller: VramDma::new(),
        }
    }

    pub const fn is_dma_active(&self) -> bool {
        self.oam_dma_controller.is_active()
    }

    pub fn read(&self, register: DmaRegister) -> u8 {
        match register {
            DmaRegister::Dma => self.oam_dma_controller.read(),
            DmaRegister::HDMA5 => self.hdma_controller.read_hdma5(),
            _ => 0xff,
        }
    }

    pub fn write(&mut self, register: DmaRegister, val: u8) {
        match register {
            DmaRegister::Dma => self.oam_dma_controller.write(val),
            DmaRegister::HDMA1 => self.hdma_controller.write_hdma1(val),
            DmaRegister::HDMA2 => self.hdma_controller.write_hdma2(val),
            DmaRegister::HDMA3 => self.hdma_controller.write_hdma3(val),
            DmaRegister::HDMA4 => self.hdma_controller.write_hdma4(val),
            DmaRegister::HDMA5 => self.hdma_controller.write_hdma5(val),
        }
    }

    pub fn emulate_oam_dma(&mut self, _ppu: &Ppu) -> Option<u16> {
        // TODO: take into account ppu state?
        self.oam_dma_controller.emulate()
    }

    pub fn start_transfer(&mut self, ppu: &Ppu, microseconds_elapsed_times_16: u8) -> bool {
        self.hdma_controller
            .start_transfer(ppu, microseconds_elapsed_times_16)
    }

    pub fn do_vram_transfer(
        &mut self,
        microseconds_elapsed_times_16: u8,
    ) -> Option<VramDMATransfer> {
        self.hdma_controller
            .do_vram_transfer(microseconds_elapsed_times_16)
    }

    pub fn vram_dma_is_done(&self) -> bool {
        self.hdma_controller.done()
    }
}
