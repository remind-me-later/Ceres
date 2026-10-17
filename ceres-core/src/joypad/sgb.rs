//! The SGB's packet receiver on the P1 lines.

use super::P1_ACTIONS_B;

/// Bits in an SGB command packet (16 bytes).
const SGB_PACKET_BITS: usize = 16 * 8;
/// Longest SGB command: 7 packets.
const SGB_COMMAND_BYTES: usize = 7 * 16;

const SGB_MLT_REQ: u8 = 0x11;

/// The SGB's packet receiver on the P1 lines (SameBoy's `GB_sgb_write`). Only
/// the multiplayer command (`MLT_REQ`) is acted upon: it changes what the
/// joypad reports with no line selected.
pub(super) struct Sgb {
    command: [u8; SGB_COMMAND_BYTES],
    command_write_index: usize,
    /// The player counter.
    current_player: u8,
    /// `MLT_REQ`'s mode: 0 for one player, 1 for two and 3 for four, also the
    /// mask of the player counter. 2 is invalid (see [`Self::multiplayer_id`]).
    multiplayer_mode: u8,
    ready_for_pulse: bool,
    ready_for_stop: bool,
    ready_for_write: bool,
}

impl Sgb {
    pub(super) const fn new() -> Self {
        Self {
            command: [0; SGB_COMMAND_BYTES],
            command_write_index: 0,
            current_player: 0,
            multiplayer_mode: 0,
            ready_for_pulse: false,
            ready_for_stop: false,
            ready_for_write: false,
        }
    }

    /// The player the joypad reads with no line selected, in multiplayer.
    ///
    /// In the invalid mode 2 the counter is stuck and reads as player 2 or
    /// 0 (SameSuite's `command_mlt_req`).
    pub(super) const fn multiplayer_id(&self) -> Option<u8> {
        match self.multiplayer_mode {
            0 => None,
            2 => Some((self.current_player + 1) & 2),
            _ => Some(self.current_player),
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
            self.multiplayer_mode = self.command[1] & 3;
            if self.multiplayer_mode != 2 {
                self.current_player &= self.multiplayer_mode;
            }
        }
    }

    /// A write of `value` to P1 that changes the selected lines; `old` is the
    /// previous value of bits 4-5.
    pub(super) fn write(&mut self, old: u8, value: u8) {
        let command_size = if self.command[0] & 0xF1 == 0xF1 {
            SGB_PACKET_BITS
        } else {
            usize::from(self.command[0] & 7).max(1) * SGB_PACKET_BITS
        };

        if value & P1_ACTIONS_B != 0
            && old & P1_ACTIONS_B == 0
            && matches!(self.multiplayer_mode, 1 | 3)
        {
            self.current_player = (self.current_player + 1) & self.multiplayer_mode;
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
