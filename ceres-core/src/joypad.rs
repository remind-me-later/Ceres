use crate::interrupts::{Interrupt, Interrupts};

#[derive(Clone, Copy)]
pub enum Button {
    Right  = 1,
    Left   = 2,
    Up     = 4,
    Down   = 8,
    A      = 1 << 4,
    B      = 2 << 4,
    Select = 4 << 4,
    Start  = 8 << 4,
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
            ints.request(Interrupt::JOYPAD);
        }

        if button as u8 & 0xf0 != 0 && self.acts {
            ints.request(Interrupt::JOYPAD);
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
