const VBLANK: u8 = 1;
const LCD: u8 = 2;
const TIMER: u8 = 4;
const SERIAL: u8 = 8;
const P1: u8 = 16;

#[derive(Default, Debug)]
pub struct Interrupts {
    ime: bool,
    ifr: u8,
    ie: u8,
}

impl Interrupts {
    #[inline]
    pub(crate) fn ill(&mut self) {
        self.ie = 0;
    }

    #[must_use]
    #[inline]
    pub(crate) fn handle(&mut self) -> u16 {
        let ints = self.ifr & self.ie;
        let tz = (ints.trailing_zeros() & 7) as u16;
        // get rightmost interrupt
        let int = u8::from(ints != 0) << tz;
        // acknowledge
        self.ifr &= !int;
        // compute direction of interrupt vector
        0x40 | tz << 3
    }

    #[must_use]
    #[inline]
    pub(crate) const fn is_any_requested(&self) -> bool {
        self.ifr & self.ie != 0
    }

    #[inline]
    pub(crate) fn enable(&mut self) {
        self.ime = true;
    }

    #[inline]
    pub(crate) fn disable(&mut self) {
        self.ime = false;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn are_enabled(&self) -> bool {
        self.ime
    }

    #[inline]
    pub(crate) fn request_p1(&mut self) {
        self.ifr |= P1;
    }

    #[inline]
    pub(crate) fn request_serial(&mut self) {
        self.ifr |= SERIAL;
    }

    #[inline]
    pub(crate) fn request_vblank(&mut self) {
        self.ifr |= VBLANK;
    }

    #[inline]
    pub(crate) fn request_lcd(&mut self) {
        self.ifr |= LCD;
    }

    #[inline]
    pub(crate) fn request_timer(&mut self) {
        self.ifr |= TIMER;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_if(&self) -> u8 {
        self.ifr | 0xE0
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_ie(&self) -> u8 {
        self.ie
    }

    #[inline]
    pub(crate) fn write_if(&mut self, val: u8) {
        self.ifr = val & 0x1F;
    }

    #[inline]
    pub(crate) fn write_ie(&mut self, val: u8) {
        self.ie = val;
    }
}
