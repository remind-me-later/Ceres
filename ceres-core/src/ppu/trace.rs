//! What the CPU saw of the PPU (the `debug` feature): its LCD interrupt
//! requests, its STAT and LYC writes and the values STAT and LY reads
//! returned, each with where the display and the timing unit were; and what
//! the pixel pipeline did: the pixels it pushed and the tile fetches it began.

use {
    super::{LCDC_ON_B, Ppu},
    crate::interrupts::Interrupts,
    alloc::vec::Vec,
    core::mem,
};

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
    /// The CPU wrote this value to LCDC (`0xFF40`), STAT (`0xFF41`), LYC
    /// (`0xFF45`) or WX (`0xFF4B`).
    Write { addr: u16, value: u8 },
    /// The pipeline pushed the pixel of column `x` to the LCD.
    Pixel { x: u8 },
    /// The background fetcher began a tile: the window's or the background's.
    TileFetch { window: bool },
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
    /// Where the display is, for a debugger.
    pub(crate) const fn debug_state(&self) -> crate::PpuState {
        let clock = self.timing_position();
        crate::PpuState {
            lcd_on: self.lcdc & LCDC_ON_B != 0,
            mode: self.mode() as u8,
            line: clock.line,
            half_dot: clock.half_dot,
            pipeline_line: self.d.current_line(),
            double_speed: self.double_speed(),
        }
    }

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
        let clock = self.timing_position();
        self.trace.events.push(PpuEvent {
            unit: self.trace.unit,
            line: self.d.current_line(),
            line_clock: self.d.line_clock(),
            second_half: !self.d.half_dot(),
            timing_line: clock.line,
            timing_h: clock.half_dot,
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

    /// The pipeline pushed the pixel of column `x`.
    pub(super) fn trace_pixel(&mut self, x: u8) {
        self.record(PpuEventKind::Pixel { x });
    }

    /// The background fetcher began a tile.
    pub(super) fn trace_tile_fetch(&mut self, window: bool) {
        self.record(PpuEventKind::TileFetch { window });
    }

    /// The CPU wrote `value` to LCDC, STAT, LYC or WX.
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
