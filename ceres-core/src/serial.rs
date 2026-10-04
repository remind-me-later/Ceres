use crate::{CgbMode, interrupts::Interrupts};
use alloc::string::String;

const START: u8 = 0x80;
const CGB_SPEED: u8 = 0x2;
const SHIFT: u8 = 0x1;

// VERY PARTIAL Serial port implementation with output capture for test ROMs
pub struct Serial {
    count: u8,
    div_mask: u16,
    master_clock: bool,
    output: String,
    sb: u8,
    sb_sent: u8, // Store the original byte being sent
    sc: u8,
}

impl Default for Serial {
    fn default() -> Self {
        Self {
            count: 0,
            div_mask: 0x80,
            master_clock: false,
            output: String::new(),
            sb: 0,
            sb_sent: 0,
            sc: 0x7E,
        }
    }
}

impl Serial {
    /// Sets the master clock flip-flop from the system counter (it toggles on
    /// every falling edge of bit 7, so it follows bit 8 until DIV is written).
    pub fn set_master_clock(&mut self, val: bool) {
        self.master_clock = val;
    }

    /// SC after the boot ROM: bit 1 (the CGB clock speed) reads 1 on a CGB.
    pub const fn set_post_boot(&mut self, cgb: bool) {
        self.sc = if cgb { 0x7F } else { 0x7E };
    }

    /// The system counter bit whose falling edges clock the serial port.
    #[must_use]
    pub const fn div_mask(&self) -> u16 {
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
    pub const fn read_sc(&self) -> u8 {
        self.sc
    }

    /// A falling edge of the selected system counter bit: toggles the master
    /// clock and shifts a bit on every second edge (port of SameBoy's
    /// `GB_serial_master_edge`).
    pub fn master_edge(&mut self, ints: &mut Interrupts) {
        self.master_clock = !self.master_clock;

        if !self.master_clock && self.sc & (START | SHIFT) == START | SHIFT {
            self.count += 1;
            if self.count == 8 {
                self.count = 0;
                self.sc &= !START;
                ints.request_serial();

                // Capture the byte that was just transferred
                // (test ROMs like Blargg's print through serial).
                let transferred_byte = self.sb_sent;
                if (0x20..0x7F).contains(&transferred_byte) {
                    self.output.push(transferred_byte as char);
                } else if transferred_byte == b'\n' {
                    self.output.push('\n');
                } else if transferred_byte == b'\r' {
                    self.output.push('\r');
                }
            }

            self.sb <<= 1;
            // When no device is connected, the input bit reads as 1.
            self.sb |= 1;
        }
    }

    pub const fn write_sb(&mut self, val: u8) {
        self.sb = val;
        self.sb_sent = val; // Store original value for later capture
    }

    pub fn write_sc(&mut self, mut val: u8, ints: &mut Interrupts, cgb_mode: CgbMode) {
        self.count = 0;

        let cgb = matches!(cgb_mode, CgbMode::Cgb);
        if !cgb {
            val |= CGB_SPEED;
        }

        // Writing SC while the master clock is high clocks the port once
        // more, with the old SC value.
        if self.master_clock {
            self.master_edge(ints);
        }

        // Bits 6-2 and bit 1 (unless CGB) always read 1.
        self.sc = val | 0x7C;
        self.div_mask = if cgb && val & CGB_SPEED != 0 {
            0x04
        } else {
            0x80
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CgbMode;

    /// Feeds the falling edges of `div_mask` for `cycles` T-cycles.
    fn run(serial: &mut Serial, ints: &mut Interrupts, div: &mut u16, cycles: u32) {
        for _ in 0..cycles {
            let old = *div;
            *div = div.wrapping_add(1);
            if old & !*div & serial.div_mask() != 0 {
                serial.master_edge(ints);
            }
        }
    }

    #[test]
    fn serial_transfer_takes_eight_bits_at_8192_hz() {
        let mut serial = Serial::default();
        let mut ints = Interrupts::default();
        let mut div = 0u16;

        serial.write_sb(0xAA);
        serial.write_sc(0x81, &mut ints, CgbMode::Dmg);

        // At most 8 bits of 512 cycles, plus the phase of the master clock.
        run(&mut serial, &mut ints, &mut div, 8 * 512 + 512);

        assert_eq!(serial.read_sc() & 0x80, 0, "transfer should be complete");
        assert_eq!(serial.read_sb(), 0xFF, "no device shifts in ones");
        assert!(ints.read_if() & 0x08 != 0, "serial interrupt requested");
    }

    #[test]
    fn writing_sc_with_the_master_clock_high_clocks_the_port() {
        let mut serial = Serial::default();
        let mut ints = Interrupts::default();

        serial.set_master_clock(true);
        serial.write_sc(0x81, &mut ints, CgbMode::Dmg);
        assert!(!serial.master_clock, "the write toggled the master clock");
        assert_eq!(serial.count, 0);

        // The clock is low now: another write does not clock the port.
        serial.write_sc(0x81, &mut ints, CgbMode::Dmg);
        assert!(!serial.master_clock);
    }
}
