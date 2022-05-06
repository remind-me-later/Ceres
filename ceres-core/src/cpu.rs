use crate::interrupts::{JOYPAD_INT, LCD_STAT_INT, SERIAL_INT, TIMER_INT, VBLANK_INT};

mod execute;
mod instructions;
mod operands;
mod registers;

use {crate::memory::Memory, registers::Regs};

pub struct Cpu {
    reg: Regs,
    pub mem: Memory,
    ei_delay: bool,
    ime: bool,
    halted: bool,
    halt_bug: bool,
}

impl Cpu {
    pub fn new(memory: Memory) -> Self {
        Self {
            reg: Regs::new(),
            ei_delay: false,
            ime: false,
            halted: false,
            halt_bug: false,
            mem: memory,
        }
    }

    pub fn run(&mut self) {
        if self.ei_delay {
            self.ime = true;
            self.ei_delay = false;
        }

        if self.halted {
            self.mem.tick_t_cycle();
        } else {
            let opcode = self.imm8();

            if self.halt_bug {
                self.reg.dec_pc();
                self.halt_bug = false;
            }

            self.exec(opcode);
        }

        if !self.mem.interrupt_controller().has_pending_interrupts() {
            return;
        }

        // handle interrupt
        if self.halted {
            self.halted = false;
        }

        if self.ime {
            self.ime = false;

            self.mem.tick_t_cycle();

            let pc = self.reg.pc;
            self.internal_push(pc);

            let interrupt = self.mem.interrupt_controller().requested_interrupt();
            self.reg.pc = match interrupt {
                VBLANK_INT => 0x40,
                LCD_STAT_INT => 0x48,
                TIMER_INT => 0x50,
                SERIAL_INT => 0x58,
                JOYPAD_INT => 0x60,
                _ => unreachable!(),
            };

            self.mem.tick_t_cycle();

            self.mem.mut_interrupt_controller().acknowledge(interrupt);
        }
    }

    fn imm8(&mut self) -> u8 {
        let addr = self.reg.pc;
        self.reg.inc_pc();
        self.mem.read(addr)
    }

    fn imm16(&mut self) -> u16 {
        let lo = self.imm8();
        let hi = self.imm8();
        u16::from_le_bytes([lo, hi])
    }

    fn internal_pop(&mut self) -> u16 {
        let lo = self.mem.read(self.reg.sp);
        self.reg.inc_sp();
        let hi = self.mem.read(self.reg.sp);
        self.reg.inc_sp();
        u16::from_le_bytes([lo, hi])
    }

    fn internal_push(&mut self, value: u16) {
        let [lo, hi] = u16::to_le_bytes(value);
        self.mem.tick_t_cycle();
        self.reg.dec_sp();
        self.mem.write(self.reg.sp, hi);
        self.reg.dec_sp();
        self.mem.write(self.reg.sp, lo);
    }

    fn internal_rl(&mut self, val: u8) -> u8 {
        let ci = u8::from(self.reg.cf());
        let co = val & 0x80;
        let res = (val << 1) | ci;
        self.reg.set_zf(res == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(co != 0);
        res
    }

    fn internal_rlc(&mut self, val: u8) -> u8 {
        let co = val & 0x80;
        let res = val.rotate_left(1);
        self.reg.set_zf(res == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(co != 0);
        res
    }

    fn internal_rr(&mut self, val: u8) -> u8 {
        let ci = u8::from(self.reg.cf());
        let co = val & 0x01;
        let res = (val >> 1) | (ci << 7);
        self.reg.set_zf(res == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(co != 0);
        res
    }

    fn internal_rrc(&mut self, value: u8) -> u8 {
        let co = value & 0x01;
        let res = value.rotate_right(1);
        self.reg.set_zf(res == 0);
        self.reg.set_nf(false);
        self.reg.set_hf(false);
        self.reg.set_cf(co != 0);
        res
    }
}
