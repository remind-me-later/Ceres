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

const HDMA_T_CYCLES_DELAY: i8 = 0;

pub struct VramDMATransfer {
    pub source_address: u16,
    pub destination_address: u16,
    pub length: u16,
}

#[derive(PartialEq, Eq)]
enum VramDmaState {
    AwaitingHBlank,
    Copying,
    FinishedLine,
}

pub struct VramDma {
    is_active: bool,
    source: u16,
    destination: u16,
    mode: VramDmaMode,
    transfer_size: u16,
    state: VramDmaState,
    bytes_to_copy: u8,
    microseconds_elapsed_times_16: i8,
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
            log::warn!("cancel hdma");
            self.is_active = false;
            return;
        }

        self.mode = val.into();
        let transfer_blocks = val & 0x7f;
        self.transfer_size = (u16::from(transfer_blocks) + 1) * 0x10;
        self.state = VramDmaState::AwaitingHBlank;
        self.microseconds_elapsed_times_16 = -HDMA_T_CYCLES_DELAY;
        self.is_active = true;
    }

    #[allow(clippy::cast_possible_wrap)]
    pub fn emulate(
        &mut self,
        ppu: &Ppu,
        microseconds_elapsed_times_16: u8,
    ) -> Option<VramDMATransfer> {
        if !self.is_active {
            return None;
        }

        self.microseconds_elapsed_times_16 = self
            .microseconds_elapsed_times_16
            .wrapping_add(microseconds_elapsed_times_16 as i8); // 2 or 4 so its safe

        if self.microseconds_elapsed_times_16 < 4 {
            return None;
        }

        self.microseconds_elapsed_times_16 -= 4;

        match self.mode {
            VramDmaMode::GeneralPurpose => {
                // log::info!(
                //     "GHDMA source: {:x}, dest: {:x}, len: {:x}",
                //     self.source,
                //     self.destination,
                //     self.transfer_size
                // );

                let hdma_transfer = VramDMATransfer {
                    source_address: self.source,
                    destination_address: self.destination,
                    length: self.transfer_size,
                };

                self.transfer_size = 0;
                self.is_active = false;

                Some(hdma_transfer)
            }
            VramDmaMode::Hblank => {
                match self.state {
                    VramDmaState::FinishedLine if ppu.mode() != Mode::HBlank => {
                        self.state = VramDmaState::AwaitingHBlank;
                    }
                    VramDmaState::AwaitingHBlank if ppu.mode() == Mode::HBlank => {
                        self.state = VramDmaState::Copying;
                        self.bytes_to_copy = 0x10;
                    }
                    _ => (),
                }

                const BYTES_TRANSFERED_PER_M_CYCLE: u8 = 1;

                if self.state == VramDmaState::Copying {
                    let hdma_transfer = VramDMATransfer {
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
                        self.state = VramDmaState::FinishedLine;
                    }

                    if self.transfer_size == 0 {
                        self.is_active = false;
                    }

                    Some(hdma_transfer)
                } else {
                    None
                }
            }
        }
    }
}
