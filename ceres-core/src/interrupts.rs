// IF and IE bits, by priority: the vector of bit n is 0x40 + 8 * n.
const INT_VBLANK_B: u8 = 0x01;
const INT_LCD_B: u8 = 0x02;
const INT_TIMER_B: u8 = 0x04;
const INT_SERIAL_B: u8 = 0x08;
const INT_P1_B: u8 = 0x10;
/// The five interrupt sources; the other bits of IF read 1.
pub const INT_MASK: u8 = 0x1F;
/// Where the vector of the first interrupt is.
pub const INT_VECTOR_BASE: u16 = 0x40;

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
        self.ifr | !INT_MASK
    }

    pub const fn request_lcd(&mut self) {
        self.ifr |= INT_LCD_B;
    }

    pub const fn request_p1(&mut self) {
        self.ifr |= INT_P1_B;
    }

    pub const fn request_serial(&mut self) {
        self.ifr |= INT_SERIAL_B;
    }

    pub const fn request_timer(&mut self) {
        self.ifr |= INT_TIMER_B;
    }

    pub const fn request_vblank(&mut self) {
        self.ifr |= INT_VBLANK_B;
    }

    pub const fn write_ie(&mut self, val: u8) {
        self.ie = val;
    }

    pub const fn write_if(&mut self, val: u8) {
        self.ifr = val & INT_MASK;
    }
}
