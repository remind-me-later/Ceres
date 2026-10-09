//! Per-dot display engine, structured after SameBoy's `GB_display_run`.
//!
//! SameBoy models the PPU as a coroutine that sleeps for a fixed number of
//! dots between observable events. This module keeps the same shape: `state`
//! is the sleep the engine is currently in, `wait` is the number of dots
//! left in it, and the state machine in `state.rs` holds the code that
//! follows each sleep.
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
mod gstat;
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
    gstat::GStat,
    objects::{ObjectFetch, ObjectSearch},
    stat::StatIrq,
    window::Window,
};

use super::{Ppu, STAT_MODE_B, oam_bug::NO_ROW};

pub(super) use state::State;

pub(super) const MODE2_LENGTH: i32 = 80;
pub(super) const LINE_LENGTH: i32 = 456;
pub(super) const LINES: u8 = 144;

#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent flags of the line state"
)]
#[derive(Clone)]
pub(super) struct Display {
    /// The sleep the state machine is in.
    state: State,
    /// Dots left before the code following the current sleep runs.
    wait: i32,
    /// Double speed: the first T-cycle of the current dot already ran.
    half_dot: bool,
    /// `cycles_for_line`: SameBoy's line-length accounting.
    cfl: i32,
    /// Dots since the last `cycles_for_line` reset (diagnostics / injection).
    line_clock: i32,

    current_line: u8,
    /// `position_in_line`: wraps like SameBoy's `uint8_t` (240..=255 is -16..=-1).
    position_in_line: u8,
    lcd_x: u8,
    line_has_fractional_scrolling: bool,

    /// The PPU entered `HBlank` (SameBoy state 33); consumed by the HDMA.
    hblank_hdma_edge: bool,
    /// Dots until the HBlank HDMA request is raised (0: none pending).
    hblank_hdma_delay: u8,
    /// `line_clock` counts from the start of the current line (false on the
    /// first line after the LCD is turned on and after a cut off line).
    line_clock_valid: bool,
    /// The LCD was switched off with a non-zero STAT mode; consumed by the HDMA.
    lcd_off_hdma_edge: bool,

    bg_fifo: Fifo,
    oam_fifo: Fifo,
    insert_bg_pixel: bool,

    objs: ObjectSearch,
    obj_fetch: ObjectFetch,
    fetcher: Fetcher,
    window: Window,
    irq: StatIrq,
    gstat: GStat,
    bus: PpuBus,
    cpu: CpuAccess,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            state: State::LcdOn,
            wait: 0,
            half_dot: false,
            cfl: 0,
            line_clock: 0,
            current_line: 0,
            position_in_line: 240,
            lcd_x: 0,
            line_has_fractional_scrolling: false,
            hblank_hdma_edge: false,
            hblank_hdma_delay: 0,
            line_clock_valid: false,
            lcd_off_hdma_edge: false,
            bg_fifo: Fifo::default(),
            oam_fifo: Fifo::default(),
            insert_bg_pixel: false,
            objs: ObjectSearch::default(),
            obj_fetch: ObjectFetch::default(),
            fetcher: Fetcher::default(),
            window: Window::default(),
            irq: StatIrq::default(),
            gstat: GStat::default(),
            bus: PpuBus::default(),
            cpu: CpuAccess::default(),
        }
    }
}

impl Display {
    pub(super) const fn restart(&mut self) {
        self.state = State::LcdOn;
        self.wait = 0;
        self.cfl = 0;
        self.window.wy_units = 0;
        self.half_dot = false;
    }

    pub(super) const fn state(&self) -> State {
        self.state
    }

    pub(super) const fn half_dot(&self) -> bool {
        self.half_dot
    }

    pub(super) const fn line_clock_valid(&self) -> bool {
        self.line_clock_valid
    }

    pub(super) const fn line_clock(&self) -> i32 {
        self.line_clock
    }

    pub(super) const fn current_line(&self) -> u8 {
        self.current_line
    }

    pub(super) const fn position_in_line(&self) -> u8 {
        self.position_in_line
    }

    /// The OAM row the PPU is reading (for the OAM bug).
    pub(super) const fn accessed_oam_row(&self) -> u8 {
        self.objs.accessed_oam_row
    }

