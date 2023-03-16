const VBLANK: u8 = 1;
const LCD: u8 = 2;
const TIMER: u8 = 4;
const SERIAL: u8 = 8;
const P1: u8 = 16;

#[derive(Default)]
pub struct Interrupts {
    ime: bool,
    ifr: u8,
    ie: u8,
}

impl Interrupts {
    pub(crate) fn ill(&mut self) {
        self.ie = 0;
    }

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

    pub(crate) const fn any(&self) -> bool {
        self.ifr & self.ie != 0
    }

    pub(crate) fn enable(&mut self) {
        self.ime = true;
    }

    pub(crate) fn disable(&mut self) {
        self.ime = false;
    }

    pub(crate) const fn enabled(&self) -> bool {
        self.ime
    }

    pub(crate) fn req_p1(&mut self) {
        self.ifr |= P1;
    }

    pub(crate) fn req_serial(&mut self) {
        self.ifr |= SERIAL;
    }

    pub(crate) fn req_vblank(&mut self) {
        self.ifr |= VBLANK;
    }

    pub(crate) fn req_lcd(&mut self) {
        self.ifr |= LCD;
    }

    pub(crate) fn req_timer(&mut self) {
        self.ifr |= TIMER;
    }

    pub(crate) const fn read_if(&self) -> u8 {
        self.ifr | 0xE0
    }

    pub(crate) const fn read_ie(&self) -> u8 {
        self.ie
    }

    pub(crate) fn write_if(&mut self, val: u8) {
        self.ifr = val & 0x1F;
    }

    pub(crate) fn write_ie(&mut self, val: u8) {
        self.ie = val;
    }
}
