use crate::ppu::{Mode, Ppu};

pub struct Oam {
    bytes: [u8; Self::SIZE as usize],
}

impl Default for Oam {
    fn default() -> Self {
        Self {
            bytes: [0; Self::SIZE as usize],
        }
    }
}

impl Oam {
    pub const SIZE: u8 = 0xA0;

    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub const fn read(&self, addr: u16) -> u8 {
        self.bytes[(addr & 0xFF) as usize]
    }

    pub const fn write(&mut self, addr: u16, val: u8) {
        self.bytes[(addr & 0xFF) as usize] = val;
    }
}

impl Ppu {
    #[must_use]
    pub const fn oam(&self) -> &Oam {
        &self.oam
    }

    #[must_use]
    pub const fn oam_mut(&mut self) -> &mut Oam {
        &mut self.oam
    }

    // TODO: why does read check for enabled DMA transfer and write for active DMA?
    #[must_use]
    pub const fn read_oam(&self, addr: u16, dma_on: bool) -> u8 {
        if dma_on {
            return 0xFF;
        }

        match self.mode() {
            Mode::HBlank | Mode::VBlank => self.oam.read(addr),
            Mode::OamScan => {
                // OAM read is blocked after the first M-cycle (4 dots) of Mode 2
                // Mode 2 is 80 dots long.
                // If remaining > 76, we are in the first 4 dots.
                if self.remaining_dots_in_mode > 76 {
                    self.oam.read(addr)
                } else {
                    0xFF
                }
            }
            Mode::Drawing => 0xFF,
        }
    }

    pub fn write_oam(&mut self, addr: u16, val: u8, dma_active: bool) {
        let mode = self.mode();
        let blocked = if dma_active {
            true
        } else {
            match mode {
                Mode::HBlank | Mode::VBlank => false,
                Mode::OamScan => {
                    // OAM write is blocked after the first 2 M-cycles (8 dots) of Mode 2
                    // Mode 2 is 80 dots long.
                    // If remaining <= 72, we are in the first 8 dots.
                    self.remaining_dots_in_mode <= 72
                }
                Mode::Drawing => true,
            }
        };

        tracing::trace!(
            target: "oam",
            addr = addr,
            value = val,
            dma_active = dma_active,
            ppu_mode = ?mode,
            blocked = blocked,
            "OAM Write"
        );

        if !blocked {
            self.oam.write(addr, val);
        }
    }

    pub const fn write_oam_by_dma(&mut self, addr: u16, val: u8) {
        // self.oam[(addr & 0xFF) as usize] = val;
        self.oam.write(addr, val);
    }
}