    pub(super) const fn fetching_object(&self) -> bool {
        self.obj_fetch.active
    }

    /// The CPU's access to the memories the PPU uses.
    pub(super) const fn cpu(&self) -> &CpuAccess {
        &self.cpu
    }

    pub(super) const fn set_tile_sel_glitch(&mut self, active: bool) {
        self.fetcher.tile_sel_glitch = active;
    }

    pub(super) const fn set_wx_just_changed(&mut self, active: bool) {
        self.window.wx_just_changed = active;
    }

    pub(super) const fn set_window_enable_pending(&mut self, active: bool) {
        self.window.enable_pending = active;
    }

    /// Whether `HBlank` was entered since the last call.
    pub(super) const fn hblank_hdma_pending(&self) -> u8 {
        self.hblank_hdma_delay
    }

    pub(super) const fn take_hblank_hdma_edge(&mut self) -> bool {
        mem::replace(&mut self.hblank_hdma_edge, false)
    }

    /// Whether the LCD was switched off in a non-zero mode since the last call.
    pub(super) const fn take_lcd_off_hdma_edge(&mut self) -> bool {
        mem::replace(&mut self.lcd_off_hdma_edge, false)
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
    pub(super) const fn sleep(&mut self, id: State, n: i32) {
        self.d.state = id;
        self.d.wait = n;
    }

    pub(in crate::ppu) fn lcd_off(&mut self) {
        self.d.lcd_off_hdma_edge = self.stat & STAT_MODE_B != 0;
        self.d.objs.accessed_oam_row = NO_ROW;
        self.d.cfl = 0;
        self.d.state = State::LcdOn;
        self.d.wait = 0;
        self.ly = 0;
        self.stat &= !STAT_MODE_B;
        self.d.current_line = 0;
        self.d.irq.ly_for_comparison = 0;
        self.d.window.wy_triggered = false;
        self.d.window.line0_wy_countdown = 0;
        self.d.line_clock_valid = false;
        self.d.cpu.unlock_all();
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
        self.gstat_unit(ints);
        // Pre-run bookkeeping (top of GB_display_run).
        if self.d.window.wy_triggered {
            self.d.window.wy_check_scheduled = false;
        }

        // A line that would outgrow 456 dots is cut off (mode 3 abort).
        // `balance` is SameBoy's `display_cycles`.
        let balance = 2 - 2 * self.d.wait - i32::from(self.d.half_dot);
        let cut = 2 * self.d.cfl + 1 + balance > 2 * LINE_LENGTH && self.d.state != State::LcdOn;
        if cut {
            if self.d.state == State::HBlankStart {
                self.stat &= !STAT_MODE_B;
                self.d.irq.mode_for_interrupt = 0;
                self.stat_update(ints);
            }
            self.d.state = State::Mode3Abort;
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
            if self.d.hblank_hdma_delay > 0 {
                self.d.hblank_hdma_delay -= 1;
                if self.d.hblank_hdma_delay == 0 {
                    self.d.hblank_hdma_edge = true;
                }
            }
            if self.d.window.line0_wy_countdown > 0 {
                self.d.window.line0_wy_countdown -= 1;
                if self.d.window.line0_wy_countdown == 0 {
                    // The trigger of line 0 is decided afresh: a WY that
                    // changed since the line began can also take it away.
                    self.d.window.wy_triggered = self.lcdc & 0x20 != 0 && self.wy == 0;
                }
            }
            if self.d.irq.line0_pulse > 0 {
                self.d.irq.line0_pulse -= 1;
                if self.d.irq.line0_pulse == 0 {
                    self.d.irq.mode_for_interrupt = 2;
                    self.stat_update(ints);
                    self.d.irq.mode_for_interrupt = -1;
                    self.stat_update(ints);
                }
            }
            self.step_state_machine(ints);
        } else if self.d.irq.line153_compare_pending {
            self.d.irq.line153_compare_pending = false;
            self.d.irq.ly_for_comparison = 153;
            self.stat_update(ints);
        } else {
            // Nothing runs in the second half of a dot.
        }
        if self.d.irq.lyc_line_hold > 0 {
            self.d.irq.lyc_line_hold -= 1;
            if self.d.irq.lyc_line_hold == 0 {
                self.stat_update(ints);
            }
        }
        self.advance_wy_units(1);
    }
}
