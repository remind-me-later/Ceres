use crate::{interrupts::JOYPAD_INT, Gb};

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
    pub fn press(&mut self, button: Button) {
        self.joy_btns |= button as u8;

        if button as u8 & 0x0f != 0 && self.joy_dirs {
            self.req_interrupt(JOYPAD_INT);
        }

        if button as u8 & 0xf0 != 0 && self.joy_acts {
            self.req_interrupt(JOYPAD_INT);
        }
    }

    pub fn release(&mut self, button: Button) {
        self.joy_btns &= !(button as u8);
    }

    #[must_use]
    pub(crate) fn read_joy(&self) -> u8 {
        let act_bits = if self.joy_acts {
            self.joy_btns >> 4 | 1 << 5
        } else {
            0
        };

        let dir_bits = if self.joy_dirs {
            self.joy_btns & 0xf | 1 << 4
        } else {
            0
        };

        // pressed on low
        !(act_bits | dir_bits)
    }

    pub(crate) fn write_joy(&mut self, val: u8) {
        self.joy_acts = (val >> 5) & 1 == 0;
        self.joy_dirs = (val >> 4) & 1 == 0;
    }
}
