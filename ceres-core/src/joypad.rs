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
    p1_btn: u8,
    p1_dirs: bool,
    p1_acts: bool,
}

impl Joypad {
    pub fn press(&mut self, button: Button, ints: &mut Interrupts) {
        let b = button as u8;

        self.p1_btn |= b;

        if b & 0x0F != 0 && self.p1_dirs || b & 0xF0 != 0 && self.p1_acts {
            ints.request_p1();
        }
    }

    pub fn release(&mut self, button: Button) {
        self.p1_btn &= !(button as u8);
    }

    #[must_use]
    pub const fn read_p1(&self) -> u8 {
        let act = if self.p1_acts {
            (self.p1_btn >> 4) | (1 << 5)
        } else {
            0
        };

        let dir = if self.p1_dirs {
            self.p1_btn & 0xF | (1 << 4)
        } else {
            0
        };

        // pressed on low
        !(act | dir)
    }

    pub fn write_joy(&mut self, val: u8) {
        self.p1_acts = val & 0x20 == 0;
        self.p1_dirs = val & 0x10 == 0;
    }
}
