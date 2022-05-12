pub use {dma::Dma, hdma::Hdma, hram::Hram, key1::Key1, wram::Wram};

use crate::Gb;

mod addresses;
mod dma;
mod hdma;
mod hram;
mod key1;
mod wram;

impl Gb {
    pub fn switch_speed(&mut self) {
        self.in_double_speed = !self.in_double_speed;
    }

    pub fn tick_t_cycle(&mut self) {
        self.emulate_dma();
        self.emulate_hdma();
        self.tick_ppu();
        self.timer.tick_t_cycle(&mut self.ints);
        self.tick_apu();
    }

    fn tick_ppu(&mut self) {
        let mus_elapsed = self.mus_since_last_tick();
        self.ppu
            .tick(&mut self.ints, self.function_mode, mus_elapsed);
    }

    fn mus_since_last_tick(&self) -> u8 {
        if self.in_double_speed { 2 } else { 4 }
    }

    fn tick_apu(&mut self) {
        let mus_elapsed = self.mus_since_last_tick();
        self.apu.tick(mus_elapsed);
    }

    fn emulate_hdma(&mut self) {
        if self.hdma.start(&self.ppu) {
            let tick = |gb: &mut Gb| {
                gb.emulate_dma();
                gb.tick_ppu();
                gb.timer.tick_t_cycle(&mut gb.ints);
                gb.tick_apu();
            };

            while !self.hdma.is_transfer_done() {
                let transfer = self.hdma.transfer();
                let addr = transfer.src;
                let val = match addr >> 8 {
                    0x00..=0x7f => self.cart.read_rom(addr),
                    // TODO: should copy garbage
                    0x80..=0x9f => 0xff,
                    0xa0..=0xbf => self.cart.read_ram(addr),
                    0xc0..=0xcf => self.wram.read_ram(addr),
                    0xd0..=0xdf => self.wram.read_bank_ram(addr),
                    _ => panic!("Illegal source addr for HDMA transfer"),
                };

                tick(self);
                self.ppu.hdma_write(transfer.dst, val);
            }
        }
    }

    // FIXME: sprites are not displayed during OAM DMA
    fn emulate_dma(&mut self) {
        if let Some(src) = self.dma.emulate() {
            let val = match src >> 8 {
                0x00..=0x7f => self.cart.read_rom(src),
                0x80..=0x9f => self.ppu.read_vram(src),
                0xa0..=0xbf => self.cart.read_ram(src),
                0xc0..=0xcf | 0xe0..=0xef => self.wram.read_ram(src),
                0xd0..=0xdf | 0xf0..=0xff => self.wram.read_bank_ram(src),
                _ => panic!("Illegal source addr for OAM DMA transfer"),
            };

            self.ppu.dma_write((src & 0xff) as u8, val);
        }
    }
}
