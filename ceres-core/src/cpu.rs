mod execute;
mod instructions;
mod operands;
mod registers;

use crate::{memory::Memory, Cartridge};
use registers::{Register16::PC, Register16::SP, Registers};

pub struct Cpu {
    registers: Registers,
    memory: Memory,
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
            memory,
        }
    }

    pub fn cartridge(&self) -> &Cartridge {
        self.memory.cartridge()
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn mut_memory(&mut self) -> &mut Memory {
        &mut self.memory
    }

    pub fn execute_instruction(&mut self) {
        if self.ei_delay {
            self.ime = true;
            self.ei_delay = false;
        }

        if self.halted {
            self.memory.tick_t_cycle();
        } else {
            let opcode = self.read_immediate();

            if self.halt_bug {
                self.registers.decrease_pc();
                self.halt_bug = false;
            }

            self.execute(opcode);
        }

        if !self.memory.interrupt_controller().has_pending_interrupts() {
            return;
        }

        // handle interrupt
        if self.halted {
            self.halted = false;
        }

        if self.ime {
            self.ime = false;

            self.memory.tick_t_cycle();

            let pc = self.registers.read16(PC);
            self.internal_push(pc);

            let interrupt = self.memory.interrupt_controller().requested_interrupt();
            self.registers.write16(PC, interrupt.handler_address());

            self.memory.tick_t_cycle();

            self.memory
                .mut_interrupt_controller()
                .acknowledge(interrupt);
        }
    }

    fn read_immediate(&mut self) -> u8 {
        let address = self.registers.read16(PC);
        self.registers.increase_pc();
        self.memory.read(address)
    }

    fn read_immediate16(&mut self) -> u16 {
        let lo = self.read_immediate();
        let hi = self.read_immediate();
        u16::from_le_bytes([lo, hi])
    }

    pub fn internal_pop(&mut self) -> u16 {
        let lo = self.memory.read(self.registers.read16(SP));
        self.registers.increase_sp();
        let hi = self.memory.read(self.registers.read16(SP));
        self.registers.increase_sp();
        u16::from_le_bytes([lo, hi])
    }

    pub fn internal_push(&mut self, value: u16) {
        let [lo, hi] = u16::to_le_bytes(value);
        self.memory.tick_t_cycle();
        self.registers.decrease_sp();
        self.memory.write(self.registers.read16(SP), hi);
        self.registers.decrease_sp();
        self.memory.write(self.registers.read16(SP), lo);
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
