use crate::{memory::HdmaState, ppu::Mode, Gb, IF_TIMER_B};

impl Gb {
    pub(crate) fn advance_cycles(&mut self, mut cycles: u32) {
        // affected by speed boost
        self.run_timers(cycles);
        self.dma_cycles += cycles as i32;

        // not affected by speed boost
        if self.double_speed {
            cycles >>= 1;
        }

        self.run_ppu(cycles);
        self.run_dma();
        self.run_apu(cycles);
    }

    fn run_dma(&mut self) {
        if !self.dma_on {
            return;
        }

        while self.dma_cycles >= 4 {
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

    fn tac_mux(&self) -> bool {
        let mask = {
            match self.tac & 3 {
                0 => 1 << 9,
                3 => 1 << 7,
                2 => 1 << 5,
                1 => 1 << 3,
                _ => unreachable!(),
            }
        };

        self.system_clk & mask != 0
    }

    fn inc_tima(&mut self) {
        let (tima, tima_overflow) = self.tima.overflowing_add(1);
        self.tima = tima;
        self.tima_overflow = tima_overflow;
    }

    pub(crate) fn run_timers(&mut self, cycles: u32) {
        for _ in 0..cycles {
            let old_bit = self.tac_mux();

            self.system_clk = self.system_clk.wrapping_add(1);

            if self.tima_overflow {
                self.do_tima_overflow();
            } else {
                let new_bit = self.tac_mux();

                // increase TIMA on falling edge of TAC mux
                if self.tac_enable && old_bit && !new_bit {
                    self.inc_tima();
                }
            }
        }
    }

    fn do_tima_overflow(&mut self) {
        self.tima = self.tma;
        self.ifr |= IF_TIMER_B;
        self.tima_overflow = false;
    }

    pub(crate) fn write_div(&mut self) {
        if self.tac_mux() {
            self.inc_tima();
        }

        self.system_clk = 0;
    }

    pub(crate) fn write_tima(&mut self, val: u8) {
        if !self.tima_overflow {
            self.tima_overflow = false;
            self.tima = val;
        }
    }

    pub(crate) fn write_tma(&mut self, val: u8) {
        if self.tima_overflow {
            self.do_tima_overflow();
        }

        self.tma = val;
    }

    pub(crate) fn write_tac(&mut self, val: u8) {
        let old_bit = self.tac_enable && self.tac_mux();
        self.tac = val & 7;
        self.tac_enable = val & 4 != 0;
        let new_bit = self.tac_enable && self.tac_mux();

        if old_bit && !new_bit {
            self.inc_tima();
        }
    }
}
