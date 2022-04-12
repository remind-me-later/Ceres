use super::{
    ppu::BgAttributes,
    sprites::{SpriteAttributes, SpriteFlags},
};

const VRAM_BANK_SIZE: usize = 0x2000;

#[derive(Clone, Copy)]
pub enum VramBank {
    Bank0,
    Bank1,
}

impl VramBank {
    const fn multiplier(self) -> u16 {
        use VramBank::*;
        match self {
            Bank0 => 0,
            Bank1 => 1,
        }
    }
}

impl From<bool> for VramBank {
    fn from(val: bool) -> Self {
        use VramBank::*;
        match val {
            true => Bank1,
            false => Bank0,
        }
    }
}

impl From<u8> for VramBank {
    fn from(val: u8) -> Self {
        use VramBank::*;
        match val & 1 {
            0 => Bank0,
            _ => Bank1,
        }
    }
}

impl From<VramBank> for u8 {
    fn from(register: VramBank) -> Self {
        use VramBank::*;
        match register {
            Bank0 => 0,
            Bank1 => 1,
        }
    }
}

pub struct Vram {
    vram: [u8; VRAM_BANK_SIZE * 2],
    bank: VramBank,
}

impl core::ops::Index<usize> for Vram {
    type Output = u8;

    fn index(&self, idx: usize) -> &Self::Output {
        &self.vram[idx]
    }
}

impl core::ops::IndexMut<usize> for Vram {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.vram[idx]
    }
}

impl Vram {
    pub const fn new() -> Self {
        Self {
            vram: [0; VRAM_BANK_SIZE * 2],
            bank: VramBank::Bank0,
        }
    }

    pub fn read_bank_number(&self) -> u8 {
        const BANK_MASK: u8 = 0xfe;
        let bank: u8 = self.bank.into();
        bank | BANK_MASK
    }

    pub fn write_bank_number(&mut self, val: u8) {
        self.bank = val.into()
    }

    pub const fn read(&self, address: u16) -> u8 {
        let address = address & 0x1fff;
        let idx = address + self.bank.multiplier() * VRAM_BANK_SIZE as u16;
        self.vram[idx as usize]
    }

    pub fn write(&mut self, address: u16, val: u8) {
        let address = address & 0x1fff;
        let idx = address + self.bank.multiplier() * VRAM_BANK_SIZE as u16;
        self.vram[idx as usize] = val
    }

    pub fn get_bank(&self, address: u16, bank: VramBank) -> u8 {
        let address = address & 0x1fff;
        let idx = address + bank.multiplier() * VRAM_BANK_SIZE as u16;
        self.vram[idx as usize]
    }

    pub fn tile_number(&self, tile_address: u16) -> u8 {
        self.get_bank(tile_address, VramBank::Bank0)
    }

    pub fn background_attributes(&self, tile_address: u16) -> BgAttributes {
        BgAttributes::from_bits_truncate(self.get_bank(tile_address, VramBank::Bank1))
    }

    pub fn tile_data(
        &self,
        tile_data_address: u16,
        background_attributes: &BgAttributes,
    ) -> (u8, u8) {
        let low = self.get_bank(
            tile_data_address & 0x1fff,
            background_attributes
                .contains(BgAttributes::VRAM_BANK_NUMBER)
                .into(),
        );

        let high = self.get_bank(
            (tile_data_address & 0x1fff) + 1,
            background_attributes
                .contains(BgAttributes::VRAM_BANK_NUMBER)
                .into(),
        );

        (low, high)
    }

    pub fn sprite_data(
        &self,
        tile_data_address: u16,
        sprite_attributes: &SpriteAttributes,
    ) -> (u8, u8) {
        let low = self.get_bank(
            tile_data_address,
            sprite_attributes
                .flags()
                .contains(SpriteFlags::TILE_VRAM_BANK)
                .into(),
        );

        let high = self.get_bank(
            tile_data_address + 1,
            sprite_attributes
                .flags()
                .contains(SpriteFlags::TILE_VRAM_BANK)
                .into(),
        );

        (low, high)
    }
}
