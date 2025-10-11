use crate::{AudioCallback, Gb};

#[derive(Default)]
pub struct Dma {
    addr: u16,
    is_enabled: bool,
    is_restarting: bool, // FIXME: check usage of restarting and on
    reg: u8,
    remaining_dots: i32,
}

impl Dma {
    const fn advance_addr(&mut self) {
        self.addr = self.addr.wrapping_add(1);
        if self.addr & 0xFF > 0x9F {
            self.is_enabled = false;
            self.is_restarting = false;
        }
    }

    pub const fn advance_dots(&mut self, dots: i32) {
        self.remaining_dots += dots;
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.is_enabled && (self.remaining_dots > 0 || self.is_restarting)
    }

    pub const fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub const fn read(&self) -> u8 {
        self.reg
    }

    pub const fn remaining_dots(&self) -> i32 {
        self.remaining_dots
    }

    pub fn write(&mut self, val: u8) {
        if self.is_enabled {
            self.is_restarting = true;
        }

        self.remaining_dots = -8; // two m-cycles delay
        self.reg = val;
        self.addr = u16::from(val) << 8;
        self.is_enabled = true;
    }
}

impl<A: AudioCallback> Gb<A> {
    #[inline]
    pub fn run_dma(&mut self) {
        if !self.dma.is_enabled() {
            return;
        }

        while self.dma.remaining_dots() >= 4 {
            self.dma.remaining_dots -= 4;

            // TODO: reading some ranges should cause problems, $DF is
            // the maximum value accesible to OAM DMA (probably reads
            // from echo RAM should work too, RESEARCH).
            // what happens if reading from IO range? (garbage? 0xff?)
            let val = self.read_mem(self.dma.addr);

            // TODO: writes from DMA can access OAM on modes 2 and 3
            // with some glitches (RESEARCH) and without trouble during
            // VBLANK (what happens in HBLANK?)
            self.ppu.write_oam_by_dma(self.dma.addr, val);

            self.dma.advance_addr();
        }
    }
}
