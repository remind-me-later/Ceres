mod sgb;

use crate::interrupts::Interrupts;
use sgb::Sgb;

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
            && let Some(id) = sgb.multiplayer_id()
        {
            res = (res & !P1_INPUTS) | (P1_INPUTS - id);
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
