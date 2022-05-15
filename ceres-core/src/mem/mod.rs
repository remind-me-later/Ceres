pub use dma::{Dma, Hdma};

use crate::{ppu::Mode, Gb};

mod addresses;
mod dma;

pub const HIGH_RAM_SIZE: usize = 0x80;
pub const KEY1_SPEED: u8 = 0x80;
pub const KEY1_SWITCH: u8 = 0x01;
const WRAM_SIZE: usize = 0x2000;
pub const WRAM_SIZE_CGB: usize = WRAM_SIZE * 4;

impl Gb {
    #[must_use]
    pub fn read_svbk(&self) -> u8 {
        self.svbk | 0xf8
    }

    pub fn write_svbk(&mut self, val: u8) {
        let tmp = val & 7;
        self.svbk = tmp;
        self.svbk_bank = if tmp == 0 { 1 } else { tmp };
    }

    #[must_use]
    pub fn read_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xfff) as usize]
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xfff) as usize] = val;
    }

    #[must_use]
    pub fn read_bank_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xfff | (self.svbk_bank as u16 * 0x1000)) as usize]
    }

    pub fn write_bank_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xfff | (self.svbk_bank as u16 * 0x1000)) as usize] = val;
    }

    pub fn tick_t_cycle(&mut self) {
        self.emulate_dma();
        self.emulate_hdma();
        self.tick_ppu();
        self.timer.tick_t_cycle(&mut self.ints);
        self.tick_apu();
    }

    #[must_use]
    pub fn mus_since_last_tick(&self) -> u8 {
        if self.double_speed { 2 } else { 4 }
    }

    fn tick_apu(&mut self) {
        let mus_elapsed = self.mus_since_last_tick();
        self.apu.tick(mus_elapsed);
    }

    fn emulate_hdma(&mut self) {
        if self.hdma.start(self.ppu_mode() == Mode::HBlank) {
            let tick = |gb: &mut Gb| {
                gb.emulate_dma();
                gb.tick_ppu();
                gb.timer.tick_t_cycle(&mut gb.ints);
                gb.tick_apu();
            };

            while !self.hdma.is_transfer_done() {
                let (src, dst) = self.hdma.transfer();
                let val = match src >> 8 {
                    0x00..=0x7f => self.cart.read_rom(src),
                    // TODO: should copy garbage
                    0x80..=0x9f => 0xff,
                    0xa0..=0xbf => self.cart.read_ram(src),
                    0xc0..=0xcf => self.read_ram(src),
                    0xd0..=0xdf => self.read_bank_ram(src),
                    _ => panic!("Illegal source addr for HDMA transfer"),
                };

                tick(self);
                self.hdma_write(dst, val);
            }
        }
    }

    // FIXME: sprites are not displayed during OAM DMA
    fn emulate_dma(&mut self) {
        if let Some(src) = self.dma.emulate() {
            let val = match src >> 8 {
                0x00..=0x7f => self.cart.read_rom(src),
                0x80..=0x9f => self.read_vram(src),
                0xa0..=0xbf => self.cart.read_ram(src),
                0xc0..=0xcf | 0xe0..=0xef => self.read_ram(src),
                0xd0..=0xdf | 0xf0..=0xff => self.read_bank_ram(src),
                _ => panic!("Illegal source addr for OAM DMA transfer"),
            };

            self.dma_write((src & 0xff) as u8, val);
        }
    }
}
