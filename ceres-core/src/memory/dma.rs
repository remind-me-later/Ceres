use crate::{AudioCallback, Gb};

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "State enum variants are logically ordered"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum DmaState {
    #[default]
    Inactive,
    Starting(u8),     // Startup delay (dots)
    Transferring(u8), // Current offset (0-159)
    Finishing,        // Extra cycle after transfer
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

    pub const fn is_enabled(&self) -> bool {
        !matches!(self.state, DmaState::Inactive)
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
        // Startup delay: 2 M-cycles = 8 dots
        self.state = DmaState::Starting(8);
        self.accumulator = 0;

        tracing::trace!(
            target: "dma",
            src_base = format!("${:04X}", self.base_addr),
            delay_dots = 8,
            "OAM DMA started"
        );
    }
}

impl<A: AudioCallback> Gb<A> {
    #[inline]
    pub fn run_dma(&mut self) {
        while let Some((src, dst_offset)) = self.dma.step() {
            // TODO: reading some ranges should cause problems, $DF is
            // the maximum value accesible to OAM DMA (probably reads
            // from echo RAM should work too, RESEARCH).
            // what happens if reading from IO range? (garbage? 0xff?)
            let val = self.read_mem(src);

            // TODO: writes from DMA can access OAM on modes 2 and 3
            // with some glitches (RESEARCH) and without trouble during
            // VBLANK (what happens in HBLANK?)
            self.ppu
                .write_oam_by_dma(u16::from(dst_offset) | 0xFE00, val);

            tracing::trace!(
                target: "dma",
                src = format!("${:04X}", src),
                oam_offset = format!("${:02X}", dst_offset),
                val = format!("${:02X}", val),
                "OAM DMA transfer byte"
            );
        }
    }
}
