mod execute;
mod instructions;
mod interrupt_service;
mod operands;
mod registers;
mod stack;

use crate::{memory::Memory, AudioCallbacks, Cartridge};
use registers::{Register16::PC, Registers};

pub struct Cpu<AR: AudioCallbacks> {
    registers: Registers,
    memory: Memory<AR>,
    ei_delay: bool,
    ime: bool,
    halted: bool,
    halt_bug: bool,
}

impl<AR: AudioCallbacks> Cpu<AR> {
    pub fn new(memory: Memory<AR>) -> Self {
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

    pub fn memory(&self) -> &Memory<AR> {
        &self.memory
    }

    pub fn mut_memory(&mut self) -> &mut Memory<AR> {
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

        self.service_interrupt();
    }

    fn get_immediate_for_print(&mut self) -> u8 {
        let address = self.registers.read16(PC);
        self.memory.read(address)
    }

    fn get_immediate_for_print_16(&mut self) -> u16 {
        let address = self.registers.read16(PC);
        let lo = self.memory.read(address);
        let hi = self.memory.read(address.wrapping_add(1));
        u16::from_le_bytes([lo, hi])
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

    // ********************
    // * Arithmetic/Logic *
    // ********************
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
