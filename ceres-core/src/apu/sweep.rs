//! The frequency sweep of channel 1 (NR10).

use super::{Ctx, revision::Revision, square::Square};

#[derive(Clone, Copy)]
pub struct Sweep {
    nr10: u8,
    /// Advances every 4th DIV event; a sweep step happens when it reaches 7.
    countdown: u8,
    /// The new period is calculated, and checked for overflow, after a delay.
    calculate_countdown: u8,
    calculate_countdown_reload_timer: u8,
    instant_calculation_done: bool,
    /// The period shifted by the sweep step.
    addend: u16,
    shadow_period: u16,
    unshifted: bool,
    /// The sweep does not see a trigger's period for a few ticks.
    restart_hold: u8,
    completed_addend: u16,
}

impl Sweep {
    pub const fn new() -> Self {
        Self {
            nr10: 0,
            countdown: 0,
            calculate_countdown: 0,
            calculate_countdown_reload_timer: 0,
            instant_calculation_done: false,
            addend: 0,
            shadow_period: 0,
            unshifted: false,
            restart_hold: 0,
            completed_addend: 0,
        }
    }

    pub const fn read_nr10(&self) -> u8 {
        self.nr10 | 0x80
    }

    pub const fn set_countdown(&mut self, countdown: u8) {
        self.countdown = countdown;
    }

    const fn shift(&self) -> u8 {
        self.nr10 & 7
    }

    const fn decreases(&self) -> bool {
        self.nr10 & 8 != 0
    }

    const fn pace(&self) -> u8 {
        (self.nr10 >> 4) & 7
    }

    fn calculation_done(&mut self, ch1: &mut Square, c: &Ctx) {
        // APU bug: the sweep frequency is checked after adding the delta twice.
        if self.restart_hold == 0 {
            self.shadow_period = ch1.period();
        }
        if self.decreases() {
            self.addend ^= 0x7FF;
        }
        if u32::from(self.shadow_period) + u32::from(self.addend) > 0x7FF && !self.decreases() {
            ch1.disable(c);
        }
        self.completed_addend = self.addend;
    }

    /// Every 4th DIV event.
    pub fn div_event(&mut self, ch1: &mut Square, c: &Ctx) {
        self.countdown = (self.countdown + 1) & 7;
        self.trigger_calculation(ch1, c);
    }

    fn trigger_calculation(&mut self, ch1: &mut Square, c: &Ctx) {
        if self.pace() == 0 || self.countdown != 7 {
            return;
        }
        if self.shift() != 0 {
            ch1.set_period(
                self.addend
                    .wrapping_add(self.shadow_period)
                    .wrapping_add(u16::from(self.decreases()))
                    & 0x7FF,
            );
        }
        if self.restart_hold == 0 {
            self.addend = ch1.period() >> self.shift();
        }

        // Recalculation and the overflow check only occur after a delay.
        self.calculate_countdown = self.shift();
        self.calculate_countdown_reload_timer = 1 + c.lf_div;
        if !c.double_speed && c.during_div_write {
            self.calculate_countdown_reload_timer = 1;
        }
        self.unshifted = self.shift() == 0;
        self.countdown = self.pace() ^ 7;
        if self.calculate_countdown == 0 {
            self.instant_calculation_done = true;
        }
    }

    /// Advances the sweep by `cycles` 2 MHz ticks (it runs at 1 MHz).
    pub fn run(&mut self, cycles: u32, ch1: &mut Square, c: &Ctx) {
        let mut sweep_cycles = cycles / 2;
        if cycles & 1 != 0 && c.lf_div == 0 {
            sweep_cycles += 1;
        }

        let reload = u32::from(self.calculate_countdown_reload_timer);
        if reload > sweep_cycles {
            self.calculate_countdown_reload_timer -= sweep_cycles as u8;
            sweep_cycles = 0;
        } else {
            if reload != 0 && self.calculate_countdown == 0 && self.instant_calculation_done {
                self.calculation_done(ch1, c);
            }
            self.instant_calculation_done = false;
            sweep_cycles -= reload;
            self.calculate_countdown_reload_timer = 0;
        }

        if self.calculate_countdown != 0 && (self.shift() != 0 || self.unshifted) {
            // The calculation is paused if the lower bits are 0.
            if u32::from(self.calculate_countdown) > sweep_cycles {
                self.calculate_countdown -= sweep_cycles as u8;
            } else {
                self.calculate_countdown = 0;
                self.calculation_done(ch1, c);
            }
        }

        if self.restart_hold != 0 {
            if u32::from(self.restart_hold) > cycles {
                self.restart_hold -= cycles as u8;
            } else {
                self.restart_hold = 0;
            }
        }
    }

