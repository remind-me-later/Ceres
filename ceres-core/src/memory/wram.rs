use super::svbk::Svbk;

#[derive(Debug)]
pub struct Wram {
    svbk: Svbk,
    wram: Box<[u8; Self::SIZE_CGB as usize]>,
}

impl Default for Wram {
    fn default() -> Self {
        #[expect(
            clippy::unwrap_used,
            reason = "RGB_BUF_SIZE is a constant, so this will never panic."
        )]
        Self {
            wram: vec![0; Self::SIZE_CGB as usize]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            svbk: Svbk::default(),
        }
    }
}

impl Wram {
    pub const SIZE_CGB: u16 = Self::SIZE_GB * 4;
    pub const SIZE_GB: u16 = 0x2000;

    #[must_use]
    pub const fn read_wram_hi(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xFFF | self.svbk.bank_offset()) as usize]
    }

    #[must_use]
    pub const fn read_wram_lo(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xFFF) as usize]
    }

    #[must_use]
    pub const fn svbk(&self) -> &Svbk {
        &self.svbk
    }

    pub const fn svbk_mut(&mut self) -> &mut Svbk {
        &mut self.svbk
    }

    #[must_use]
    pub const fn wram(&self) -> &[u8; Self::SIZE_CGB as usize] {
        &self.wram
    }

    pub const fn wram_mut(&mut self) -> &mut [u8; Self::SIZE_CGB as usize] {
        &mut self.wram
    }

    pub fn write_wram_hi(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xFFF | self.svbk.bank_offset()) as usize] = val;
    }

    pub fn write_wram_lo(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xFFF) as usize] = val;
    }
}
