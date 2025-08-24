use crate::{AudioCallback, Gb, ppu};

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Order follows the state machine transitions"
)]
#[derive(Default)]
pub enum HdmaState {
    #[default]
    Sleep,
    WaitHBlank,
    HBlankDone,
    General,
}

#[derive(Default)]
pub struct Hdma {
    dst: u16,
    hdma5: u8,
    len: u16,
    src: u16,
    state: HdmaState,
}

impl Hdma {
    #[must_use]
    const fn is_on(&self) -> bool {
        !matches!(self.state, HdmaState::Sleep)
    }

    #[must_use]
    pub const fn read_hdma5(&self) -> u8 {
        // active on low
        ((!self.is_on() as u8) << 7) | self.hdma5
    }

    pub fn write_hdma1(&mut self, val: u8) {
        self.src = (u16::from(val) << 8) | (self.src & 0xF0);
    }

    pub fn write_hdma2(&mut self, val: u8) {
        self.src = (self.src & 0xFF00) | u16::from(val & 0xF0);
    }

    pub fn write_hdma3(&mut self, val: u8) {
        self.dst = (u16::from(val & 0x1F) << 8) | (self.dst & 0xF0);
    }

    pub fn write_hdma4(&mut self, val: u8) {
        self.dst = (self.dst & 0x1F00) | u16::from(val & 0xF0);
    }

    pub fn write_hdma5(&mut self, val: u8) {
        use HdmaState::{General, Sleep, WaitHBlank};

        debug_assert!(
            !matches!(self.state, HdmaState::General),
            "HDMA transfer in progress, cannot write HDMA5"
        );

        // stop current transfer
        if self.is_on() && val & 0x80 == 0 {
            self.state = Sleep;
            return;
        }

        self.hdma5 = val & 0x7F;
        self.len = (u16::from(self.hdma5) + 1) * 0x10;
        self.state = if val & 0x80 == 0 { General } else { WaitHBlank };
    }
}

impl<A: AudioCallback> Gb<A> {
    pub fn run_hdma(&mut self) {
        use HdmaState::{General, HBlankDone, Sleep, WaitHBlank};

        match self.hdma.state {
            General => (),
            WaitHBlank if matches!(self.ppu.mode(), ppu::Mode::HBlank) => (),
            HBlankDone if !matches!(self.ppu.mode(), ppu::Mode::HBlank) => {
                self.hdma.state = WaitHBlank;
                return;
            }
            _ => return,
        }

        let len = if matches!(self.hdma.state, WaitHBlank) {
            self.hdma.len -= 0x10;
            self.hdma.state = if self.hdma.len == 0 {
                Sleep
            } else {
                HBlankDone
            };
            self.hdma.hdma5 = ((self.hdma.len / 0x10).wrapping_sub(1) & 0xFF) as u8;
            0x10
        } else {
            self.hdma.state = Sleep;
            self.hdma.hdma5 = 0xFF;
            let len = self.hdma.len;
            self.hdma.len = 0;
            len
        };

        for _ in 0..len {
            // TODO: the same problems as normal DMA plus reading from
            // VRAM should copy garbage
            let val = self.read_mem(self.hdma.src);
            self.ppu.write_vram(self.hdma.dst, val);
            self.hdma.dst += 1;
            self.hdma.src += 1;
        }

        // can be outside of loop because HDMA should not
        // access IO range (clk registers, ifr,
        // etc..). If the PPU reads VRAM during an HDMA transfer it
        // should be glitchy anyways
        // FIXME: timings
        if self.key1.is_enabled() {
            self.advance_dots(i32::from(len) * 2 * 2);
        } else {
            self.advance_dots(i32::from(len) * 2);
        }
    }
}
