use crate::video::{Ppu, PpuMode};

#[derive(Clone, Copy, PartialEq, Eq)]
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

const HDMA_T_CYCLES_DELAY: i8 = 12;

pub struct HDMATransfer {
    pub source_address: u16,
    pub destination_address: u16,
    pub length: u16,
}

#[derive(PartialEq, Eq)]
enum HdmaState {
    AwaitingHBlank,
    Copying,
    FinishedLine,
}

pub struct Hdma {
    is_active: bool,
    source: u16,
    destination: u16,
    mode: HdmaMode,
    transfer_size: u16,
    state: HdmaState,
    bytes_to_copy: u8,
    microseconds_elapsed_times_16: i8,
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
            microseconds_elapsed_times_16: 0,
        }
    }

    pub fn write_hdma1(&mut self, val: u8) {
        self.source = (self.source & 0xf0) | (u16::from(val) << 8);
    }

    pub fn write_hdma2(&mut self, val: u8) {
        self.source = (self.source & 0xff00) | (u16::from(val) & 0xf0);
    }

    pub fn write_hdma3(&mut self, val: u8) {
        self.destination = (self.destination & 0xf0) | (u16::from(val) << 8);
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
        if val & 0x80 == 0 {
            self.is_active = false;
            return;
        }

        self.mode = val.into();
        let transfer_blocks = val & 0b0111_1111;
        self.transfer_size = (u16::from(transfer_blocks) + 1) * 0x10;
        self.state = HdmaState::AwaitingHBlank;
        self.microseconds_elapsed_times_16 = -HDMA_T_CYCLES_DELAY;
        self.is_active = true;
    }

    #[allow(clippy::cast_possible_wrap)]
    pub fn emulate(
        &mut self,
        ppu: &Ppu,
        microseconds_elapsed_times_16: u8,
    ) -> Option<HDMATransfer> {
        if !self.is_active || !ppu.is_enabled() {
            return None;
        }

        self.microseconds_elapsed_times_16 = self
            .microseconds_elapsed_times_16
            .wrapping_add(microseconds_elapsed_times_16 as i8); // 2 or 4 so its safe

        if self.microseconds_elapsed_times_16 >= 4 {
            self.microseconds_elapsed_times_16 -= 4;

            if self.mode == HdmaMode::GeneralPurpose {
                let hdma_transfer = HDMATransfer {
                    source_address: self.source,
                    destination_address: self.destination,
                    length: self.transfer_size,
                };

                self.transfer_size = 0;
                self.is_active = false;

                return Some(hdma_transfer);
            }
            // else
            match self.state {
                HdmaState::FinishedLine if ppu.mode() != PpuMode::HBlank => {
                    self.state = HdmaState::AwaitingHBlank;
                }
                HdmaState::AwaitingHBlank if ppu.mode() == PpuMode::HBlank => {
                    self.state = HdmaState::Copying;
                    self.bytes_to_copy = 0x10;
                }
                _ => (),
            }

            // according to
            // https://gbdev.io/pandocs/CGB_Registers.html?highlight=speed%20switch#ff4d---key1---cgb-mode-only---prepare-speed-switch
            // should be 2 but games break ¯\_(ツ)_/¯
            const BYTES_TRANSFERED_PER_M_CYCLE: u8 = 1;

            if self.state == HdmaState::Copying {
                let hdma_transfer = HDMATransfer {
                    source_address: self.source,
                    destination_address: self.destination,
                    length: u16::from(BYTES_TRANSFERED_PER_M_CYCLE),
                };

                self.destination = self
                    .destination
                    .wrapping_add(u16::from(BYTES_TRANSFERED_PER_M_CYCLE));
                self.source = self
                    .source
                    .wrapping_add(u16::from(BYTES_TRANSFERED_PER_M_CYCLE));
                self.transfer_size -= u16::from(BYTES_TRANSFERED_PER_M_CYCLE);
                self.bytes_to_copy -= BYTES_TRANSFERED_PER_M_CYCLE;

                if self.bytes_to_copy == 0 {
                    self.state = HdmaState::FinishedLine;
                }

                if self.transfer_size == 0 {
                    self.is_active = false;
                }

                Some(hdma_transfer)
            } else {
                None
            }
        } else {
            None
        }
    }
}
