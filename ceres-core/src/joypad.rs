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
    fn new() -> Self {
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

    fn clear_command(&mut self) {
        self.command_write_index = 0;
        self.command = [0; SGB_COMMAND_BYTES];
    }

    fn command_ready(&mut self) {
        if self.command[0] >> 3 == SGB_MLT_REQ {
            self.player_count = (self.command[1] & 3) + 1;
            if self.player_count == 3 {
                self.player_count += 1;
            }
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

        if value & 0x20 != 0 && old & 0x20 == 0 && self.player_count & 1 == 0 {
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
                    self.command_write_index += 1;
                    self.ready_for_pulse = false;
                    if self.command_write_index & (SGB_PACKET_BITS - 1) == 0 {
                        self.ready_for_stop = true;
                    }
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
                    self.command_write_index += 1;
                    self.ready_for_pulse = false;
                    if self.command_write_index & (SGB_PACKET_BITS - 1) == 0 {
                        self.ready_for_stop = true;
                    }
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

pub struct Joypad {
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
    pub fn new(is_sgb: bool) -> Self {
        Self {
            actions_flag: true,
            directions_flag: true,
            button_mask: 0,
            sgb: is_sgb.then(Sgb::new),
        }
    }

    pub const fn press(&mut self, button: Button, ints: &mut Interrupts) {
        let b = button as u8;

        self.button_mask |= b;

        if b & 0x0F != 0 && self.directions_flag || b & 0xF0 != 0 && self.actions_flag {
            ints.request_p1();
        }
    }

    #[must_use]
    pub const fn read_p1(&self) -> u8 {
        let mut res = 0xCF;

        if self.actions_flag {
            res &= !(self.button_mask >> 4);
        } else {
            res |= 0x20;
        }

        if self.directions_flag {
            res &= !(self.button_mask & 0xF);
        } else {
            res |= 0x10;
        }

        // With no line selected a multiplayer SGB reports the player ID.
        if !self.actions_flag
            && !self.directions_flag
            && let Some(sgb) = &self.sgb
            && sgb.player_count > 1
        {
            res = (res & 0xF0) | (0xF - sgb.current_player);
        }

        res
    }

    pub const fn release(&mut self, button: Button) {
        self.button_mask &= !(button as u8);
    }

    pub fn write_joy(&mut self, val: u8) {
        let old = (u8::from(!self.actions_flag) << 5) | (u8::from(!self.directions_flag) << 4);
        if let Some(sgb) = &mut self.sgb {
            // The packet receiver only sees changes of the selected lines.
            if old != val & 0x30 {
                sgb.write(old, val);
            }
        }
        self.actions_flag = val & 0x20 == 0;
        self.directions_flag = val & 0x10 == 0;
    }
}
