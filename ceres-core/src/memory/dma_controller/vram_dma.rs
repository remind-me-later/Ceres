use crate::video::ppu::{Mode, Ppu};

#[derive(Clone, Copy, PartialEq, Eq)]
enum VramDmaMode {
    GeneralPurpose,
    Hblank,
}

impl From<u8> for VramDmaMode {
    fn from(val: u8) -> Self {
        match val >> 7 {
            0 => Self::GeneralPurpose,
            _ => Self::Hblank,
        }
    }
}

pub struct VramDMATransfer {
    pub source_address: u16,
    pub destination_address: u16,
}

#[derive(PartialEq, Eq)]
enum VramDmaState {
    AwaitingHBlank,
    FinishedLine,
}

pub struct VramDma {
    is_active: bool,
    source: u16,
    destination: u16,
    mode: VramDmaMode,
    transfer_size: u16,
    state: VramDmaState,
    bytes_to_copy: u16,
}

impl VramDma {
    pub const fn new() -> Self {
        Self {
            is_active: false,
            source: 0,
            destination: 0,
            mode: VramDmaMode::GeneralPurpose,
            transfer_size: 0,
            state: VramDmaState::AwaitingHBlank,
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
        self.state = VramDmaState::AwaitingHBlank;
        self.is_active = true;
    }

    pub fn start_transfer(&mut self, ppu: &Ppu) -> bool {
        if !self.is_active {
            return false;
        }

        match self.mode {
            VramDmaMode::GeneralPurpose => {
                self.bytes_to_copy = self.transfer_size;
                true
            }
            VramDmaMode::Hblank => match self.state {
                VramDmaState::FinishedLine if ppu.mode() != Mode::HBlank => {
                    self.state = VramDmaState::AwaitingHBlank;
                    false
                }
                VramDmaState::AwaitingHBlank if ppu.mode() == Mode::HBlank => {
                    self.bytes_to_copy = 0x10;
                    true
                }
                _ => false,
            },
        }
    }

    pub fn is_transfer_done(&self) -> bool {
        !self.is_active || self.state == VramDmaState::FinishedLine
    }

    pub fn do_vram_transfer(&mut self) -> VramDMATransfer {
        let hdma_transfer = VramDMATransfer {
            source_address: self.source,
            destination_address: self.destination,
        };

        self.destination = self.destination.wrapping_add(1);
        self.source = self.source.wrapping_add(1);
        self.transfer_size -= 1;
        self.bytes_to_copy -= 1;

        if self.bytes_to_copy == 0 {
            self.state = VramDmaState::FinishedLine;
        }

        if self.transfer_size == 0 {
            self.is_active = false;
        }

        hdma_transfer
    }
}
