//! The machine side of the CPU's bus: how a CPU step runs, and what each
//! bus access and internal cycle does to the rest of the machine.

mod write;

use crate::sm83::Bus;
use crate::{AudioCallback, Gb, memory::SwitchHdma, ppu::Mode};
use core::mem;

impl<A: AudioCallback> Gb<A> {
    /// Runs exactly one CPU step (one instruction, interrupt dispatch or
    /// HALT M-cycle), then flushes any bus time the step left deferred.
    /// The CPU is invoked via `mem::take` so `self` can be handed to it
    /// wholesale as its `Bus` without overlapping borrows.
    #[inline]
    pub fn run_cpu(&mut self) {
        let mut cpu = mem::take(&mut self.cpu);
        cpu.step(self);
        self.flush_deferred_time();
        self.cpu = cpu;
    }
}

impl<A: AudioCallback> Gb<A> {
    /// Advances the machine by any bus time a CPU step deferred but never
    /// consumed (only the interrupt dispatch's 2-T-cycle tail can survive
    /// across a step boundary).
    #[inline]
    fn flush_deferred_time(&mut self) {
        if self.time_deferred != 0 {
            self.advance_t_cycles(self.time_deferred);
            self.time_deferred = 0;
        }
    }
}

impl<A: AudioCallback> Bus for Gb<A> {
    #[inline]
    fn tick(&mut self) {
        self.time_deferred += 4;
    }

    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        self.flush_deferred_time();
        self.address_bus = addr;
        let val = self.cpu_read_mem(addr);
        self.time_deferred = 4;
        val
    }

    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        self.cpu_write(addr, val);
    }

    #[inline]
    fn defer(&mut self, t_cycles: i32) {
        let flush = self.time_deferred - t_cycles;
        if flush > 0 {
            self.advance_t_cycles(flush);
        }
        self.time_deferred = t_cycles;
    }

    fn interrupts_pending(&self) -> bool {
        self.ints.is_any_requested()
    }

    fn read_if(&self) -> u8 {
        self.ints.read_if()
    }

    fn read_ie(&self) -> u8 {
        self.ints.read_ie()
    }

    fn ack_interrupt(&mut self, bit: u8) {
        // Gambatte runs the LCD two cycles ahead of an acknowledge: the LYC
        // interrupt that the compare around line 153 raises that soon counts
        // as already requested and is swallowed by the acknowledge. (In double
        // speed the compare itself comes early enough.)
        if !self.key1.is_enabled() {
            self.ppu
                .run_ahead(&mut self.ints, self.cgb_mode, false, 2, false);
        }
        // On the CGB a timer interrupt due within the next cycles counts as
        // already requested: acknowledging the timer bit swallows it.
        if self.model.is_cgb_hardware() {
            let headroom = 3 + self.time_deferred;
            if (1..=headroom).contains(&i32::from(self.clock.tima_irq_countdown)) {
                self.clock.tima_irq_countdown = 0;
                self.ints.request_timer();
            }
            // The serial port is looked ahead too.
            self.serial.complete_if_due(
                self.clock.div,
                u16::try_from(3 + self.time_deferred).unwrap_or(0),
                &mut self.ints,
            );
        }
        self.ints.acknowledge_interrupt(bit);
    }

    fn clear_ie(&mut self) {
        self.ints.illegal();
    }

    fn tick_hdma(&mut self) {
        if self.hdma.is_on() {
            if mem::take(&mut self.hdma_halt_prefetch) {
                self.time_deferred -= 4;
            }
            self.run_hdma();
        }
    }

    fn tick_oam_bug(&mut self, addr: u16) {
        self.flush_deferred_time();
        self.address_bus = addr;
        self.ppu.trigger_oam_bug(addr);
        self.time_deferred = 4;
    }

    fn trigger_oam_bug(&mut self, addr: u16) {
        self.ppu.trigger_oam_bug(addr);
    }

    fn dma_run(&mut self, wake: bool) {
        if wake {
            self.dma.set_cycles(4);
        }
        self.run_dma();
    }

    fn dma_finish_before_halt(&mut self) {
        if self.dma.is_in_last_step() {
            self.dma_run(true);
        }
    }

    fn drop_deferred(&mut self) {
        self.time_deferred = 0;
    }

    fn set_halted(&mut self, halted: bool) {
        let hblank = self.ppu.hdma_period();
        self.hdma.set_cpu_halted(halted, hblank);
        self.ppu
            .set_cpu_idle(self.hdma.cpu_halted() || self.clock.stopped);
    }

    fn advance(&mut self, t_cycles: i32) {
        self.advance_t_cycles(t_cycles);
    }

    fn is_cgb_hardware(&self) -> bool {
        self.model.is_cgb_hardware()
    }

    fn wake_from_stop(&mut self) {
        self.leave_stop();
        self.speed_switch.halt_countdown = 0;
    }

    fn leave_stop(&mut self) {
        self.ppu.leave_stop_mode();
        self.clock.stopped = false;
        let hblank = self.ppu.hdma_period();
        self.hdma.set_cpu_halted(false, hblank);
        // A speed switch does not request the HBlank transfer (gambatte).
        let switching = self.ppu.gambatte_stat() && self.speed_switch.halt_countdown != 0;
        if !switching {
            self.hdma.wake(hblank);
        }
        self.ppu.set_cpu_idle(false);
    }

    fn flush(&mut self) {
        self.flush_deferred_time();
    }

    fn peek(&self, addr: u16) -> u8 {
        self.read_mem(addr)
    }

    fn is_stopped(&self) -> bool {
        self.clock.stopped
    }

    fn clear_speed_switch_halt(&mut self) {
        self.speed_switch.halt_countdown = 0;
    }

    fn note_halt_prefetch(&mut self) {
        self.hdma_halt_prefetch = true;
    }

    fn hdma_request_pending(&self) -> bool {
        self.ppu.gambatte_stat() && self.hdma.hblank_requested()
    }

    fn take_unhalt(&mut self) -> bool {
        let unhalt = mem::take(&mut self.speed_switch.unhalt);
        if unhalt && self.ppu.gambatte_stat() {
            // The wake of a speed switch requests the HBlank transfer
            // (gambatte's `intevent_unhalt`).
            let period =
                self.hdma.hblank_enabled() && self.ppu.gstat_hdma_period(0).unwrap_or(false);
            if (period && self.hdma.switch_state() == SwitchHdma::Low)
                || self.hdma.switch_state() == SwitchHdma::Requested
            {
                self.hdma.request_hblank();
            }
            if self.hdma.switch_state() == SwitchHdma::Requested {
                // The transfer dropped at the switch runs in the time of the
                // opcode prefetched then.
                self.hdma_halt_prefetch = true;
            }
        }
        unhalt
    }

    fn enter_stop(&mut self, ime: bool) {
        if self.ppu.gambatte_stat() && self.key1.is_requested() {
            let period =
                self.hdma.hblank_enabled() && self.ppu.gstat_hdma_period(0).unwrap_or(false);
            self.hdma
                .set_switch_state(if self.hdma.hblank_requested() && self.key1.is_enabled() {
                    SwitchHdma::Requested
                } else if period {
                    SwitchHdma::High
                } else {
                    SwitchHdma::Low
                });
        }
        self.tima_speed_change_catch_up();
        self.write_div();
        if !ime {
            self.clock.div_cycles = -4;
        }
        self.ppu.enter_stop_mode();
        self.clock.stopped = true;
        self.ppu.set_cpu_idle(true);
        self.hdma.note_stop(matches!(self.ppu.mode(), Mode::HBlank));
    }

    fn speed_switch_requested(&self) -> bool {
        self.key1.is_requested()
    }

    fn begin_speed_switch(&mut self, interrupt_pending: bool) {
        self.flush_deferred_time();
        if self.ppu.gambatte_stat() && self.key1.is_enabled() {
            // gambatte's `Memory::stop`: a pending HBlank transfer survives a
            // switch to double speed (it runs during the halt); leaving double
            // speed drops it and requests it again at the wake.
            self.hdma.ack_hblank_request();
        }
        if self.key1.is_enabled() {
            self.key1.set_double_speed(false);
            self.left_double_speed();
        } else {
            self.speed_switch.countdown = 6;
            self.speed_switch.freeze = 1;
        }
        if !interrupt_pending {
            self.speed_switch.halt_countdown = 0x20008;
            self.speed_switch.freeze = 5;
        }
        self.key1.clear_request();
    }
}
