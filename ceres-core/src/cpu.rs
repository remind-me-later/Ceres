mod execute;
mod instructions;
mod operands;
mod registers;

use crate::memory::Memory;
use registers::Registers;

pub struct Cpu {
    registers: Registers,
    pub mem: Memory,
    ei_delay: bool,
    ime: bool,
    halted: bool,
    halt_bug: bool,
}

impl Cpu {
    pub fn new(memory: Memory) -> Self {
        Self {
            registers: Registers::new(),
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
                self.registers.decrease_pc();
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

            let pc = self.registers.pc;
            self.internal_push(pc);

            let interrupt = self.mem.interrupt_controller().requested_interrupt();
            self.registers.pc = interrupt.handler_address();

            self.mem.tick_t_cycle();

            self.mem.mut_interrupt_controller().acknowledge(interrupt);
        }
    }

    fn imm8(&mut self) -> u8 {
        let address = self.registers.pc;
        self.registers.increase_pc();
        self.mem.read(address)
    }

    fn imm16(&mut self) -> u16 {
        let lo = self.imm8();
        let hi = self.imm8();
        u16::from_le_bytes([lo, hi])
    }

    fn internal_pop(&mut self) -> u16 {
        let lo = self.mem.read(self.registers.sp);
        self.registers.increase_sp();
        let hi = self.mem.read(self.registers.sp);
        self.registers.increase_sp();
        u16::from_le_bytes([lo, hi])
    }

    fn internal_push(&mut self, value: u16) {
        let [lo, hi] = u16::to_le_bytes(value);
        self.mem.tick_t_cycle();
        self.registers.decrease_sp();
        self.mem.write(self.registers.sp, hi);
        self.registers.decrease_sp();
        self.mem.write(self.registers.sp, lo);
    }

    fn internal_rl(&mut self, val: u8) -> u8 {
        let ci = u8::from(self.registers.cf());
        let co = val & 0x80;
        let res = (val << 1) | ci;
        self.registers.set_zf(res == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(co != 0);
        res
    }

    fn internal_rlc(&mut self, val: u8) -> u8 {
        let co = val & 0x80;
        let res = val.rotate_left(1);
        self.registers.set_zf(res == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(co != 0);
        res
    }

    fn internal_rr(&mut self, val: u8) -> u8 {
        let ci = u8::from(self.registers.cf());
        let co = val & 0x01;
        let res = (val >> 1) | (ci << 7);
        self.registers.set_zf(res == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(co != 0);
        res
    }

    fn internal_rrc(&mut self, value: u8) -> u8 {
        let co = value & 0x01;
        let res = value.rotate_right(1);
        self.registers.set_zf(res == 0);
        self.registers.set_nf(false);
        self.registers.set_hf(false);
        self.registers.set_cf(co != 0);
        res
    }
}
