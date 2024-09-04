use crate::{AudioCallback, Gb};

#[derive(Clone, Copy, Default, Debug)]
pub enum TIMAState {
    Reloading,
    Reloaded,
    #[default]
    Running,
}

impl<A: AudioCallback> Gb<A> {
    pub(crate) fn advance_t_cycles(&mut self, mut cycles: i32) {
        // affected by speed boost
        self.run_timers(cycles);
        self.dma_cycles += cycles;

        // not affected by speed boost
        if self.key1.enabled() {
            cycles >>= 1;
        }

        // TODO: is this order right?
        self.ppu.run(cycles, &mut self.ints, &self.cgb_mode);
        self.run_dma();

        self.apu.run(cycles);
        self.cart.run_rtc(cycles);

        self.dot_accumulator += cycles;
    }

    #[inline]
    fn advance_tima_state(&mut self) {
        match self.tima_state {
            TIMAState::Reloading => {
                self.ints.req_timer();
                self.tima_state = TIMAState::Reloaded;
            }
            TIMAState::Reloaded => {
                self.tima_state = TIMAState::Running;
            }
            TIMAState::Running => (),
        }
    }

    #[inline]
    fn inc_tima(&mut self) {
        self.tima = self.tima.wrapping_add(1);

        if self.tima == 0 {
            self.tima = self.tma;
            self.tima_state = TIMAState::Reloading;
        }
    }

    // only modify div inside this function
    // TODO: this could be optimized
    fn set_system_clk(&mut self, val: u16) {
        #[must_use]
        #[inline]
        const fn sys_clk_tac_mux(tac: u8) -> u16 {
            match tac & 3 {
                0 => 1 << 9,
                1 => 1 << 3,
                2 => 1 << 5,
                _ => 1 << 7,
            }
        }

        let triggers = self.div & !val;
        let apu_bit = if self.key1.enabled() { 0x2000 } else { 0x1000 };

        // increase TIMA on falling edge of TAC mux
        if self.tac_enabled() && (triggers & sys_clk_tac_mux(self.tac) != 0) {
            self.inc_tima();
        }

        // advance serial master clock
        if triggers & u16::from(self.serial.div_mask()) != 0 {
            self.serial.run_master(&mut self.ints);
        }

        // advance APU on falling edge of APU_DIV bit
        if triggers & apu_bit != 0 {
            self.apu.step_div_apu();
        }

        self.div = val;
    }

    #[inline]
    pub(crate) fn run_timers(&mut self, cycles: i32) {
        for _ in 0..cycles / 4 {
            self.advance_tima_state();
            self.set_system_clk(self.div.wrapping_add(4));
        }
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_div(&self) -> u8 {
        ((self.div >> 8) & 0xFF) as u8
    }

    #[inline]
    pub(crate) fn write_div(&mut self) {
        self.set_system_clk(0);
    }

    #[inline]
    pub(crate) fn write_tima(&mut self, val: u8) {
        self.tima = val;
    }

    #[inline]
    pub(crate) fn write_tma(&mut self, val: u8) {
        self.tma = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_tac(&self) -> u8 {
        0xF8 | self.tac
    }

    #[inline]
    pub(crate) fn write_tac(&mut self, val: u8) {
        self.tac = val;
    }

    #[must_use]
    #[inline]
    const fn tac_enabled(&self) -> bool {
        self.tac & 4 != 0
    }
}
