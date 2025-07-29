use crate::interrupts::Interrupts;

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

#[derive(Default, Debug)]
pub struct Joypad {
    // P1
    button_mask: u8,
    directions_flag: bool,
    actions_flag: bool,
}

impl Joypad {
    pub const fn press(&mut self, button: Button, ints: &mut Interrupts) {
        let b = button as u8;

        self.button_mask |= b;

        if b & 0x0F != 0 && self.directions_flag || b & 0xF0 != 0 && self.actions_flag {
            ints.request_p1();
        }
    }

    pub const fn release(&mut self, button: Button) {
        self.button_mask &= !(button as u8);
    }

    #[must_use]
    pub const fn read_p1(&self) -> u8 {
        let act = if self.actions_flag {
            (self.button_mask >> 4) | (1 << 5)
        } else {
            0
        };

        let dir = if self.directions_flag {
            self.button_mask & 0xF | (1 << 4)
        } else {
            0
        };

        // pressed on low
        !(act | dir)
    }

    pub const fn write_joy(&mut self, val: u8) {
        self.actions_flag = val & 0x20 == 0;
        self.directions_flag = val & 0x10 == 0;
    }
}
