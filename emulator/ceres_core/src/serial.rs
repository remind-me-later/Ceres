use crate::{interrupts::Interrupts, CMode};

const START: u8 = 0x80;
const SPEED: u8 = 0x2;
const SHIFT: u8 = 0x1;

// TODO: always off
#[derive(Default)]
pub struct Serial {
    sc: u8,
    sb: u8,
    count: u8,
    div_mask: u8,
    master_clock: bool,
}

impl Serial {
    pub(crate) fn run_master(&mut self, ints: &mut Interrupts) {
        self.master_clock ^= true;

        if !self.master_clock && (self.sc & (START | SHIFT) == (START | SHIFT)) {
            self.count += 1;
            if self.count > 7 {
                self.count = 0;
                ints.req_serial();
                self.sc &= !START;
            }

            self.sb <<= 1;

            // TODO: always off
            self.sb |= 1;
        }
    }

    pub(crate) const fn div_mask(&self) -> u8 {
        self.div_mask
    }

    pub(crate) const fn read_sb(&self) -> u8 {
        self.sb
    }

    pub(crate) const fn read_sc(&self) -> u8 {
        self.sc
    }

    pub(crate) fn write_sb(&mut self, val: u8) {
        self.sb = val;
    }

    pub(crate) fn write_sc(&mut self, mut val: u8, ints: &mut Interrupts, mode: CMode) {
        self.count = 0;

        if matches!(mode, CMode::Cgb) {
            val |= 2;
        }

        self.sc = val | !(START | SPEED | SHIFT);
        self.div_mask = if matches!(mode, CMode::Cgb) && val & SPEED != 0 {
            4
        } else {
            0x80
        };

        if self.master_clock {
            self.run_master(ints);
        }
    }
}
