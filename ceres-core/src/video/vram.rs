use super::{
    ppu::BG_TILE_BANK,
    sprites::{SpriteAttr, SPR_TILE_BANK},
};

const VRAM_SIZE: usize = 0x2000;
const VRAM_SIZE_CGB: usize = VRAM_SIZE * 2;

pub struct Vram {
    vram: [u8; VRAM_SIZE_CGB],
    cgb_vram_bank: u8, // 0 or 1
}

impl Default for Vram {
    fn default() -> Self {
        Self {
            vram: [0; VRAM_SIZE_CGB],
            cgb_vram_bank: 0,
        }
    }
}

impl Vram {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_bank_number(&self) -> u8 {
        self.cgb_vram_bank | 0xfe
    }

    pub fn write_bank_number(&mut self, val: u8) {
        self.cgb_vram_bank = val & 1
    }

    pub fn read(&self, addr: u16) -> u8 {
        self.vram[((addr & 0x1fff) + self.cgb_vram_bank as u16 * VRAM_SIZE as u16) as usize]
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        self.vram[((addr & 0x1fff) + self.cgb_vram_bank as u16 * VRAM_SIZE as u16) as usize] = val
    }

    pub fn get_bank(&self, addr: u16, bank: u8) -> u8 {
        self.vram[((addr & 0x1fff) + bank as u16 * VRAM_SIZE as u16) as usize]
    }

    pub fn tile_number(&self, tile_addr: u16) -> u8 {
        self.get_bank(tile_addr, 0)
    }

    pub fn background_attributes(&self, tile_addr: u16) -> u8 {
        self.get_bank(tile_addr, 1)
    }

    pub fn tile_data(&self, tile_data_addr: u16, bg_attr: u8) -> (u8, u8) {
        let low = self.get_bank(tile_data_addr & 0x1fff, (bg_attr & BG_TILE_BANK != 0) as u8);

        let high = self.get_bank(
            (tile_data_addr & 0x1fff) + 1,
            (bg_attr & BG_TILE_BANK != 0) as u8,
        );

        (low, high)
    }

    pub fn sprite_data(&self, tile_data_addr: u16, sprite_attributes: &SpriteAttr) -> (u8, u8) {
        let low = self.get_bank(
            tile_data_addr,
            (sprite_attributes.flags() & SPR_TILE_BANK != 0) as u8,
        );

        let high = self.get_bank(
            tile_data_addr + 1,
            (sprite_attributes.flags() & SPR_TILE_BANK != 0) as u8,
        );

        (low, high)
    }
}
