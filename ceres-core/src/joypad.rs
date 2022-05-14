use crate::interrupts::{Interrupts, JOYPAD_INT};

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

#[derive(Default)]
pub struct Joypad {
    btns: u8,
    dirs: bool,
    acts: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn press(&mut self, ints: &mut Interrupts, button: Button) {
        self.btns |= button as u8;

        if button as u8 & 0x0f != 0 && self.dirs {
            ints.req(JOYPAD_INT);
        }

        if button as u8 & 0xf0 != 0 && self.acts {
            ints.req(JOYPAD_INT);
        }
    }

    pub fn release(&mut self, button: Button) {
        self.btns &= !(button as u8);
    }

    pub const fn read(&self) -> u8 {
        let act_bits = if self.acts {
            self.btns >> 4 | 1 << 5
        } else {
            0
        };

        let dir_bits = if self.dirs {
            self.btns & 0xf | 1 << 4
        } else {
            0
        };

        // pressed on low
        !(act_bits | dir_bits)
    }

    pub fn write(&mut self, val: u8) {
        self.acts = (val >> 5) & 1 == 0;
        self.dirs = (val >> 4) & 1 == 0;
    }
}
