//! A log of the CPU's accesses to the I/O registers (0xFF00-0xFFFF), or to
//! any address, with when they happened, for traces.

use {
    super::MemoryAccess,
    crate::{AudioCallback, Gb},
    alloc::vec::Vec,
    core::mem,
};

/// A CPU access to an I/O register (or HRAM, or IE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct IoAccess {
    pub addr: u16,
    /// The value read or written.
    pub value: u8,
    pub access: MemoryAccess,
    /// `Gb::clock_units` when the access's M-cycle began.
    pub clock_units: u64,
    /// Where the display was then, on the timing unit's clock (see
    /// `PpuState`).
    pub line: u8,
    pub half_dot: i32,
}

#[derive(Default)]
pub(crate) struct IoLog {
    enabled: bool,
    /// Every address, not only the I/O registers'.
    all: bool,
    accesses: Vec<IoAccess>,
}

impl<A: AudioCallback> Gb<A> {
    /// Starts logging the CPU's I/O accesses, or stops and forgets them.
    #[inline]
    pub fn set_io_log(&mut self, enabled: bool) {
        self.io_log.enabled = enabled;
        self.io_log.all = false;
        self.io_log.accesses.clear();
    }

    /// Starts logging every CPU access, to any address, or stops and
    /// forgets them. `take_io_log` returns them.
    #[inline]
    pub fn set_bus_log(&mut self, enabled: bool) {
        self.io_log.enabled = enabled;
        self.io_log.all = enabled;
        self.io_log.accesses.clear();
    }

    /// The I/O accesses logged since the last call.
    #[inline]
    pub fn take_io_log(&mut self) -> Vec<IoAccess> {
        mem::take(&mut self.io_log.accesses)
    }

    /// Logs a CPU access, if the log is on and takes the address.
    /// `t_cycles_ahead`: how far the access is past the clock (a write
    /// lands after the time still deferred).
    #[inline]
    pub(crate) fn log_io(
        &mut self,
        addr: u16,
        value: u8,
        access: MemoryAccess,
        t_cycles_ahead: i32,
    ) {
        if !self.io_log.enabled || (addr < 0xFF00 && !self.io_log.all) {
            return;
        }
        let units_per_cycle = if self.key1.is_enabled() { 1 } else { 2 };
        let ahead = u64::try_from(t_cycles_ahead.max(0)).unwrap_or_default() * units_per_cycle;
        let ppu = self.ppu.debug_state();
        self.io_log.accesses.push(IoAccess {
            addr,
            value,
            access,
            clock_units: self.clock_units + ahead,
            line: ppu.line,
            half_dot: ppu.half_dot,
        });
    }
}
