const VBLANK: u8 = 1;
const LCD: u8 = 2;
const TIMER: u8 = 4;
const SERIAL: u8 = 8;
const P1: u8 = 16;

#[derive(Default)]
pub struct Interrupts {
    ie: u8,
    ifr: u8,
    ime: bool,
}

impl Interrupts {
    /// Acknowledges a specific interrupt by clearing its flag in IF.
    /// Should be called after the interrupt dispatch is complete.
    pub const fn acknowledge_interrupt(&mut self, int_bit: u8) {
        self.ifr &= !int_bit;
    }

    #[must_use]
    pub const fn are_enabled(&self) -> bool {
        self.ime
    }

    /// Determines which interrupt should be dispatched based on current IE & IF state.
    /// Returns the interrupt bit and vector address.
    /// Used during interrupt dispatch to allow IE re-checking mid-push.
    /// Returns (0, 0x0000) if no interrupt should be dispatched.
    #[must_use]
    pub fn determine_interrupt(&self) -> (u8, u16) {
        let ints = self.ifr & self.ie;
        if ints == 0 {
            // No interrupt to dispatch - return 0x0000 as vector
            return (0, 0x0000);
        }
        let tz = (ints.trailing_zeros() & 7) as u16;
        // get rightmost interrupt bit
        let int = u8::from(ints != 0) << tz;
        // compute interrupt vector
        let vector = 0x40 | (tz << 3);
        (int, vector)
    }

    pub const fn disable(&mut self) {
        self.ime = false;
    }

    pub const fn enable(&mut self) {
        self.ime = true;
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
        self.ie | 0xE0
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
        self.ie = val & 0x1F;
    }

    pub const fn write_if(&mut self, val: u8) {
        self.ifr = val & 0x1F;
    }
}
