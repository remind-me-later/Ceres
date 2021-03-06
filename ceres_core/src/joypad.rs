use crate::{Gb, IF_P1_B};

/// Represents a `GameBoy` physical button.
#[derive(Clone, Copy)]
pub enum Button {
    Right  = 0x01,
    Left   = 0x02,
    Up     = 0x04,
    Down   = 0x08,
    A      = 0x10,
    B      = 0x20,
    Select = 0x40,
    Start  = 0x80,
}

impl Gb {
    /// Press the `button` button.
    pub fn press(&mut self, button: Button) {
        let b = button as u8;

        self.p1_btn |= b;

        if b & 0x0F != 0 && self.p1_dirs || b & 0xF0 != 0 && self.p1_acts {
            self.ifr |= IF_P1_B;
        }
    }

    /// Release the `button` button.
    pub fn release(&mut self, button: Button) {
        self.p1_btn &= !(button as u8);
    }

    #[must_use]
    pub(crate) fn read_p1(&self) -> u8 {
        let act = if self.p1_acts {
            self.p1_btn >> 4 | 1 << 5
        } else {
            0
        };

        let dir = if self.p1_dirs {
            self.p1_btn & 0xF | 1 << 4
        } else {
            0
        };

        // pressed on low
        !(act | dir)
    }

    pub(crate) fn write_joy(&mut self, val: u8) {
        self.p1_acts = val & 0x20 == 0;
        self.p1_dirs = val & 0x10 == 0;
    }
}
