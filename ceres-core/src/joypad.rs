use crate::interrupts::Interrupts;

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Order follows the button bit representation"
)]
#[expect(clippy::exhaustive_enums, reason = "Exhaustive by design")]
#[derive(Clone, Copy)]
pub enum Button {
    Right = 0x01,
    Left = 0x02,
    Up = 0x04,
    Down = 0x08,
    A = 0x10,
    B = 0x20,
    Select = 0x40,
    Start = 0x80,
}

// P1 bits
/// Low selects the action buttons (A, B, Select, Start).
const P1_ACTIONS_B: u8 = 0x20;
/// Low selects the direction keys.
const P1_DIRECTIONS_B: u8 = 0x10;
const P1_SELECT: u8 = P1_ACTIONS_B | P1_DIRECTIONS_B;
/// The input lines, low while a selected button is pressed.
const P1_INPUTS: u8 = 0x0F;

/// Bits in an SGB command packet (16 bytes).
const SGB_PACKET_BITS: usize = 16 * 8;
/// Longest SGB command: 7 packets.
const SGB_COMMAND_BYTES: usize = 7 * 16;

const SGB_MLT_REQ: u8 = 0x11;

/// The SGB's packet receiver on the P1 lines (SameBoy's `GB_sgb_write`). Only
/// the multiplayer command (`MLT_REQ`) is acted upon: it changes what the
/// joypad reports with no line selected.
struct Sgb {
    command: [u8; SGB_COMMAND_BYTES],
    command_write_index: usize,
    current_player: u8,
    /// 1, 2 or 4 players.
    player_count: u8,
    ready_for_pulse: bool,
    ready_for_stop: bool,
    ready_for_write: bool,
}

impl Sgb {
    const fn new() -> Self {
        Self {
            command: [0; SGB_COMMAND_BYTES],
            command_write_index: 0,
            current_player: 0,
            player_count: 1,
            ready_for_pulse: false,
            ready_for_stop: false,
            ready_for_write: false,
        }
    }

    const fn clear_command(&mut self) {
        self.command_write_index = 0;
        self.command = [0; SGB_COMMAND_BYTES];
    }

    /// A command bit was received (zero unless it was set first).
    const fn next_command_bit(&mut self) {
        self.command_write_index += 1;
        self.ready_for_pulse = false;
        if self.command_write_index & (SGB_PACKET_BITS - 1) == 0 {
            self.ready_for_stop = true;
        }
    }

    const fn command_ready(&mut self) {
        if self.command[0] >> 3 == SGB_MLT_REQ {
            self.player_count = match self.command[1] & 3 {
                0 => 1,
                1 => 2,
                _ => 4,
            };
            self.current_player &= self.player_count - 1;
        }
    }

    /// A write of `value` to P1 that changes the selected lines; `old` is the
    /// previous value of bits 4-5.
    fn write(&mut self, old: u8, value: u8) {
        let command_size = if self.command[0] & 0xF1 == 0xF1 {
            SGB_PACKET_BITS
        } else {
            usize::from(self.command[0] & 7).max(1) * SGB_PACKET_BITS
        };

        if value & P1_ACTIONS_B != 0 && old & P1_ACTIONS_B == 0 && self.player_count & 1 == 0 {
            self.current_player = (self.current_player + 1) & (self.player_count - 1);
        }

        match (value >> 4) & 3 {
            3 => self.ready_for_pulse = true,
            // Zero
            2 => {
                if !self.ready_for_pulse || !self.ready_for_write {
                    return;
                }
                if self.ready_for_stop {
                    if self.command_write_index == command_size {
                        self.command_ready();
                        self.clear_command();
                    }
                    self.ready_for_pulse = false;
                    self.ready_for_write = false;
                    self.ready_for_stop = false;
                } else if self.command_write_index < SGB_COMMAND_BYTES * 8 {
                    self.next_command_bit();
                } else {
                    // The command buffer is full: the bit is dropped.
                }
            }
            // One
            1 => {
                if !self.ready_for_pulse || !self.ready_for_write {
                    return;
                }
                if self.ready_for_stop {
                    // Corrupt command.
                    self.ready_for_pulse = false;
                    self.ready_for_write = false;
                    self.clear_command();
                } else if self.command_write_index < SGB_COMMAND_BYTES * 8 {
                    self.command[self.command_write_index / 8] |=
                        1 << (self.command_write_index & 7);
                    self.next_command_bit();
                } else {
                    // The command buffer is full: the bit is dropped.
                }
            }
            // Reset pulse.
            _ => {
                if !self.ready_for_pulse {
                    return;
                }
                self.ready_for_write = true;
                self.ready_for_pulse = false;
                if self.command_write_index & (SGB_PACKET_BITS - 1) != 0
                    || self.command_write_index == 0
                    || self.ready_for_stop
                {
                    self.clear_command();
                    self.ready_for_stop = false;
                }
            }
        }
    }
}

