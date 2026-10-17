use super::svbk::Svbk;
use alloc::{boxed::Box, vec};

pub(crate) struct Wram {
    svbk: Svbk,
    wram: Box<[u8; Self::SIZE_CGB as usize]>,
}

impl Default for Wram {
    fn default() -> Self {
        #[expect(
            clippy::unwrap_used,
            reason = "SIZE_CGB is a constant, so this will never panic."
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
    pub(crate) const SIZE_CGB: u16 = Self::SIZE_GB * 4;
    pub(crate) const SIZE_GB: u16 = 0x2000;
    /// Bank 0 at 0xC000, the switchable bank at 0xD000.
    pub(crate) const BANK_SIZE: u16 = 0x1000;

    /// The work RAM as the hardware powers on. It is random on every unit;
    /// the DMG's tends to come up as alternating blocks of 0xFF and 0x00.
    #[must_use]
    pub(crate) fn power_on(cgb_hardware: bool) -> Self {
        let mut wram = Self::default();
        if !cgb_hardware {
            let (blocks, _) = wram.wram[..Self::SIZE_GB.into()].as_chunks_mut::<0x100>();
            for (i, block) in blocks.iter_mut().enumerate() {
                block.fill(if i % 2 == 0 { 0xFF } else { 0x00 });
            }
        }
        wram
    }

    #[must_use]
    pub(crate) const fn read_wram_hi(&self, addr: u16) -> u8 {
        self.wram[(addr & (Self::BANK_SIZE - 1) | self.svbk.bank_offset()) as usize]
    }

    #[must_use]
    pub(crate) const fn read_wram_lo(&self, addr: u16) -> u8 {
        self.wram[(addr & (Self::BANK_SIZE - 1)) as usize]
    }

    #[must_use]
    pub(crate) const fn svbk(&self) -> &Svbk {
        &self.svbk
    }

    pub(crate) const fn svbk_mut(&mut self) -> &mut Svbk {
        &mut self.svbk
    }

    #[must_use]
    pub(crate) const fn wram(&self) -> &[u8; Self::SIZE_CGB as usize] {
        &self.wram
    }

    pub(crate) const fn wram_mut(&mut self) -> &mut [u8; Self::SIZE_CGB as usize] {
        &mut self.wram
    }

    pub(crate) fn write_wram_hi(&mut self, addr: u16, val: u8) {
        self.wram[(addr & (Self::BANK_SIZE - 1) | self.svbk.bank_offset()) as usize] = val;
    }

    pub(crate) fn write_wram_lo(&mut self, addr: u16, val: u8) {
        self.wram[(addr & (Self::BANK_SIZE - 1)) as usize] = val;
    }
}
