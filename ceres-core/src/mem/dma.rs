use crate::{ppu::Mode, Gb};

#[derive(PartialEq, Eq)]
pub enum HdmaState {
    Sleep,
    HBlank,
    HBlankDone,
    General,
}

impl Gb {
    pub(crate) fn write_dma(&mut self, val: u8) {
        if self.dma_on {
            self.dma_restarting = true;
        }

        self.dma_cycles = -8; // two m-cycles delay
        self.dma = val;
        self.dma_addr = u16::from(val) << 8;
        self.dma_on = true;
    }

    // FIXME: sprites are not displayed during OAM DMA
    pub(crate) fn emulate_dma(&mut self) {
        self.dma_cycles += 4;

        if !self.dma_on || self.dma_cycles < 4 {
            return;
        }

        self.dma_cycles -= 4;
        let src = self.dma_addr;

        let val = match src >> 8 {
            0x00..=0x7f => self.cart.read_rom(src),
            0x80..=0x9f => self.read_vram(src),
            0xa0..=0xbf => self.cart.read_ram(src),
            0xc0..=0xcf | 0xe0..=0xef => self.read_ram(src),
            0xd0..=0xdf | 0xf0..=0xff => self.read_bank_ram(src),
            _ => panic!("Illegal source addr for OAM DMA transfer"),
        };

        // mode doesn't matter writes from DMA can always access OAM
        self.oam[((src & 0xff) as u8) as usize] = val;

        self.dma_addr = self.dma_addr.wrapping_add(1);
        if self.dma_addr & 0xff >= 0xa0 {
            self.dma_on = false;
            self.dma_restarting = false;
        }
    }

    pub(crate) fn dma_active(&self) -> bool {
        self.dma_on && (self.dma_cycles > 0 || self.dma_restarting)
    }

    pub(crate) fn write_hdma1(&mut self, val: u8) {
        self.hdma_src = u16::from(val) << 8 | self.hdma_src & 0xf0;
    }

    pub(crate) fn write_hdma2(&mut self, val: u8) {
        self.hdma_src = self.hdma_src & 0xff00 | u16::from(val) & 0xf0;
    }

    pub(crate) fn write_hdma3(&mut self, val: u8) {
        self.hdma_dst = u16::from(val & 0x1f) << 8 | self.hdma_dst & 0xf0;
    }

    pub(crate) fn write_hdma4(&mut self, val: u8) {
        self.hdma_dst = self.hdma_dst & 0x1f00 | u16::from(val) & 0xf0;
    }

    fn hdma_on(&self) -> bool {
        !matches!(self.hdma_state, HdmaState::Sleep)
    }

    pub(crate) fn read_hdma5(&self) -> u8 {
        // active on low
        ((!self.hdma_on()) as u8) << 7 | self.hdma5
    }

    pub(crate) fn write_hdma5(&mut self, val: u8) {
        // stop current transfer
        if self.hdma_on() && val & 0x80 == 0 {
            self.hdma_state = HdmaState::Sleep;
            return;
        }

        self.hdma5 = val & !0x80;
        let transfer_blocks = val & 0x7f;
        self.hdma_len = (u16::from(transfer_blocks) + 1) * 0x10;
        self.hdma_state = if val & 0x80 == 0 {
            HdmaState::General
        } else {
            HdmaState::HBlank
        };
    }

    pub(crate) fn emulate_hdma(&mut self) {
        match self.hdma_state {
            HdmaState::General => (),
            HdmaState::HBlank if self.ppu_mode() == Mode::HBlank => (),
            HdmaState::HBlankDone if self.ppu_mode() != Mode::HBlank => {
                self.hdma_state = HdmaState::HBlank;
                return;
            }
            _ => return,
        }

        let len = if self.hdma_state == HdmaState::HBlank {
            self.hdma_state = HdmaState::HBlankDone;
            self.hdma5 = (self.hdma_len / 0x10).wrapping_sub(1) as u8;
            self.hdma_len -= 0x10;
            0x10
        } else {
            self.hdma_state = HdmaState::Sleep;
            self.hdma5 = 0xff;
            let len = self.hdma_len;
            self.hdma_len = 0;
            len
        };

        for _ in 0..len {
            let val = match self.hdma_src >> 8 {
                0x00..=0x7f => self.cart.read_rom(self.hdma_src),
                // TODO: should copy garbage
                0x80..=0x9f => 0xff,
                0xa0..=0xbf => self.cart.read_ram(self.hdma_src),
                0xc0..=0xcf => self.read_ram(self.hdma_src),
                0xd0..=0xdf => self.read_bank_ram(self.hdma_src),
                _ => panic!("Illegal source addr for HDMA transfer"),
            };

            self.emulate_dma();
            self.tick_ppu();
            self.tick_timer();
            self.tick_apu();

            self.write_vram(self.hdma_dst, val);

            self.hdma_dst += 1;
            self.hdma_src += 1;
        }

        if self.hdma_len == 0 {
            self.hdma_state = HdmaState::Sleep;
        }
    }
}
