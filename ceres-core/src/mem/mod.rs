pub use dma::HdmaState;

use crate::Gb;

mod addresses;
mod dma;

pub const HIGH_RAM_SIZE: usize = 0x80;
pub const KEY1_SPEED: u8 = 0x80;
pub const KEY1_SWITCH: u8 = 0x01;
const WRAM_SIZE: usize = 0x2000;
pub const WRAM_SIZE_CGB: usize = WRAM_SIZE * 4;

impl Gb {
    #[must_use]
    pub(crate) fn read_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xfff) as usize]
    }

    pub(crate) fn write_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xfff) as usize] = val;
    }

    #[must_use]
    pub(crate) fn read_bank_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xfff | (self.svbk_true as u16 * 0x1000)) as usize]
    }

    pub(crate) fn write_bank_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xfff | (self.svbk_true as u16 * 0x1000)) as usize] = val;
    }

    pub(crate) fn tick_t_cycle(&mut self) {
        self.emulate_dma();
        self.emulate_hdma();
        self.tick_ppu();
        self.tick_timer();
        self.tick_apu();
    }

    #[must_use]
    pub(crate) fn t_elapsed(&self) -> u8 {
        if self.double_speed { 2 } else { 4 }
    }
}
