//! What the CPU saw of the PPU (the `debug` feature): its LCD interrupt
//! requests, its STAT and LYC writes and the values STAT and LY reads
//! returned, each with where the display and the timing unit were.

use {super::Ppu, crate::interrupts::Interrupts, alloc::vec::Vec, core::mem};

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PpuEventKind {
    /// The PPU requested the LCD (STAT) interrupt.
    LcdInterrupt,
    /// The CPU read STAT and got this value.
    StatRead(u8),
    /// The CPU read LY and got this value.
    LyRead(u8),
    /// The CPU wrote this value to STAT (`0xFF41`) or LYC (`0xFF45`).
    Write { addr: u16, value: u8 },
}

/// An event and where the display was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PpuEvent {
    /// Half dots the display has run since power on.
    pub unit: u64,
    /// The line the display was on.
    pub line: u8,
    /// Dots since the line began.
    pub line_clock: i32,
    /// The event came in the second half of the dot.
    pub second_half: bool,
    /// The line and half dot on the timing unit's clock (`timing.rs`).
    pub timing_line: u8,
    pub timing_h: i32,
    /// The CPU ran at double speed.
    pub double_speed: bool,
    pub kind: PpuEventKind,
}

#[derive(Default)]
pub(super) struct Trace {
    enabled: bool,
    events: Vec<PpuEvent>,
    unit: u64,
    /// `Interrupts::lcd_requests` when last looked at.
    lcd_requests: u32,
}

impl Ppu {
    /// Starts recording events, or stops and forgets them.
    pub(crate) fn set_trace(&mut self, enabled: bool, ints: &Interrupts) {
        self.trace.enabled = enabled;
        self.trace.events.clear();
        self.trace.lcd_requests = ints.lcd_requests();
    }

    /// The events recorded since the last call.
    pub(crate) fn take_trace(&mut self) -> Vec<PpuEvent> {
        mem::take(&mut self.trace.events)
    }

    fn record(&mut self, kind: PpuEventKind) {
        if !self.trace.enabled {
            return;
        }
        self.trace.events.push(PpuEvent {
            unit: self.trace.unit,
            line: self.d.current_line(),
            line_clock: self.d.line_clock(),
            second_half: !self.d.half_dot(),
            timing_line: self.timing_position().0,
            timing_h: self.timing_position().1,
            double_speed: self.double_speed(),
            kind,
        });
    }

    /// A display unit ran.
    pub(super) fn trace_unit(&mut self, ints: &Interrupts) {
        self.trace.unit += 1;
        self.trace_requests(ints);
    }

    /// Records the LCD interrupt requests made since the last look.
    pub(super) fn trace_requests(&mut self, ints: &Interrupts) {
        let requests = ints.lcd_requests();
        for _ in 0..requests.wrapping_sub(self.trace.lcd_requests) {
            self.record(PpuEventKind::LcdInterrupt);
        }
        self.trace.lcd_requests = requests;
    }

    /// The CPU wrote `value` to STAT or LYC.
    pub(super) fn trace_write(&mut self, addr: u16, value: u8) {
        self.record(PpuEventKind::Write { addr, value });
    }

    /// The CPU read `value` from STAT or LY.
    pub(crate) fn trace_read(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF41 => self.record(PpuEventKind::StatRead(value)),
            0xFF44 => self.record(PpuEventKind::LyRead(value)),
            _ => {}
        }
    }
}
