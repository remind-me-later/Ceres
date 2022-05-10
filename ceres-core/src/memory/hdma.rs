use crate::video::ppu::{Mode::HBlank, Ppu};

pub struct HdmaTransfer {
    pub src: u16,
    pub dst: u16,
}

enum State {
    Inactive,
    HBlankAwait,
    HBlankCopy { bytes: u8 },
    HBlankDone,
    General,
}

impl Default for State {
    fn default() -> Self {
        Self::Inactive
    }
}

#[derive(Default)]
pub struct Hdma {
    src: u16,
    dst: u16,
    len: u16,
    state: State,
    hdma5: u8, // stores only low 7 bits
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

    fn is_active(&self) -> bool {
        !matches!(self.state, State::Inactive)
    }

    pub fn read_hdma5(&self) -> u8 {
        // active on low
        let is_active_bit = u8::from(!self.is_active()) << 7;
        is_active_bit | self.hdma5
    }

    pub fn write_hdma5(&mut self, val: u8) {
        // stop current transfer
        if self.is_active() && val & 0x80 == 0 {
            self.state = State::Inactive;
            return;
        }

        self.hdma5 = val & !0x80;
        let transfer_blocks = val & 0x7f;
        self.len = (u16::from(transfer_blocks) + 1) * 0x10;
        self.state = match val >> 7 {
            0 => State::General,
            1 => State::HBlankAwait,
            _ => unreachable!(),
        };
    }

    pub fn start(&mut self, ppu: &Ppu) -> bool {
        match self.state {
            State::General => true,
            State::HBlankDone if ppu.mode() != HBlank => {
                self.state = State::HBlankAwait;
                false
            }
            State::HBlankAwait if ppu.mode() == HBlank => {
                self.state = State::HBlankCopy { bytes: 0x10 };
                true
            }
            State::HBlankCopy { .. } => unreachable!(),
            _ => false,
        }
    }

    pub fn is_transfer_done(&self) -> bool {
        matches!(self.state, State::Inactive | State::HBlankDone)
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
            self.state = State::Inactive;
            self.hdma5 = 0xff;
        } else if let State::HBlankCopy { mut bytes } = self.state {
            bytes -= 1;

            if bytes == 0 {
                self.state = State::HBlankDone;
                self.hdma5 = (self.len / 0x10).wrapping_sub(1) as u8;
            } else {
                self.state = State::HBlankCopy { bytes };
            }
        }

        hdma_transfer
    }
}
