use crate::{AudioCallback, Gb, Model};

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "State enum variants are logically ordered"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum DmaState {
    #[default]
    Inactive,
    Starting(u8),        // Startup delay (dots), OAM accessible
    StartingBlocked(u8), // Startup delay (dots), OAM blocked
    Transferring(u8),    // Current offset (0-159)
    Finishing,           // Extra cycle after transfer
}

#[derive(Default)]
pub struct Dma {
    accumulator: u8,
    base_addr: u16,
    reg: u8,
    state: DmaState,
}

impl Dma {
    pub const fn advance_dots(&mut self, dots: i32) {
        if !matches!(self.state, DmaState::Inactive) {
            #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            {
                self.accumulator = self.accumulator.wrapping_add(dots as u8);
            }
        }
    }

    pub const fn blocks_oam(&self) -> bool {
        match self.state {
            DmaState::Inactive | DmaState::Starting(_) => false,
            DmaState::StartingBlocked(_) | DmaState::Transferring(_) | DmaState::Finishing => true,
        }
    }

    pub const fn read(&self) -> u8 {
        self.reg
    }

    // Returns Some((src_addr, dst_offset)) if a byte should be transferred.
    // Should be called in a loop until it returns None.
    pub fn step(&mut self) -> Option<(u16, u8)> {
        while self.accumulator >= 4 {
            self.accumulator -= 4;

            match self.state {
                DmaState::Inactive => return None,
                DmaState::Starting(dots) => {
                    if dots <= 4 {
                        self.state = DmaState::Transferring(1);
                        return Some((self.base_addr, 0));
                    }
                    self.state = DmaState::Starting(dots - 4);
                }
                DmaState::StartingBlocked(dots) => {
                    if dots <= 4 {
                        self.state = DmaState::Transferring(1);
                        return Some((self.base_addr, 0));
                    }
                    self.state = DmaState::StartingBlocked(dots - 4);
                }
                DmaState::Transferring(offset) => {
                    let src = self.base_addr.wrapping_add(u16::from(offset));
                    let dst = offset;

                    if offset == 159 {
                        self.state = DmaState::Finishing;
                    } else {
                        self.state = DmaState::Transferring(offset + 1);
                    }

                    return Some((src, dst));
                }
                DmaState::Finishing => {
                    self.state = DmaState::Inactive;
                }
            }
        }
        None
    }

    pub fn write(&mut self, val: u8) {
        self.reg = val;
        self.base_addr = u16::from(val) << 8;
        // Startup delay: 1 M-cycle = 4 dots
        // The write instruction itself takes 1 M-cycle (4 dots), which is "lost"
        // because tick_m_cycle() runs before write_mem().
        // So we only need to wait 1 more M-cycle to reach the total 2 M-cycle delay.
        //
        // If a DMA transfer is already active (restarting), OAM is already blocked,
        // so we use StartingBlocked to keep it blocked during the delay.
        if matches!(self.state, DmaState::Inactive) {
            self.state = DmaState::Starting(8);
        } else {
            self.state = DmaState::StartingBlocked(8);
        }
        self.accumulator = 0;
    }
}

impl<A: AudioCallback> Gb<A> {
    #[inline]
    pub fn run_dma(&mut self) {
        while let Some((src, dst_offset)) = self.dma.step() {
            // HDMA/DMA conflict: Skip DMA transfer if HDMA is in progress
            // (except for the last byte of an HDMA block)
            // Aligned with SameBoy line 1870-1871
            if self.hdma.is_transferring()
                && (self.hdma.has_multiple_steps_left() || !self.hdma.is_at_block_end())
            {
                continue;
            }

            // Source address handling aligned with SameBoy lines 1873-1883
            let val = if src < 0xE000 {
                // Normal read from ROM, RAM, or external RAM
                self.read_mem(src)
            } else {
                // Reading from 0xE000-0xFFFF during DMA
                match self.model {
                    Model::Cgb0
                    | Model::CgbA
                    | Model::CgbB
                    | Model::CgbC
                    | Model::CgbD
                    | Model::CgbE => {
                        // CGB: Invalid source, reads 0xFF
                        0xFF
                    }
                    Model::DmgB | Model::Mgb => {
                        // DMG/MGB: Mirrors 0xC000-0xDFFF (mask with 0xDFFF = ~0x2000)
                        self.read_mem(src & 0xDFFF)
                    }
                }
            };

            // TODO: writes from DMA can access OAM on modes 2 and 3
            // with some glitches (RESEARCH) and without trouble during
            // VBLANK (what happens in HBLANK?)
            self.ppu
                .write_oam_by_dma(u16::from(dst_offset) | 0xFE00, val);
        }
    }
}
