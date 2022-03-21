mod hdma;
mod oam_dma;

pub use self::hdma::HDMATransfer;
use self::hdma::Hdma;
use self::oam_dma::OamDma;
use crate::video::Ppu;

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
    hdma_controller: Hdma,
}

impl DmaController {
    pub const fn new() -> Self {
        Self {
            oam_dma_controller: OamDma::new(),
            hdma_controller: Hdma::new(),
        }
    }

    pub const fn is_dma_active(&self) -> bool {
        self.oam_dma_controller.is_active()
    }

    pub fn read(&self, register: DmaRegister) -> u8 {
        match register {
            DmaRegister::Dma => self.oam_dma_controller.read(),
            DmaRegister::HDMA1 | DmaRegister::HDMA2 | DmaRegister::HDMA3 | DmaRegister::HDMA4 => {
                0xff
            }
            DmaRegister::HDMA5 => self.hdma_controller.read_hdma5(),
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

    pub fn emulate_hdma(
        &mut self,
        ppu: &Ppu,
        microseconds_elapsed_times_16: u8,
    ) -> Option<HDMATransfer> {
        self.hdma_controller
            .emulate(ppu, microseconds_elapsed_times_16)
    }
}
