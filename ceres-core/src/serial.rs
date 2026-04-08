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
        self.sc = if is_cgb { val | 0x7C } else { val | 0x7E };

        self.div_mask = if is_cgb && (val & CGB_SPEED != 0) {
            4
        } else {
            0x80
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CgbMode;

    #[test]
    fn test_serial_transfer_timing() {
        let mut serial = Serial::default();
        let mut ints = Interrupts::default();

        // Setup: SB = 0xAA, SC = 0x81 (Start, Master, Normal Speed)
        serial.write_sb(0xAA);
        serial.write_sc(0x81, &mut ints, CgbMode::Dmg);

        // Serial edge triggers on bit 7 falling edge of 4MHz clock (fires every 256 cycles).
        // 1st edge: Master Clock False -> True (No shift)
        // 2nd edge: Master Clock True -> False (Shift 1)
        // ...
        // 16th edge: Master Clock True -> False (Shift 8, Transfer complete)

        // Total cycles for 16 edges: 16 * 256 = 4096 cycles.
        // But if we just reset DIV, the first edge happens at cycle 128 (Rising) or 256 (Falling).
        // Bit 7 goes 0->1 at cycle 128, 1->0 at cycle 256.

        let mut div: u16 = 0;
        let mut cycles = 0;

        let fire_edge =
            |serial: &mut Serial, ints: &mut Interrupts, div: &mut u16, cycles: &mut u32| {
                let old_div = *div;
                *div = div.wrapping_add(1);
                *cycles += 1;

                let triggers = old_div & !*div;
                if triggers & u16::from(serial.div_mask()) != 0 {
                    serial.run_master(ints);
                    true
                } else {
                    false
                }
            };

        // Run until transfer complete
        let mut edges = 0;
        while serial.read_sc(CgbMode::Dmg) & 0x80 != 0 {
            if fire_edge(&mut serial, &mut ints, &mut div, &mut cycles) {
                edges += 1;
            }
            if cycles > 10000 {
                panic!("Transfer timed out");
            }
        }

        assert_eq!(edges, 16, "Transfer should take exactly 16 edges");
        assert_eq!(cycles, 16 * 256, "Transfer should take exactly 4096 cycles");
        assert_eq!(
            serial.read_sb(),
            0xFF,
            "SB should be all 1s after shifting in from no-device"
        );
        assert!(
            ints.read_if() & 0x08 != 0,
            "Serial interrupt should be requested"
        );
    }

    #[test]
    fn test_serial_zombie_clocking() {
        let mut serial = Serial::default();
        let mut ints = Interrupts::default();

        // 1. Manually toggle master clock high by "fake" edges
        serial.run_master(&mut ints);
        assert!(serial.master_clock);

        // 2. Write to SC while master clock is high.
        // This should trigger an edge using the OLD SC value.
        // If old SC didn't have START set, no shift happens but master_clock toggles.
        serial.write_sc(0x81, &mut ints, CgbMode::Dmg);
        assert!(!serial.master_clock);
        assert_eq!(serial.count, 0); // No shift because START was not set in old SC

        // 3. Now master clock is low. Write SC again.
        // It shouldn't trigger an edge because master clock is low.
        serial.write_sc(0x81, &mut ints, CgbMode::Dmg);
        assert!(!serial.master_clock);
    }
}