    /// Channel 1 was triggered (`was_active`: it was already playing).
    pub fn trigger(&mut self, ch1: &Square, was_active: bool, c: &Ctx) {
        self.instant_calculation_done = false;
        self.shadow_period = 0;
        self.completed_addend = 0;
        if self.shift() != 0 {
            // APU bug: if the shift is nonzero the overflow check also happens
            // on trigger.
            self.calculate_countdown = self.shift();
            self.calculate_countdown_reload_timer =
                if ((c.lf_div != 0) ^ !c.double_speed) && c.rev <= Revision::CgbC {
                    3
                } else {
                    2
                };
            self.unshifted = false;
            if !was_active {
                self.calculate_countdown_reload_timer += 1;
            }
            self.addend = ch1.period() >> self.shift();
        } else {
            self.addend = 0;
        }
        self.restart_hold = 2 - c.lf_div + u8::from(c.rev.is_cgb() && c.rev != Revision::CgbD) * 2;
        self.countdown = self.pace() ^ 7;
    }

    pub fn write_nr10(&mut self, value: u8, ch1: &mut Square, c: &Ctx) {
        if self.calculate_countdown != 0 || self.calculate_countdown_reload_timer != 0 {
            self.write_glitch(value, ch1, c);
        }
        let old_negate = self.decreases() || c.rev <= Revision::CgbC;
        self.nr10 = value;
        if u32::from(self.shadow_period) + u32::from(self.completed_addend) + u32::from(old_negate)
            > 0x7FF
            && !self.decreases()
        {
            ch1.disable(c);
        }
        self.trigger_calculation(ch1, c);
    }

    /// Writing NR10 during a calculation.
    fn write_glitch(&mut self, value: u8, ch1: &mut Square, c: &Ctx) {
        // TODO (SameBoy): check all of these in APU odd mode.
        if c.rev <= Revision::CgbC {
            if self.calculate_countdown_reload_timer == 1 && c.lf_div == 0 {
                if c.double_speed {
                    // Instance-specific data corruption (two CGB-Cs and a CGB-A).
                    const CORRUPTION: [u8; 8] = [7, 7, 5, 7, 3, 3, 5, 7];
                    self.calculate_countdown =
                        CORRUPTION[usize::from(self.calculate_countdown & 7)];
                }
            } else if self.calculate_countdown_reload_timer > 1 {
                if c.double_speed {
                    self.calculate_countdown = value & 7;
                }
            } else if self.calculate_countdown != 0 {
                let should_zombie_step = if self.shift() == 0 {
                    (c.lf_div != 0) ^ c.double_speed
                } else {
                    c.double_speed && self.calculate_countdown == 1
                };
                if should_zombie_step {
                    self.calculate_countdown -= 1;
                    if self.calculate_countdown <= 1 {
                        self.calculate_countdown = 0;
                        self.calculation_done(ch1, c);
                    }
                }
            }
        } else {
            if self.calculate_countdown_reload_timer == 2 {
                // The countdown just reloaded: re-reload it.
                self.calculate_countdown = value & 0x7;
                if self.calculate_countdown == 0 {
                    self.calculate_countdown_reload_timer = 0;
                }
            }
            if value & 7 != 0 && self.shift() == 0 && c.lf_div == 0 && self.calculate_countdown > 1
            {
                self.calculate_countdown -= 1;
                if self.calculate_countdown == 0 {
                    self.calculation_done(ch1, c);
                }
            }
        }
    }
}
