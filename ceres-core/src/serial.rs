use crate::{CgbMode, interrupts::Interrupts};
#[cfg(feature = "debug")]
use alloc::string::String;

// SC bits
/// A transfer is running (or requested).
const SC_START_B: u8 = 0x80;
/// CGB: the fast clock.
const SC_FAST_CLOCK_B: u8 = 0x02;
/// This side drives the clock.
const SC_INTERNAL_CLOCK_B: u8 = 0x01;
/// A transfer this side clocks.
const SC_INTERNAL_TRANSFER: u8 = SC_START_B | SC_INTERNAL_CLOCK_B;

/// The system counter bit whose falling edges toggle the master clock: a bit
/// shifts every second edge, at 8192 Hz (262144 Hz with the fast clock).
const CLOCK_BIT: u16 = 0x80;
const FAST_CLOCK_BIT: u16 = 0x04;

/// The serial port, without a link cable: a transfer on the internal clock
/// shifts in ones. With the `debug` feature the bytes sent are captured for
/// test ROMs.
pub(crate) struct Serial {
    count: u8,
    div_mask: u16,
    master_clock: bool,
    /// The printable bytes sent.
    #[cfg(feature = "debug")]
    output: String,
    sb: u8,
    /// The byte being sent, as written.
    #[cfg(feature = "debug")]
    sb_sent: u8,
    sc: u8,
}

impl Default for Serial {
    fn default() -> Self {
        Self {
            count: 0,
            div_mask: CLOCK_BIT,
            master_clock: false,
            #[cfg(feature = "debug")]
            output: String::new(),
            sb: 0,
            #[cfg(feature = "debug")]
            sb_sent: 0,
            sc: 0x7E,
        }
    }
}

impl Serial {
    /// Sets the master clock flip-flop from the system counter (it toggles on
    /// every falling edge of bit 7, so it follows bit 8 until DIV is written).
    pub(crate) const fn set_master_clock(&mut self, val: bool) {
        self.master_clock = val;
    }

    /// The system counter bit whose falling edges clock the serial port.
    #[must_use]
    pub(crate) const fn div_mask(&self) -> u16 {
        self.div_mask
    }

    /// The printable bytes sent (test ROMs print their results).
    #[cfg(feature = "debug")]
    #[must_use]
    pub(crate) fn output(&self) -> &str {
        &self.output
    }

    #[must_use]
    pub(crate) const fn read_sb(&self) -> u8 {
        self.sb
    }

    #[must_use]
    pub(crate) const fn read_sc(&self) -> u8 {
        self.sc
    }

    /// A falling edge of the selected system counter bit: toggles the master
    /// clock and shifts a bit on every second edge (port of SameBoy's
    /// `GB_serial_master_edge`).
    pub(crate) fn master_edge(&mut self, ints: &mut Interrupts) {
        self.master_clock = !self.master_clock;

        if !self.master_clock && self.sc & SC_INTERNAL_TRANSFER == SC_INTERNAL_TRANSFER {
            self.shift_in(ints);
        }
    }

    /// Shifts one bit in (and out); the eighth one completes the transfer.
    #[cfg_attr(
        not(feature = "debug"),
        expect(clippy::missing_const_for_fn, reason = "not const with the capture")
    )]
    fn shift_in(&mut self, ints: &mut Interrupts) {
        self.count += 1;
        if self.count == 8 {
            self.count = 0;
            self.sc &= !SC_START_B;
            ints.request_serial();

            #[cfg(feature = "debug")]
            {
                let byte = self.sb_sent;
                if (0x20..0x7F).contains(&byte) || matches!(byte, b'\n' | b'\r') {
                    self.output.push(char::from(byte));
                }
            }
        }

        self.sb <<= 1;
        // When no device is connected, the input bit reads as 1.
        self.sb |= 1;
    }

    /// Completes the transfer now if its last bit is shifted within `cycles`
    /// system clock cycles (the interrupt acknowledge looks ahead that far).
    pub(crate) fn complete_if_due(&mut self, div: u16, cycles: u16, ints: &mut Interrupts) {
        if !self.master_clock
            || self.sc & SC_INTERNAL_TRANSFER != SC_INTERNAL_TRANSFER
            || self.count != 7
        {
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

    pub(crate) const fn write_sb(&mut self, val: u8) {
        self.sb = val;
        #[cfg(feature = "debug")]
        {
            self.sb_sent = val;
        }
    }

    pub(crate) fn write_sc(&mut self, mut val: u8, ints: &mut Interrupts, cgb_mode: CgbMode) {
        self.count = 0;

        let cgb = matches!(cgb_mode, CgbMode::Cgb);
        if !cgb {
            val |= SC_FAST_CLOCK_B;
        }

        // Writing SC while the master clock is high clocks the port once
        // more, with the old SC value.
        if self.master_clock {
            self.master_edge(ints);
        }

        // Bits 6-2 and bit 1 (unless CGB) always read 1.
        self.sc = val | 0x7C;
        self.div_mask = if cgb && val & SC_FAST_CLOCK_B != 0 {
            FAST_CLOCK_BIT
        } else {
            CLOCK_BIT
        };
    }
}
