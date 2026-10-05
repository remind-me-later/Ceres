//! Per-dot display engine, structured after SameBoy's `GB_display_run`.
//!
//! SameBoy models the PPU as a coroutine that sleeps for a fixed number of
//! dots between observable events. This module keeps the same shape: `state`
//! is the id of the sleep the engine is currently in (the same numbers as
//! SameBoy's `display_state`), `wait` is the number of dots left in it, and
//! the state machine in `state.rs` holds the code that follows each sleep.
//!
//! - `state.rs`: the line and frame state machine
//! - `objects.rs`: the mode 2 object search and the object fetch
//! - `fetcher.rs`: the background/window tile fetcher
//! - `window.rs`: window triggering and its glitches
//! - `mode3.rs`: the mode 3 drawing loop
//! - `fifo.rs` and `pixels.rs`: the pixel FIFOs, mixing and palette lookup
//! - `stat.rs`: the STAT interrupt line and LY=LYC
//! - `bus.rs`: the PPU's own memory accesses, the DMA/HDMA bus conflicts and
//!   the CPU-visible access locks

mod bus;
mod fetcher;
mod fifo;
mod mode3;
mod objects;
mod pixels;
mod stat;
mod state;
mod window;

use {
    crate::{CgbMode, Model, interrupts::Interrupts},
    bus::{CpuAccess, PpuBus},
    core::mem,
    fetcher::Fetcher,
    fifo::Fifo,
    objects::{ObjectFetch, ObjectSearch},
    stat::StatIrq,
    window::Window,
};

use super::{Ppu, STAT_MODE_B, oam_bug::NO_ROW};

pub(super) const MODE2_LENGTH: i32 = 80;
pub(super) const LINE_LENGTH: i32 = 456;
pub(super) const LINES: u8 = 144;

#[expect(
    clippy::partial_pub_fields,
    reason = "The rest of the PPU reads the line position and the HDMA edges"
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent flags of the line state"
)]
#[derive(Clone)]
pub(super) struct Display {
    /// SameBoy's `display_state`: id of the current sleep (0 = not started).
    pub state: u8,
    /// Dots left before the code following the current sleep runs.
    wait: i32,
    /// Double speed: the first T-cycle of the current dot already ran.
    pub half_dot: bool,
    /// `cycles_for_line`: SameBoy's line-length accounting.
    cfl: i32,
    /// Dots since the last `cycles_for_line` reset (diagnostics / injection).
    pub line_clock: i32,

    pub current_line: u8,
    /// `position_in_line`: wraps like SameBoy's `uint8_t` (240..=255 is -16..=-1).
    pub position_in_line: u8,
    lcd_x: u8,
    line_has_fractional_scrolling: bool,

    /// The PPU entered `HBlank` (SameBoy state 33); consumed by the HDMA.
    pub hblank_hdma_edge: bool,
    /// The LCD was switched off with a non-zero STAT mode; consumed by the HDMA.
    pub lcd_off_hdma_edge: bool,

    bg_fifo: Fifo,
    oam_fifo: Fifo,
    insert_bg_pixel: bool,

    pub objs: ObjectSearch,
    pub obj_fetch: ObjectFetch,
    pub fetcher: Fetcher,
    pub window: Window,
    irq: StatIrq,
    bus: PpuBus,
    pub cpu: CpuAccess,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            state: 0,
            wait: 0,
            half_dot: false,
            cfl: 0,
            line_clock: 0,
            current_line: 0,
            position_in_line: 240,
            lcd_x: 0,
            line_has_fractional_scrolling: false,
            hblank_hdma_edge: false,
            lcd_off_hdma_edge: false,
            bg_fifo: Fifo::default(),
            oam_fifo: Fifo::default(),
            insert_bg_pixel: false,
            objs: ObjectSearch::default(),
            obj_fetch: ObjectFetch::default(),
            fetcher: Fetcher::default(),
            window: Window::default(),
            irq: StatIrq::default(),
            bus: PpuBus::default(),
            cpu: CpuAccess::default(),
        }
    }
}

impl Display {
    pub(super) const fn restart(&mut self) {
        self.state = 0;
        self.wait = 0;
        self.cfl = 0;
        self.window.wy_units = 0;
        self.half_dot = false;
    }

