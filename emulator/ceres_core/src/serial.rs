use crate::{Gb, IF_SERIAL_B};

const T_START: u8 = 0x80;
// const CLOCK_S: u8 = 0x2;
// const SH_CLK: u8 = 0x1;

impl Gb {
    pub(crate) fn run_serial(&mut self, t_cycles: i32) {
        if self.sc & T_START == 0 {
            return;
        }

        // TODO: always off
        // TODO: timing
        for _ in 0..t_cycles {
            self.sb = self.sb << 1 | 1;
            self.serial_bit += 1;

            if self.serial_bit > 7 {
                self.serial_bit = 0;
                self.ifr |= IF_SERIAL_B;
                self.sc &= !T_START;
            }
        }
    }
}
