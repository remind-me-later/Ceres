pub const VBLANK_INT: u8 = 1;
pub const LCD_STAT_INT: u8 = 1 << 1;
pub const TIMER_INT: u8 = 1 << 2;
pub const SERIAL_INT: u8 = 1 << 3;
pub const JOYPAD_INT: u8 = 1 << 4;

#[derive(Default)]
pub struct Interrupts {
    interrupt_flag: u8,
    interrupt_enable: u8,
}

impl Interrupts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_pending_interrupts(&self) -> bool {
        self.interrupt_flag & self.interrupt_enable & 0x1f != 0
    }

    pub fn pending_interrupts(&self) -> u8 {
        self.interrupt_flag & self.interrupt_enable & 0x1f
    }

    pub fn requested_interrupt(&self) -> u8 {
        let pending = self.pending_interrupts();

        if pending == 0 {
            return 0;
        }

        // get rightmost interrupt
        1 << pending.trailing_zeros()
    }

    pub fn request(&mut self, interrupt: u8) {
        self.interrupt_flag |= interrupt;
    }

    pub fn acknowledge(&mut self, interrupt: u8) {
        self.interrupt_flag &= !interrupt;
    }

    pub fn read_if(&self) -> u8 {
        self.interrupt_flag | 0xe0
    }

    pub fn read_ie(&self) -> u8 {
        self.interrupt_enable
    }

    pub fn write_if(&mut self, value: u8) {
        self.interrupt_flag = value & 0x1f
    }

    pub fn write_ie(&mut self, value: u8) {
        self.interrupt_enable = value
    }
}