    /// Schedule the WY comparison that follows an LCDC/WY write.
    pub(super) const fn schedule_wy_check(&mut self) {
        self.window.wy_check_scheduled = true;
    }
}

/// Model helpers matching SameBoy's `gb->model` comparisons.
const fn model_ge_cgb_d(m: Model) -> bool {
    matches!(m, Model::CgbD | Model::CgbE | Model::Agb)
}

impl Ppu {
    #[inline]
    pub(in crate::ppu) const fn hw_cgb(&self) -> bool {
        self.model.is_cgb_hardware()
    }

    #[inline]
    pub(super) const fn cgb_mode_on(&self) -> bool {
        matches!(self.cgb_mode, CgbMode::Cgb)
    }

    #[inline]
    pub(super) const fn double_speed(&self) -> bool {
        self.double_speed
    }

    /// True when a DMG-family model (not SGB) is running.
    #[inline]
    pub(super) const fn is_dmg_family(&self) -> bool {
        matches!(self.model, Model::Dmg0 | Model::DmgB | Model::Mgb)
    }

    /// Sleep for `n` dots; the code of state `id` runs afterwards.
    #[inline]
    pub(super) const fn sleep(&mut self, id: u8, n: i32) {
        self.d.state = id;
        self.d.wait = n;
    }

    pub(in crate::ppu) fn lcd_off(&mut self) {
        self.d.lcd_off_hdma_edge = self.stat & STAT_MODE_B != 0;
        self.d.objs.accessed_oam_row = NO_ROW;
        self.d.cfl = 0;
        self.d.state = 0;
        self.d.wait = 0;
        self.ly = 0;
        self.stat &= !STAT_MODE_B;
        self.d.current_line = 0;
        self.d.irq.ly_for_comparison = 0;
        self.d.window.wy_triggered = false;
        self.d.cpu.oam_read_blocked = false;
        self.d.cpu.vram_read_blocked = false;
        self.d.cpu.oam_write_blocked = false;
        self.d.cpu.vram_write_blocked = false;
        self.d.cpu.cgb_palettes_blocked = false;
    }

    /// Start of a line for lines 0..=143 (SameBoy's `for` body head).
    pub(super) fn line_start(&mut self) {
        self.wy_check();
        self.d.cpu.oam_write_blocked = self.hw_cgb() && !self.double_speed();
        self.d.objs.accessed_oam_row = 0;
        self.sleep(35, 2);
    }

    pub(super) fn present_frame(&mut self) {
        self.rgba_buf_present = mem::take(&mut self.rgb_buf);
    }

    /// Advance the display by one unit (half a dot, SameBoy's 8 MHz tick).
    ///
    /// SameBoy's state machine runs the code that follows a sleep as soon as
    /// the time slept is exceeded, so a dot's work runs at the first of its
    /// two units (`half_dot` says whether that already happened). It matters
    /// for the accesses that land between units: after a speed switch or in
    /// a split write in double speed.
    pub(in crate::ppu) fn run_display(&mut self, ints: &mut Interrupts) {
        // Pre-run bookkeeping (top of GB_display_run).
        if self.d.window.wy_triggered {
            self.d.window.wy_check_scheduled = false;
        }

        // A line that would outgrow 456 dots is cut off (mode 3 abort).
        // `balance` is SameBoy's `display_cycles`.
        let balance = 2 - 2 * self.d.wait - i32::from(self.d.half_dot);
        let cut = 2 * self.d.cfl + 1 + balance > 2 * LINE_LENGTH && self.d.state != 0;
        if cut {
            if self.d.state == 22 {
                self.stat &= !STAT_MODE_B;
                self.d.irq.mode_for_interrupt = 0;
                self.stat_update(ints);
            }
            self.d.state = 9;
            self.d.wait = 0;
        }

        self.d.half_dot = !self.d.half_dot;
        if self.d.half_dot || cut {
            if self.d.irq.delayed_glitch_hblank_interrupt && self.d.current_line < LINES {
                self.d.irq.delayed_glitch_hblank_interrupt = false;
                self.d.irq.mode_for_interrupt = 0;
                self.stat_update(ints);
                self.d.irq.mode_for_interrupt = 3;
            }

            self.d.line_clock += 1;
            self.step_state_machine(ints);
        }
        self.advance_wy_units(1);
    }
}
