use crate::{memory::HdmaState, ppu::Mode, Gb, IF_TIMER_B};

impl Gb {
    pub(crate) fn advance_cycles(&mut self, mut cycles: u32) {
        for _ in 0..cycles {
            // affected by speed boost
            self.run_dma();
            self.run_timer();
        }

        // not affected by speed boost
        if self.double_speed {
            cycles <<= 1;
        } else {
            cycles <<= 2;
        }

        self.run_ppu(cycles);
        self.run_apu(cycles);
    }

    fn run_dma(&mut self) {
        if !self.dma_on {
            return;
        }

        self.dma_cycles += 4;

        if self.dma_cycles < 4 {
            return;
        }

        self.dma_cycles -= 4;

        // TODO: reading some ranges should cause problems, $DF is
        // the maximum value accesible to OAM DMA (probably reads
        // from echo RAM should work too, RESEARCH).
        // what happens if reading from IO range? (garbage? 0xff?)
        let val = self.read_mem(self.dma_addr);

        // TODO: writes from DMA can access OAM on modes 2 and 3
        // with some glitches (RESEARCH) and without trouble during
        // VBLANK (what happens in HBLANK?)
        self.oam[(self.dma_addr & 0xFF) as usize] = val;

        self.dma_addr = self.dma_addr.wrapping_add(1);
        if self.dma_addr & 0xFF >= 0xA0 {
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
            self.hdma_len -= 0x10;
            self.hdma_state = if self.hdma_len == 0 {
                HdmaState::Sleep
            } else {
                HdmaState::HBlankDone
            };
            self.hdma5 = ((self.hdma_len / 0x10).wrapping_sub(1) & 0xFF) as u8;
            0x10
        } else {
            self.hdma_state = HdmaState::Sleep;
            self.hdma5 = 0xFF;
            let len = self.hdma_len;
            self.hdma_len = 0;
            len
        };

        for _ in 0..len {
            // TODO: the same problems as normal DMA plus reading from
            // VRAM should copy garbage
            let val = self.read_mem(self.hdma_src);
            self.write_vram(self.hdma_dst, val);
            self.hdma_dst += 1;
            self.hdma_src += 1;
        }

        // can be outside of loop because HDMA should not
        // access IO range (clk registers, ifr,
        // etc..). If the PPU reads VRAM during an HDMA transfer it
        // should be glitchy anyways

        if self.double_speed {
            self.advance_cycles(u32::from(len) * 2);
        } else {
            self.advance_cycles(u32::from(len));
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

    pub(crate) fn run_timer(&mut self) {
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
