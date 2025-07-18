use crate::{AudioCallback, Gb};
use core::time::Duration;

pub const FRAME_DURATION: Duration = Duration::new(0, 16_742_706);
pub const TC_PER_FRAME: i32 = 70224; // t-cycles per frame

// t-cycles per second
pub const TC_SEC: i32 = 0x40_0000; // 2^22

#[derive(Default, Debug)]
pub struct Clock {
    tima: u8,
    tma: u8,
    tac: u8,
    div: u16,
    tima_state: TIMAState,
}

impl Clock {
    pub const fn tima(&self) -> u8 {
        self.tima
    }

    pub const fn tma(&self) -> u8 {
        self.tma
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub enum TIMAState {
    Reloading,
    Reloaded,
    #[default]
    Running,
}

impl<A: AudioCallback> Gb<A> {
    pub fn advance_t_cycles(&mut self, mut cycles: i32) {
        // affected by speed boost
        self.run_timers(cycles);
        self.dma.advance_t_cycles(cycles);

        // not affected by speed boost
        if self.key1.is_enabled() {
            cycles >>= 1;
        }

        // TODO: is this order right?
        self.ppu.run(cycles, &mut self.ints, self.cgb_mode);
        self.run_dma();

        self.apu.run(cycles);
        self.cart.run_rtc(cycles);

        self.t_cycles_run += cycles;
    }

    const fn advance_tima_state(&mut self) {
        match self.clock.tima_state {
            TIMAState::Reloading => {
                self.ints.request_timer();
                self.clock.tima_state = TIMAState::Reloaded;
            }
            TIMAState::Reloaded => {
                self.clock.tima_state = TIMAState::Running;
            }
            TIMAState::Running => (),
        }
    }

    const fn inc_tima(&mut self) {
        self.clock.tima = self.clock.tima.wrapping_add(1);

        if self.clock.tima == 0 {
            self.clock.tima = self.clock.tma;
            self.clock.tima_state = TIMAState::Reloading;
        }
    }

    // only modify div inside this function
    // TODO: this could be optimized
    fn set_system_clk(&mut self, val: u16) {
        #[must_use]
        const fn sys_clk_tac_mux(tac: u8) -> u16 {
            match tac & 3 {
                0 => 1 << 9,
                1 => 1 << 3,
                2 => 1 << 5,
                _ => 1 << 7,
            }
        }

        let triggers = self.clock.div & !val;
        let apu_bit = if self.key1.is_enabled() {
            0x2000
        } else {
            0x1000
        };

        // increase TIMA on falling edge of TAC mux
        if self.is_tac_enabled() && (triggers & sys_clk_tac_mux(self.clock.tac) != 0) {
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

        self.clock.div = val;
    }

    pub fn run_timers(&mut self, cycles: i32) {
        for _ in 0..cycles / 4 {
            self.advance_tima_state();
            self.set_system_clk(self.clock.div.wrapping_add(4));
        }
    }

    #[must_use]
    pub const fn read_div(&self) -> u8 {
        ((self.clock.div >> 8) & 0xFF) as u8
    }

    pub fn write_div(&mut self) {
        self.set_system_clk(0);
    }

    pub const fn write_tima(&mut self, val: u8) {
        self.clock.tima = val;
    }

    pub const fn write_tma(&mut self, val: u8) {
        self.clock.tma = val;
    }

    #[must_use]
    pub const fn read_tac(&self) -> u8 {
        0xF8 | self.clock.tac
    }

    pub const fn write_tac(&mut self, val: u8) {
        self.clock.tac = val;
    }

    #[must_use]
    const fn is_tac_enabled(&self) -> bool {
        self.clock.tac & 4 != 0
    }
}
