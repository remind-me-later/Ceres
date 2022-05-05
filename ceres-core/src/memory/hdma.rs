use self::State::*;
use crate::video::ppu::Mode::HBlank;
use crate::video::ppu::Ppu;

enum State {
    Inactive,
    General,
    CopyingHBlank { bytes_left: u8 },
    AwaitHBlank,
    DoneHBlank,
}

impl Default for State {
    fn default() -> Self {
        Inactive
    }
}

pub struct HdmaTransfer {
    pub src: u16,
    pub dst: u16,
}

#[derive(Default)]
pub struct Hdma {
    src: u16,
    dst: u16,
    len: u16,
    state: State,
}

impl Hdma {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_hdma1(&mut self, val: u8) {
        self.src = u16::from(val) << 8 | self.src & 0xf0;
    }

    pub fn write_hdma2(&mut self, val: u8) {
        self.src = self.src & 0xff00 | u16::from(val) & 0xf0;
    }

    pub fn write_hdma3(&mut self, val: u8) {
        self.dst = u16::from(val & 0x1f) << 8 | self.dst & 0xf0;
    }

    pub fn write_hdma4(&mut self, val: u8) {
        self.dst = self.dst & 0x1f00 | u16::from(val) & 0xf0;
    }

    pub fn read_hdma5(&self) -> u8 {
        let is_active_bit = u8::from(self.is_active()) << 7;
        let blocks_bits = (self.len / 0x10).wrapping_sub(1) as u8;
        is_active_bit | blocks_bits
    }

    fn is_active(&self) -> bool {
        !matches!(self.state, Inactive)
    }

    pub fn write_hdma5(&mut self, val: u8) {
        // stop current transfer
        // TODO: reload
        if self.is_active() && val & 0x80 == 0 {
            self.state = Inactive;
            return;
        }

        self.state = match (val >> 7) & 1 {
            0 => General,
            1 => AwaitHBlank,
            _ => unreachable!(),
        };

        let transfer_blocks = val & 0x7f;
        self.len = (u16::from(transfer_blocks) + 1) * 0x10;
    }

    pub fn start(&mut self, ppu: &Ppu) -> bool {
        match self.state {
            Inactive => false,
            General => true,
            DoneHBlank if ppu.mode() != HBlank => {
                self.state = AwaitHBlank;
                false
            }
            AwaitHBlank if ppu.mode() == HBlank => {
                self.state = CopyingHBlank { bytes_left: 0x10 };
                true
            }
            _ => unreachable!(),
        }
    }

    pub fn is_transfer_done(&self) -> bool {
        matches!(self.state, Inactive | DoneHBlank)
    }

    pub fn transfer(&mut self) -> HdmaTransfer {
        let hdma_transfer = HdmaTransfer {
            src: self.src,
            dst: self.dst,
        };

        self.dst += 1;
        self.src += 1;
        self.len -= 1;

        if self.len == 0 {
            self.state = Inactive;
        } else if let CopyingHBlank { mut bytes_left } = self.state {
            bytes_left -= 1;

            if bytes_left == 0 {
                self.state = DoneHBlank;
            }
        }

        hdma_transfer
    }
}
