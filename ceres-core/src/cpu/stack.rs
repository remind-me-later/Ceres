use super::{registers::Register16::SP, Cpu};
use crate::AudioCallbacks;

impl<'a, AR: AudioCallbacks> Cpu<AR> {
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
}
