pub use registers::Regs;

use crate::{Gb, IF_LCD_B, IF_P1_B, IF_SERIAL_B, IF_TIMER_B, IF_VBLANK_B};

mod execute;
mod instructions;
mod operands;
mod registers;

impl Gb {
    pub(crate) fn run(&mut self) {
        if self.cpu_ei_delay {
            self.ime = true;
            self.cpu_ei_delay = false;
        }

        if self.cpu_halted {
            self.tick();
        } else {
            let opcode = self.imm8();

            if self.cpu_halt_bug {
                self.dec_pc();
                self.cpu_halt_bug = false;
            }

            self.exec(opcode);
        }

        if !self.any_interrrupt() {
            return;
        }

        // handle interrupt
        if self.cpu_halted {
            self.cpu_halted = false;
        }

        if self.ime {
            self.ime = false;

            self.tick();

            let pc = self.pc;
            self.internal_push(pc);

            let interrupt = {
                let pending = self.ifr & self.ie & 0x1f;

                if pending == 0 {
                    0
                } else {
                    // get rightmost interrupt
                    1 << pending.trailing_zeros()
                }
            };

            self.pc = match interrupt {
                IF_VBLANK_B => 0x40,
                IF_LCD_B => 0x48,
                IF_TIMER_B => 0x50,
                IF_SERIAL_B => 0x58,
                IF_P1_B => 0x60,
                _ => unreachable!(),
            };

            self.tick();
            // acknoledge
            self.ifr &= !interrupt;
        }
    }

    #[must_use]
    fn any_interrrupt(&self) -> bool {
        self.ifr & self.ie & 0x1f != 0
    }

    fn imm8(&mut self) -> u8 {
        let addr = self.pc;
        self.inc_pc();
        self.read_mem(addr)
    }

    fn imm16(&mut self) -> u16 {
        let lo = self.imm8();
        let hi = self.imm8();
        u16::from_le_bytes([lo, hi])
    }

    fn internal_pop(&mut self) -> u16 {
        let lo = self.read_mem(self.sp);
        self.inc_sp();
        let hi = self.read_mem(self.sp);
        self.inc_sp();
        u16::from_le_bytes([lo, hi])
    }

    fn internal_push(&mut self, val: u16) {
        let [lo, hi] = u16::to_le_bytes(val);
        self.tick();
        self.dec_sp();
        self.write_mem(self.sp, hi);
        self.dec_sp();
        self.write_mem(self.sp, lo);
    }

    fn internal_rl(&mut self, val: u8) -> u8 {
        let ci = u8::from(self.cf());
        let co = val & 0x80;
        let res = (val << 1) | ci;
        self.set_zf(res == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(co != 0);
        res
    }

    fn internal_rlc(&mut self, val: u8) -> u8 {
        let co = val & 0x80;
        let res = val.rotate_left(1);
        self.set_zf(res == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(co != 0);
        res
    }

    fn internal_rr(&mut self, val: u8) -> u8 {
        let ci = u8::from(self.cf());
        let co = val & 0x01;
        let res = (val >> 1) | (ci << 7);
        self.set_zf(res == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(co != 0);
        res
    }

    fn internal_rrc(&mut self, val: u8) -> u8 {
        let co = val & 0x01;
        let res = val.rotate_right(1);
        self.set_zf(res == 0);
        self.set_nf(false);
        self.set_hf(false);
        self.set_cf(co != 0);
        res
    }
}
