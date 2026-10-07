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
            self.shift_in(ints);
        }
    }

    /// Shifts one bit in (and out); the eighth one completes the transfer.
    fn shift_in(&mut self, ints: &mut Interrupts) {
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
            } else {
                // Not printable: ignored.
            }
        }

        self.sb <<= 1;
        // When no device is connected, the input bit reads as 1.
        self.sb |= 1;
    }

    /// Completes the transfer now if its last bit is shifted within `cycles`
    /// system clock cycles (the interrupt acknowledge looks ahead that far).
    pub fn complete_if_due(&mut self, div: u16, cycles: u16, ints: &mut Interrupts) {
        if !self.master_clock || self.sc & (START | SHIFT) != START | SHIFT || self.count != 7 {
            return;
        }
        // The edge is the system counter's selected bit falling: the
        // increment that wraps the bits below it.
        let period = self.div_mask << 1;
        let cycles_to_edge = period - (div & (period - 1));
        if cycles_to_edge <= cycles {
            self.shift_in(ints);
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
