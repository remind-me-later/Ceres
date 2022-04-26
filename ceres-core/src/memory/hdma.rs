use crate::video::ppu::{Mode, Ppu};

#[derive(PartialEq, Eq)]
enum HdmaMode {
    GeneralPurpose,
    Hblank,
}

impl From<u8> for HdmaMode {
    fn from(val: u8) -> Self {
        match val >> 7 {
            0 => Self::GeneralPurpose,
            _ => Self::Hblank,
        }
    }
}

pub struct HdmaTransfer {
    pub src: u16,
    pub dst: u16,
}

#[derive(PartialEq, Eq)]
enum HdmaState {
    AwaitingHBlank,
    FinishedLine,
}

pub struct Hdma {
    is_active: bool,
    source: u16,
    destination: u16,
    mode: HdmaMode,
    transfer_size: u16,
    state: HdmaState,
    bytes_to_copy: u16,
}

impl Hdma {
    pub const fn new() -> Self {
        Self {
            is_active: false,
            source: 0,
            destination: 0,
            mode: HdmaMode::GeneralPurpose,
            transfer_size: 0,
            state: HdmaState::AwaitingHBlank,
            bytes_to_copy: 0,
        }
    }

    pub fn write_hdma1(&mut self, val: u8) {
        self.source = (self.source & 0xf0) | (u16::from(val) << 8);
    }

    pub fn write_hdma2(&mut self, val: u8) {
        self.source = (self.source & 0xff00) | (u16::from(val) & 0xf0);
    }

    pub fn write_hdma3(&mut self, val: u8) {
        self.destination = (self.destination & 0xf0) | (u16::from(val & 0x1f) << 8);
    }

    pub fn write_hdma4(&mut self, val: u8) {
        self.destination = (self.destination & 0x1f00) | (u16::from(val) & 0xf0);
    }

    pub fn read_hdma5(&self) -> u8 {
        let is_active_bit = u8::from(self.is_active) << 7;
        let blocks_bits = ((self.transfer_size / 0x10).wrapping_sub(1)) as u8;
        is_active_bit | blocks_bits
    }

    pub fn write_hdma5(&mut self, val: u8) {
        // stop current transfer
        if self.is_active && val & 0x80 == 0 {
            self.is_active = false;
            return;
        }

        self.mode = val.into();
        let transfer_blocks = val & 0x7f;
        self.transfer_size = (u16::from(transfer_blocks) + 1) * 0x10;
        self.state = HdmaState::AwaitingHBlank;
        self.is_active = true;
    }

    pub fn start(&mut self, ppu: &Ppu) -> bool {
        if !self.is_active {
            return false;
        }

        match self.mode {
            HdmaMode::GeneralPurpose => {
                self.bytes_to_copy = self.transfer_size;
                true
            }
            HdmaMode::Hblank => match self.state {
                HdmaState::FinishedLine if ppu.mode() != Mode::HBlank => {
                    self.state = HdmaState::AwaitingHBlank;
                    false
                }
                HdmaState::AwaitingHBlank if ppu.mode() == Mode::HBlank => {
                    self.bytes_to_copy = 0x10;
                    true
                }
                _ => false,
            },
        }
    }

    pub fn is_transfer_done(&self) -> bool {
        !self.is_active || self.state == HdmaState::FinishedLine
    }

    pub fn transfer(&mut self) -> HdmaTransfer {
        let hdma_transfer = HdmaTransfer {
            src: self.source,
            dst: self.destination,
        };

        self.destination = self.destination.wrapping_add(1);
        self.source = self.source.wrapping_add(1);
        self.transfer_size -= 1;
        self.bytes_to_copy -= 1;

        if self.bytes_to_copy == 0 {
            self.state = HdmaState::FinishedLine;
        }

        if self.transfer_size == 0 {
            self.is_active = false;
        }

        hdma_transfer
    }
}
