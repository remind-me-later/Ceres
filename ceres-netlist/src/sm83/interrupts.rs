//! The SM83's interrupt logic (`hdl/rtl/sm83_interrupts.vhd`): IE, the
//! requests that are enabled, their priority, the acknowledges to IF and
//! the entry's vector. The requests are latched while `data_phase` is low;
//! in `write_phase` the lowest is selected, and M-cycle 6 of the entry
//! acknowledges it and loads its vector, 40 + 8 times its number.
//!
//! @hw die: dmg-schematics, categories irq and irq-ie

use super::{control::Controls, decoder::Steps};

/// The interrupt logic's latches.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct NetCoreInterrupts {
    /// IE: its master takes the data bus in `write_phase` while the core
    /// writes FFFF; the output follows while it does not.
    // @hw die: REG_IE (dffr_cc_ee_reg_ie_bit), IRQ_AND, REG_IE_NOT_WE
    pub ie_next: u8,
    pub ie: u8,
    /// The requests that are enabled, latched while `data_phase` is low.
    // @hw die: IRQ_LATCH (dlatch_ee_irq)
    pub pending: u8,
}

/// What the interrupt logic gives in a phase.
#[derive(Clone, Copy, Default)]
pub struct Selected {
    /// An enabled interrupt is requested (latched; and as it is now).
    pub int_pending: bool,
    pub int_now: bool,
    /// One is selected (in `write_phase`).
    pub int_take: bool,
    /// The acknowledges.
    pub inta: u8,
    /// The vector's bits 7 to 3, in place.
    pub int_vector: u8,
}

/// The core addresses FFFF.
#[must_use]
pub const fn ffff(addr: u16) -> bool {
    addr == 0xFFFF
}

impl NetCoreInterrupts {
    /// IE's output follows its master while the core does not write it.
    pub const fn show(&mut self, c: &Controls, addr: u16) {
        if !(ffff(addr) & c.mwrite) {
            self.ie = self.ie_next;
        }
    }

    /// The bits a read of FFFF pulls down on the data bus.
    // @hw die: IRQ_NAND2_NAND3
    #[must_use]
    pub const fn db_pull(&self, rd: bool, addr: u16) -> u8 {
        if rd & ffff(addr) { !self.ie } else { 0 }
    }

    /// IE takes the data bus; the requests are latched; the lowest is
    /// selected, and acknowledged in the entry's M-cycle 6.
    // @hw die: IRQ_PRIO (nand_a, nand_b, dist_nor, and, nand_c to nand_e)
    #[expect(clippy::too_many_arguments, reason = "the logic's inputs")]
    pub fn settle(
        &mut self,
        s: &Steps,
        c: &Controls,
        phases: (bool, bool),
        sys_reset: bool,
        addr: u16,
        db: u8,
        int: u8,
        nmi_entry: bool,
    ) -> Selected {
        let (data_phase, write_phase) = phases;
        if sys_reset {
            self.ie_next = 0;
        } else if ffff(addr) & c.mwrite & write_phase {
            self.ie_next = db;
        } else {
            // The master holds.
        }
        let requested = self.ie & int;
        if !data_phase {
            self.pending = requested;
        }
        let selected = if write_phase & !nmi_entry {
            crate::interrupts::lowest(self.pending)
        } else {
            0
        };
        let inta = if s.int_entry_m6 { selected } else { 0 };
        let vector = u8::from(inta & 0xAA != 0) << 3
            | u8::from(inta & 0xCC != 0) << 4
            | u8::from(inta & 0xF0 != 0) << 5
            | u8::from(s.int_entry_m6 & write_phase & (selected != 0)) << 6
            | u8::from(s.int_entry_m6 & nmi_entry) << 7;
        Selected {
            int_pending: self.pending != 0,
            int_now: requested != 0,
            int_take: selected != 0,
            inta,
            int_vector: vector,
        }
    }
}
