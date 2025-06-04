pub const WRAM_SIZE_GB: u16 = 0x2000;
pub const WRAM_SIZE_CGB: u16 = WRAM_SIZE_GB * 4;

#[derive(Debug)]
pub struct Wram {
    svbk: Svbk,
    wram: Box<[u8; WRAM_SIZE_CGB as usize]>,
}

impl Default for Wram {
    fn default() -> Self {
        #[expect(
            clippy::unwrap_used,
            reason = "RGB_BUF_SIZE is a constant, so this will never panic."
        )]
        Self {
            wram: vec![0; WRAM_SIZE_CGB as usize]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            svbk: Svbk::default(),
        }
    }
}

impl Wram {
    #[must_use]
    pub const fn read_wram_lo(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xFFF) as usize]
    }

    pub fn write_wram_lo(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xFFF) as usize] = val;
    }

    #[must_use]
    pub const fn read_wram_hi(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xFFF | self.svbk.bank_offset()) as usize]
    }

    pub fn write_wram_hi(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xFFF | self.svbk.bank_offset()) as usize] = val;
    }

    #[must_use]
    pub const fn svbk(&self) -> &Svbk {
        &self.svbk
    }

    pub const fn svbk_mut(&mut self) -> &mut Svbk {
        &mut self.svbk
    }

    #[must_use]
    pub const fn wram(&self) -> &[u8; WRAM_SIZE_CGB as usize] {
        &self.wram
    }

    pub const fn wram_mut(&mut self) -> &mut [u8; WRAM_SIZE_CGB as usize] {
        &mut self.wram
    }
}

#[derive(Default, Debug)]
pub struct Svbk {
    svbk: u8,
}

impl Svbk {
    #[must_use]
    pub const fn read(&self) -> u8 {
        self.svbk | 0xF8
    }

    pub const fn write(&mut self, val: u8) {
        self.svbk = val & 7;
    }

    #[must_use]
    pub const fn bank_offset(&self) -> u16 {
        // Return value between 0x1000 and 0x7000
        (if self.svbk == 0 { 1 } else { self.svbk } as u16) * 0x1000
    }
}
