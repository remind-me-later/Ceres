use crate::{memory::HdmaState, ppu::Mode, Gb, IF_TIMER_B};

impl Gb {
    pub(crate) fn advance_cycles(&mut self, cycles: u32) {
        for _ in 0..cycles {
            // affeected by speed boost
            self.run_dma();
            self.tick_timer();

            // not affected by speed boost
            let mut cycles = 4;

            if self.double_speed {
                cycles >>= 1;
            }

            self.tick_apu(cycles);
            self.tick_ppu(cycles);
        }
    }

    // FIXME: sprites are not displayed during OAM DMA
    fn run_dma(&mut self) {
        if !self.dma_on {
            return;
        }

        self.dma_cycles += 4;

        if self.dma_cycles < 4 {
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

    pub(crate) fn run_hdma(&mut self) {
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

            self.advance_cycles(1);

            self.write_vram(self.hdma_dst, val);

            self.hdma_dst += 1;
            self.hdma_src += 1;
        }

        if self.hdma_len == 0 {
            self.hdma_state = HdmaState::Sleep;
        }
    }

    fn counter_bit(&self) -> bool {
        let mask = {
            match self.tac & 3 {
                3 => 1 << 5,
                2 => 1 << 3,
                1 => 1 << 1,
                0 => 1 << 7,
                _ => unreachable!(),
            }
        };

        self.clk_wide & mask != 0
    }

    fn inc_timer(&mut self) {
        let (counter, overflow) = self.tima.overflowing_add(1);
        self.tima = counter;
        self.clk_overflow = overflow;
    }

    pub(crate) fn tick_timer(&mut self) {
        if self.clk_overflow {
            self.clk_wide = self.clk_wide.wrapping_add(1);
            self.tima = self.tma;
            self.ifr |= IF_TIMER_B;
            self.clk_overflow = false;
        } else if self.clk_on && self.counter_bit() {
            self.clk_wide = self.clk_wide.wrapping_add(1);
            let new_bit = self.counter_bit();
            if !new_bit {
                self.inc_timer();
            }
        } else {
            self.clk_wide = self.clk_wide.wrapping_add(1);
        }
    }

    pub(crate) fn write_div(&mut self) {
        if self.counter_bit() {
            self.inc_timer();
        }

        self.clk_wide = 0;
    }

    pub(crate) fn write_tima(&mut self, val: u8) {
        let overflow = self.clk_overflow;

        if !overflow {
            self.clk_overflow = false;
            self.tima = val;
        }
    }

    pub(crate) fn write_tma(&mut self, val: u8) {
        let overflow = self.clk_overflow;

        self.tma = val;

        if overflow {
            self.tima = val;
        }
    }

    pub(crate) fn write_tac(&mut self, val: u8) {
        let old_bit = self.clk_on && self.counter_bit();
        self.tac = val & 7;
        self.clk_on = val & 4 != 0;
        let new_bit = self.clk_on && self.counter_bit();

        if old_bit && !new_bit {
            self.inc_timer();
        }
    }
}
