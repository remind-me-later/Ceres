use crate::Gb;

pub const VBLANK_INT: u8 = 1;
pub const LCD_STAT_INT: u8 = 1 << 1;
pub const TIMER_INT: u8 = 1 << 2;
pub const SERIAL_INT: u8 = 1 << 3;
pub const JOYPAD_INT: u8 = 1 << 4;

impl Gb {
    #[must_use]
    pub(crate) fn any_interrrupt(&self) -> bool {
        self.interrupt_flag & self.interrupt_enable & 0x1f != 0
    }

    pub(crate) fn req_interrupt(&mut self, interrupt: u8) {
        self.interrupt_flag |= interrupt;
    }
}