pub(crate) struct Joypad {
    // P1
    actions_flag: bool,
    button_mask: u8,
    directions_flag: bool,
    sgb: Option<Sgb>,
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Joypad {
    #[must_use]
    pub(crate) fn new(is_sgb: bool) -> Self {
        Self {
            actions_flag: true,
            directions_flag: true,
            button_mask: 0,
            sgb: is_sgb.then(Sgb::new),
        }
    }

    pub(crate) const fn press(&mut self, button: Button, ints: &mut Interrupts) {
        let old = self.read_p1();
        self.button_mask |= button as u8;
        // The interrupt fires when an input line goes low: not for a button
        // that is already held or not selected.
        if old & !self.read_p1() & P1_INPUTS != 0 {
            ints.request_p1();
        }
    }

    #[must_use]
    pub(crate) const fn read_p1(&self) -> u8 {
        // Bits 6 and 7 read 1.
        let mut res = !P1_SELECT;

        if self.actions_flag {
            res &= !(self.button_mask >> 4);
        } else {
            res |= P1_ACTIONS_B;
        }

        if self.directions_flag {
            res &= !(self.button_mask & P1_INPUTS);
        } else {
            res |= P1_DIRECTIONS_B;
        }

        // With no line selected a multiplayer SGB reports the player ID.
        if !self.actions_flag
            && !self.directions_flag
            && let Some(ref sgb) = self.sgb
            && sgb.player_count > 1
        {
            res = (res & !P1_INPUTS) | (P1_INPUTS - sgb.current_player);
        }

        res
    }

    pub(crate) const fn release(&mut self, button: Button) {
        self.button_mask &= !(button as u8);
    }

    pub(crate) fn write_joy(&mut self, val: u8) {
        let old = (u8::from(!self.actions_flag) << 5) | (u8::from(!self.directions_flag) << 4);
        if let Some(ref mut sgb) = self.sgb {
            // The packet receiver only sees changes of the selected lines.
            if old != val & P1_SELECT {
                sgb.write(old, val);
            }
        }
        self.actions_flag = val & P1_ACTIONS_B == 0;
        self.directions_flag = val & P1_DIRECTIONS_B == 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{Button, Joypad, P1_ACTIONS_B};
    use crate::interrupts::{INT_MASK, Interrupts};

    #[test]
    fn interrupt_only_when_a_line_goes_low() {
        let mut joy = Joypad::new(false);
        let mut ints = Interrupts::default();
        // Only the directions are selected.
        joy.write_joy(P1_ACTIONS_B);

        joy.press(Button::A, &mut ints);
        assert_eq!(ints.read_if() & INT_MASK, 0, "actions not selected");

        joy.press(Button::Right, &mut ints);
        assert_ne!(ints.read_if() & INT_MASK, 0, "right pressed");

        ints.write_if(0);
        joy.press(Button::Right, &mut ints);
        assert_eq!(ints.read_if() & INT_MASK, 0, "right already held");
    }
}
