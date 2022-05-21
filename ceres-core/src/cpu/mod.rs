use crate::{Gb, IF_LCD_B, IF_P1_B, IF_SERIAL_B, IF_TIMER_B, IF_VBLANK_B};

mod execute;
mod operands;
mod registers;

const ZF_B: u16 = 0x80;
const NF_B: u16 = 0x40;
const HF_B: u16 = 0x20;
const CF_B: u16 = 0x10;

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
                self.pc = self.pc.wrapping_sub(1);
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
        self.pc = self.pc.wrapping_add(1);
        self.read_mem(addr)
    }

    fn imm16(&mut self) -> u16 {
        let lo = self.imm8();
        let hi = self.imm8();
        u16::from_le_bytes([lo, hi])
    }

    fn internal_pop(&mut self) -> u16 {
        let lo = self.read_mem(self.sp);
        self.sp = self.sp.wrapping_add(1);
        let hi = self.read_mem(self.sp);
        self.sp = self.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    fn internal_push(&mut self, val: u16) {
        let [lo, hi] = u16::to_le_bytes(val);
        self.tick();
        self.sp = self.sp.wrapping_sub(1);
        self.write_mem(self.sp, hi);
        self.sp = self.sp.wrapping_sub(1);
        self.write_mem(self.sp, lo);
    }
}
