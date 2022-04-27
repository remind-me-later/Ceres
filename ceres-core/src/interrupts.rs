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
    pub fn handler_address(self) -> u16 {
        match self {
            Self::VBLANK => 0x40,
            Self::LCD_STAT => 0x48,
            Self::TIMER => 0x50,
            Self::SERIAL => 0x58,
            Self::JOYPAD => 0x60,
            _ => unreachable!("bad interrupt"),
        }
    }
}

pub struct Interrupts {
    interrupt_flag: Interrupt,
    interrupt_enable: u8,
}

impl Interrupts {
    pub fn new() -> Self {
        Self {
            interrupt_flag: Interrupt::from_bits_truncate(0),
            interrupt_enable: 0x00,
        }
    }

    pub fn halt_bug_condition(&self) -> bool {
        self.interrupt_flag.bits() & self.interrupt_enable & 0x1f != 0
    }

    pub fn has_pending_interrupts(&self) -> bool {
        !(self.interrupt_flag & Interrupt::from_bits_truncate(self.interrupt_enable)).is_empty()
    }

    pub fn pending_interrupts(&self) -> Interrupt {
        self.interrupt_flag & Interrupt::from_bits_truncate(self.interrupt_enable)
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
        self.interrupt_flag.insert(interrupt);
    }

    pub fn acknowledge(&mut self, interrupt: Interrupt) {
        self.interrupt_flag.remove(interrupt);
    }

    pub fn read_if(&self) -> u8 {
        self.interrupt_flag.bits() | 0xe0
    }

    pub fn read_ie(&self) -> u8 {
        self.interrupt_enable
    }

    pub fn write_if(&mut self, value: u8) {
        self.interrupt_flag = Interrupt::from_bits_truncate(value)
    }

    pub fn write_ie(&mut self, value: u8) {
        self.interrupt_enable = value
    }
}
