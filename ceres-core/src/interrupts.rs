use bitflags::bitflags;

bitflags! {
    pub struct Interrupt: u8 {
        const VBLANK   = 1;
        const LCD_STAT = 1 << 1;
        const TIMER    = 1 << 2;
        const SERIAL   = 1 << 3;
        const JOYPAD   = 1 << 4;
    }
}

impl Interrupt {
    pub const fn handler_address(self) -> u16 {
        match self {
            Self::VBLANK => 0x40,
            Self::LCD_STAT => 0x48,
            Self::TIMER => 0x50,
            Self::SERIAL => 0x58,
            Self::JOYPAD => 0x60,
            _ => 0x00,
        }
    }
}

#[derive(Clone, Copy)]
pub enum ICRegister {
    If,
    Ie,
}

pub struct InterruptController {
    ifr: Interrupt,
    ier: u8,
}

impl InterruptController {
    pub const fn new() -> Self {
        Self {
            ifr: Interrupt::from_bits_truncate(1),
            ier: 0x00,
        }
    }

    pub const fn halt_bug_condition(&self) -> bool {
        self.ifr.bits() & self.ier & 0x1f != 0
    }

    pub fn pending_interrupts(&self) -> Interrupt {
        self.ifr & Interrupt::from_bits_truncate(self.ier)
    }

    pub fn requested_interrupt(&self) -> Interrupt {
        let pending = self.pending_interrupts();

        if pending.is_empty() {
            return Interrupt::empty();
        }

        // get rightmost interrupt
        Interrupt::from_bits_truncate(1 << pending.bits().trailing_zeros())
    }

    pub fn request(&mut self, interrupt: Interrupt) {
        self.ifr.insert(interrupt);
    }

    pub fn acknowledge(&mut self, interrupt: Interrupt) {
        self.ifr.remove(interrupt);
    }

    pub const fn read(&self, register: ICRegister) -> u8 {
        match register {
            ICRegister::If => self.ifr.bits() | 0xe0,
            ICRegister::Ie => self.ier,
        }
    }

    pub fn write(&mut self, register: ICRegister, value: u8) {
        match register {
            ICRegister::If => self.ifr = Interrupt::from_bits_truncate(value),
            ICRegister::Ie => self.ier = value,
        }
    }
}
