use crate::{CgbMode, interrupts::Interrupts};
use alloc::string::String;

const START: u8 = 0x80;
const CGB_SPEED: u8 = 0x2;
const SHIFT: u8 = 0x1;

// VERY PARTIAL Serial port implementation with output capture for test ROMs
#[derive(Default)]
pub struct Serial {
    count: u8,
    div_mask: u8,
    master_clock: bool,
    output: String,
    sb: u8,
    sb_sent: u8, // Store the original byte being sent
    sc: u8,
}

impl Serial {
    #[must_use]
    pub const fn div_mask(&self) -> u8 {
        self.div_mask
    }

    /// Get the serial output as a string (used by test ROMs)
    #[must_use]
    pub fn output(&self) -> &str {
        &self.output
    }

    #[must_use]
    pub const fn read_sb(&self) -> u8 {
        self.sb
    }

    #[must_use]
    pub const fn read_sc(&self, _cgb_mode: CgbMode) -> u8 {
        self.sc
    }

    pub fn run_master(&mut self, ints: &mut Interrupts) {
        self.master_clock ^= true;

        if !self.master_clock && (self.sc & START != 0) && (self.sc & SHIFT != 0) {
            self.count += 1;

            self.sb <<= 1;
            // When no device is connected, the input bit reads as 1
            self.sb |= 1;

            if self.count == 8 {
                // Transfer complete - capture the ORIGINAL byte that was sent
                let transferred_byte = self.sb_sent;

                self.count = 0;
                ints.request_serial();
                self.sc &= !START;

                // Capture the byte that was just transferred
                // Test ROMs like Blargg's tests output via serial
                if (0x20..0x7F).contains(&transferred_byte) {
                    // Printable ASCII character
                    self.output.push(transferred_byte as char);
                } else if transferred_byte == b'\n' {
                    self.output.push('\n');
                } else if transferred_byte == b'\r' {
                    self.output.push('\r');
                }
            }
        }
    }

    pub const fn write_sb(&mut self, val: u8) {
        self.sb = val;
        self.sb_sent = val; // Store original value for later capture
    }

    pub fn write_sc(&mut self, mut val: u8, ints: &mut Interrupts, cgb_mode: CgbMode) {
        self.count = 0;

        let is_cgb = !matches!(cgb_mode, CgbMode::Dmg);

        if !is_cgb {
            val |= CGB_SPEED;
        }

        // Writing to SC while master clock is high triggers a clock edge (zombie clocking)
        // This edge uses the OLD value of SC.
        if self.master_clock {
            self.run_master(ints);
        }

        // Bits 6-2 are always 1. Bit 1 is also always 1 on DMG.
        self.sc = if is_cgb {
            val | 0x7C
        } else {
            val | 0x7E
        };

        self.div_mask = if is_cgb && (val & CGB_SPEED != 0) {
            4
        } else {
            0x80
        };
    }
}
