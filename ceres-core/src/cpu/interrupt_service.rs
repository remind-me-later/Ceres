use super::registers::Register16::PC;
use super::Cpu;
use crate::AudioCallbacks;

impl<'a, AR: AudioCallbacks> Cpu<AR> {
    pub fn service_interrupt(&mut self) {
        if self
            .memory
            .interrupt_controller()
            .requested_interrupt()
            .is_empty()
        {
            return;
        }

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
}
