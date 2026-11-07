//! The netlist's LCDC and the PPU's reset.
//!
//! Transcribed from `hdl/rtl/lcd_control.vhd`, for `NetChip` and for the
//! replay of the model's traces (`tests/replay.rs`), which checks the
//! transcription.
//!
//! LCDC (FF40) is a set of latches, open while the CPU writes it, and
//! cleared by the chip's reset (`reset2`). They open a moment after the
//! write strobe, so the flip-flops clocked at the edge that starts the
//! write's first phase still take the old value. Bit 7 clear holds the PPU
//! in reset (`ppu_reset`), as `reset2` does; bit 2 is the sprites' height,
//! which the OAM scan takes.

/// LCDC's bits that other blocks take: the LCD's enable, the sprites'
/// height.
const LCDC_ON_B: u8 = 0x80;
const LCDC_TALL_B: u8 = 0x04;

/// The netlist's LCDC.
#[derive(Clone, Copy)]
pub struct NetLcdControl {
    /// LCDC's latches.
    pub lcdc: u8,
}

impl NetLcdControl {
    /// LCDC at power-on.
    pub const POWER_ON: Self = Self { lcdc: 0 };

    /// A phase: the chip's reset clears the latches; else the CPU's write
    /// of FF40, if any, opens them.
    // @hw die: VYXE XYLO XYMO XAFO WEXU WYMO WOKY XONA (drlatch_ee), WARU
    // XUBO XURE, XARE
    pub const fn settle(&mut self, reset2: bool, write: Option<u8>) {
        if reset2 {
            self.lcdc = 0;
        } else if let Some(data) = write {
            self.lcdc = data;
        } else {
            // The latches hold.
        }
    }

    /// The PPU's reset: the LCD is off, or the chip is reset.
    // @hw die: XODO XAPO PYRY, LYHA LYFE
    #[must_use]
    pub const fn ppu_reset(&self, reset2: bool) -> bool {
        (self.lcdc & LCDC_ON_B == 0) | reset2
    }

    /// LCDC bit 2: sprites of 16 lines.
    #[must_use]
    pub const fn tall(&self) -> bool {
        self.lcdc & LCDC_TALL_B != 0
    }

    /// What a read of `addr` drives: LCDC, at FF40.
    // @hw die: VYRE WYCE, WYPO XERO WYJU WUKA VOKE VATO VAHA XEBU (not_if0)
    #[must_use]
    pub const fn read(&self, addr: u16) -> Option<u8> {
        if addr == 0xFF40 {
            Some(self.lcdc)
        } else {
            None
        }
    }
}
