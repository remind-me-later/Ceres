const VBLANK: u8 = 1;
const LCD: u8 = 2;
const TIMER: u8 = 4;
const SERIAL: u8 = 8;
const P1: u8 = 16;

#[derive(Default)]
pub struct Interrupts {
    ie: u8,
    ifr: u8,
}

impl Interrupts {
    /// Acknowledges a specific interrupt by clearing its flag in IF.
    /// Should be called after the interrupt dispatch is complete.
    pub const fn acknowledge_interrupt(&mut self, int_bit: u8) {
        self.ifr &= !int_bit;
    }

    pub const fn illegal(&mut self) {
        self.ie = 0;
    }

    #[must_use]
    pub const fn is_any_requested(&self) -> bool {
        self.ifr & self.ie != 0
    }

    #[must_use]
    pub const fn read_ie(&self) -> u8 {
        self.ie
    }

    #[must_use]
    pub const fn read_if(&self) -> u8 {
        self.ifr | 0xE0
    }

    pub const fn request_lcd(&mut self) {
        self.ifr |= LCD;
    }

    pub const fn request_p1(&mut self) {
        self.ifr |= P1;
    }

    pub const fn request_serial(&mut self) {
        self.ifr |= SERIAL;
    }

    pub const fn request_timer(&mut self) {
        self.ifr |= TIMER;
    }

    pub const fn request_vblank(&mut self) {
        self.ifr |= VBLANK;
    }

    pub const fn write_ie(&mut self, val: u8) {
        self.ie = val;
    }

    pub const fn write_if(&mut self, val: u8) {
        self.ifr = val & 0x1F;
    }
}
