//! The volume envelope of the square and noise channels (NRx2).

use super::revision::Revision;

/// The clock line of the envelope. Ticking it to the volume limit locks it
/// until the channel is restarted.
#[derive(Clone, Copy)]
pub struct EnvelopeClock {
    pub locked: bool,
    pub clock: bool,
    pub should_lock: bool,
}

impl EnvelopeClock {
    pub const fn set(&mut self, value: bool, direction: bool, volume: u8) {
        if self.clock == value {
            return;
        }
        if value {
            self.clock = true;
            self.should_lock = (volume == 0xF && direction) || (volume == 0x0 && !direction);
        } else {
            self.clock = false;
            self.locked |= self.should_lock;
        }
    }
}

#[derive(Clone, Copy)]
pub struct Envelope {
    /// NRx2: initial volume (bits 4-7), direction (bit 3, 1 = up) and pace.
    pub nrx2: u8,
    pub volume: u8,
    pub countdown: u8,
    pub clock: EnvelopeClock,
}

impl Envelope {
    pub const fn new() -> Self {
        Self {
            nrx2: 0,
            volume: 0,
            countdown: 0,
            clock: EnvelopeClock {
                locked: false,
                clock: false,
                should_lock: false,
            },
        }
    }

    /// The upper 5 bits of NRx2 power the channel's DAC.
    pub const fn dac_enabled(self) -> bool {
        self.nrx2 & 0xF8 != 0
    }

    const fn pace(self) -> u8 {
        self.nrx2 & 7
    }

    const fn increases(self) -> bool {
        self.nrx2 & 8 != 0
    }

    /// The channel was triggered.
    pub const fn unlock(&mut self) {
        self.clock.locked = false;
        self.clock.clock = false;
    }

    /// Loads the initial volume and pace on a trigger.
    pub const fn restart(&mut self) {
        self.volume = self.nrx2 >> 4;
        self.countdown = self.pace();
    }

    /// Every 8th DIV event, the countdown of an envelope whose clock is low
    /// advances.
    pub const fn step_countdown(&mut self) {
        if !self.clock.clock {
            self.countdown = self.countdown.wrapping_sub(1) & 7;
        }
    }

    /// The secondary DIV event raises the clock of an envelope whose
    /// countdown expired.
    pub const fn reload(&mut self) {
        if self.countdown == 0 {
            self.countdown = self.pace();
            self.clock
                .set(self.pace() != 0, self.increases(), self.volume);
        }
    }

    /// Steps the volume on a clock edge. Returns the volume before the step,
    /// or `None` if the envelope did not step.
    pub const fn tick(&mut self) -> Option<u8> {
        self.clock.set(false, false, 0);
        if self.clock.locked || self.pace() == 0 {
            return None;
        }
        let old = self.volume;
        if self.increases() {
            self.volume = self.volume.wrapping_add(1);
        } else {
            self.volume = self.volume.wrapping_sub(1);
        }
        Some(old)
    }

    /// Writes NRx2 while the channel plays (the "zombie mode" volume
    /// glitch). Before the CGB-D some of these are non-deterministic.
    pub fn write_while_active(&mut self, rev: Revision, value: u8) {
        let old = self.nrx2;
        if rev <= Revision::CgbC {
            self.zombie_step(0xFF, old);
            self.zombie_step(value, 0xFF);
        } else {
            self.zombie_step(value, old);
        }
        self.nrx2 = value;
    }

    const fn zombie_step(&mut self, value: u8, old_value: u8) {
        if self.clock.clock {
            self.countdown = value & 7;
        }
        let mut should_tick = (value & 7) != 0 && (old_value & 7) == 0 && !self.clock.locked;
        let should_invert = (value & 8) ^ (old_value & 8) != 0;

        if (value & 0xF) == 8 && (old_value & 0xF) == 8 && !self.clock.locked {
            should_tick = true;
        }

        if should_invert {
            // The way the clocks of this counter are connected cause some
            // odd ways for it to invert.
            if value & 8 != 0 {
                if (old_value & 7) == 0 && !self.clock.locked {
                    self.volume ^= 0xF;
                } else {
                    self.volume = 0xE_u8.wrapping_sub(self.volume) & 0xF;
                }
                should_tick = false; // Somehow prevents ticking?
            } else {
                self.volume = 0x10_u8.wrapping_sub(self.volume) & 0xF;
            }
        }
        if should_tick {
            if value & 8 != 0 {
                self.volume = self.volume.wrapping_add(1);
            } else {
                self.volume = self.volume.wrapping_sub(1);
            }
            self.volume &= 0xF;
        } else if (value & 7) == 0 && self.clock.clock {
            self.clock.set(false, false, 0);
        }
    }
}
