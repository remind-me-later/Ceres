//! Per-dot display engine, structured after SameBoy's `GB_display_run`.
//!
//! SameBoy models the PPU as a coroutine that sleeps for a fixed number of
//! dots between observable events. This module keeps the same shape: `state`
//! is the sleep the engine is currently in, `wait` is the number of dots
//! left in it, and the state machine in `state.rs` holds the code that
//! follows each sleep. Those and mode 3's timing are the display's pace
//! (`Pace`), which the PPU keeps apart from the rest (`Ppu::pace`), so that
//! what mode 3's loop asks of the PPU cannot change it.
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

mod background;
mod bus;
mod cgb_c;
mod fetcher;
mod fifo;
mod mode3;
mod objects;
mod pixels;
mod stat;
mod state;
mod timing;
mod window;

use {
    crate::{CgbMode, Model, interrupts::Interrupts},
    background::Background,
    bus::{CpuAccess, PpuBus},
    cgb_c::CgbC,
    core::mem,
    fetcher::Fetcher,
    fifo::Fifo,
    objects::{OamLatches, ObjectFetch, ObjectSearch},
    window::Window,
};

use super::{LCDC_WIN_EN_B, PX_HEIGHT, Ppu, PpuCore, STAT_MODE_B, oam_bug::NO_ROW};

pub(super) use state::State;

pub(super) const MODE2_LENGTH: i32 = 80;
/// The PPU addresses VRAM from 0 (0x8000 for the CPU), bank 1 after bank 0.
pub(super) const VRAM_BANK1: u16 = 0x2000;
/// The offset in a VRAM bank.
pub(super) const VRAM_OFFSET_MASK: u16 = VRAM_BANK1 - 1;
/// Two bytes for each of the 8 rows of a tile.
pub(super) const TILE_BYTES: u16 = 0x10;
pub(super) const LINE_LENGTH: i32 = 456;
/// The visible lines: VBlank starts with the next one.
pub(super) const LINES: u8 = PX_HEIGHT;
/// The last line of a frame.
pub(super) const LAST_LINE: u8 = 153;
pub(super) const LINES_PER_FRAME: u8 = LAST_LINE + 1;

/// The display's pace: what decides when it does what. The sleep the
/// state machine is in, and mode 3's: the background's pipeline, the
/// objects left to draw, the object fetch and the window.
///
/// Mode 3's loop (`mode3::mode3`) changes it; what the loop asks of the
/// PPU (`mode3::Mode3Ppu`: the fetchers' reads, the object's row, the
/// pixels) only reads it, as the PPU keeps it apart from the rest
/// (`Ppu::pace`, beside `PpuCore`), and the hooks are the core's.
#[derive(Clone, Default)]
pub(super) struct Pace {
    /// The sleep the state machine is in.
    state: State,
    /// Dots left before the code following the current sleep runs.
    wait: i32,
    /// `cycles_for_line`: SameBoy's line-length accounting.
    cfl: i32,
    bg: Background,
    objs: ObjectSearch,
    /// An object's tile is being fetched.
    fetching_object: bool,
    /// The DMG aborted the object fetch (LCDC's object bit fell).
    object_fetch_aborted: bool,
    window: Window,
}

/// The rest of the display's state: what mode 3's hooks may change (the
/// fetchers' reads, the object FIFO, the LCD's column) and what the line's
/// timing keeps outside mode 3.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent flags of the line state"
)]
#[derive(Clone, Default)]
pub(super) struct Display {
    /// Double speed: the first T-cycle of the current dot already ran.
    half_dot: bool,
    /// Dots since the last `cycles_for_line` reset (diagnostics / injection).
    line_clock: i32,

    current_line: u8,
    lcd_x: u8,

    /// The PPU entered `HBlank` (SameBoy state 33); consumed by the HDMA.
    hblank_hdma_edge: bool,
    /// Dots until the HBlank HDMA request is raised (0: none pending).
    hblank_hdma_delay: u8,
    /// `line_clock` counts from the start of the current line (false on the
    /// first line after the LCD is turned on and after a cut off line).
    line_clock_valid: bool,
    /// The LCD was switched off with a non-zero STAT mode; consumed by the HDMA.
    lcd_off_hdma_edge: bool,

    /// What the background's fetcher reads.
    fetcher: Fetcher,
    oam_fifo: Fifo,
    oam_latches: OamLatches,
    obj_fetch: ObjectFetch,
    /// The line the WY check compares with on the DMG and in double speed
    /// (SameBoy's `ly_for_comparison`); `-1` around LY changes.
    ly_for_comparison: i32,
    /// Line 153's comparison line changes in the second half of the current
    /// dot (CGB-C in double speed).
    line153_compare_pending: bool,
    cgb_c: CgbC,
    /// The LCD timing and STAT interrupt (`timing.rs`).
    timing: timing::LcdTiming,
    bus: PpuBus,
    cpu: CpuAccess,
}

/// The `position_in_line` of a signed position: like SameBoy's `uint8_t`,
/// the lead-in positions -16..=-1 are 240..=255.
pub(super) const fn pos(signed: i8) -> u8 {
    signed.cast_unsigned()
}

impl Pace {
    /// Sleep for `n` dots; the code of state `id` runs afterwards.
    #[inline]
    pub(super) const fn sleep(&mut self, id: State, n: i32) {
        self.state = id;
        self.wait = n;
    }

    /// Sleep for `n` dots that count towards the length of the line.
    #[inline]
    pub(super) const fn sleep_in_line(&mut self, id: State, n: i32) {
        self.cfl += n;
        self.sleep(id, n);
    }

    /// A dot of the sleep passes: whether it ended, and the code that follows
    /// it runs (`wait` dots remain, it ends when they reach zero).
    #[inline]
    pub(super) const fn wake(&mut self) -> bool {
        if self.wait > 0 {
            self.wait -= 1;
        }
        self.wait == 0
    }

    pub(super) const fn restart(&mut self) {
        self.state = State::LcdOn;
        self.wait = 0;
        self.cfl = 0;
        self.window.wy_units = 0;
    }

    pub(super) const fn state(&self) -> State {
        self.state
    }

    pub(super) const fn position_in_line(&self) -> u8 {
        self.bg.position
    }

    /// The OAM row the PPU is reading (for the OAM bug).
    pub(super) const fn accessed_oam_row(&self) -> u8 {
        self.objs.accessed_oam_row
    }

    pub(super) const fn fetching_object(&self) -> bool {
        self.fetching_object
    }

    pub(super) const fn set_wx_just_changed(&mut self, active: bool) {
        self.window.wx_just_changed = active;
    }

    pub(super) const fn set_window_enable_pending(&mut self, active: bool) {
        self.window.enable_pending = active;
    }

    /// Schedule the WY comparison that follows an LCDC/WY write.
    pub(super) const fn schedule_wy_check(&mut self) {
        self.window.wy_check_scheduled = true;
    }
}

impl Display {
    pub(super) const fn restart(&mut self) {
        self.half_dot = false;
        self.cgb_c.restart();
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

    /// The CPU's access to the memories the PPU uses.
    pub(super) const fn cpu(&self) -> &CpuAccess {
        &self.cpu
    }

    pub(super) const fn set_tile_sel_glitch(&mut self, active: bool) {
        self.fetcher.tile_sel_glitch = active;
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
}

/// Steps a countdown that is idle at 0; true when it just ran out.
const fn count_down(counter: &mut u8) -> bool {
    if *counter == 0 {
        return false;
    }
    *counter -= 1;
    *counter == 0
}

/// Model helpers matching SameBoy's `gb->model` comparisons.
const fn model_ge_cgb_d(m: Model) -> bool {
    matches!(m, Model::CgbD | Model::CgbE | Model::Agb)
}

impl PpuCore {
    #[inline]
    pub(in crate::ppu) const fn is_cgb_hardware(&self) -> bool {
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

    pub(super) fn present_frame(&mut self) {
        self.rgba_buf_present = mem::take(&mut self.rgb_buf);
    }
}

impl Ppu {
    /// Sleep for `n` dots; the code of state `id` runs afterwards.
    #[inline]
    pub(super) const fn sleep(&mut self, id: State, n: i32) {
        self.pace.sleep(id, n);
    }

    /// Sleep for `n` dots that count towards the length of the line.
    #[inline]
    pub(super) const fn sleep_in_line(&mut self, id: State, n: i32) {
        self.pace.sleep_in_line(id, n);
    }

    pub(in crate::ppu) const fn lcd_off(&mut self) {
        let core = &mut self.core;
        core.d.lcd_off_hdma_edge = core.stat & STAT_MODE_B != 0;
        self.pace.objs.accessed_oam_row = NO_ROW;
        self.pace.cfl = 0;
        self.pace.state = State::LcdOn;
        self.pace.wait = 0;
        core.ly = 0;
        core.stat &= !STAT_MODE_B;
        core.d.current_line = 0;
        core.d.ly_for_comparison = 0;
        self.pace.window.wy_triggered = false;
        self.pace.window.line0_wy_countdown = 0;
        core.d.line_clock_valid = false;
        core.d.cpu.unlock_all();
    }

    /// Advance the display by one unit (half a dot, SameBoy's 8 MHz tick).
    ///
    /// SameBoy's state machine runs the code that follows a sleep as soon as
    /// the time slept is exceeded, so a dot's work runs at the first of its
    /// two units (`half_dot` says whether that already happened). It matters
    /// for the accesses that land between units: after a speed switch or in
    /// a split write in double speed.
    pub(in crate::ppu) fn run_display(&mut self, ints: &mut Interrupts) {
        self.timing_unit(ints);
        self.cgb_c_unit();
        // Pre-run bookkeeping (top of GB_display_run).
        if self.pace.window.wy_triggered {
            self.pace.window.wy_check_scheduled = false;
        }

        // A line that would outgrow 456 dots is cut off (mode 3 abort).
        // `balance` is SameBoy's `display_cycles`.
        let balance = 2 - 2 * self.pace.wait - i32::from(self.d.half_dot);
        let cut =
            2 * self.pace.cfl + 1 + balance > 2 * LINE_LENGTH && self.pace.state != State::LcdOn;
        if cut {
            if self.pace.state == State::HBlankStart {
                self.stat &= !STAT_MODE_B;
            }
            self.pace.state = State::Mode3Abort;
            self.pace.wait = 0;
        }

        self.d.half_dot = !self.d.half_dot;
        if self.d.half_dot || cut {
            self.d.line_clock += 1;
            if count_down(&mut self.d.hblank_hdma_delay) {
                self.d.hblank_hdma_edge = true;
            }
            if count_down(&mut self.pace.window.line0_wy_countdown) {
                // The trigger of line 0 is decided afresh: a WY that
                // changed since the line began can also take it away.
                self.pace.window.wy_triggered = self.lcdc & LCDC_WIN_EN_B != 0 && self.wy == 0;
            }
            self.step_state_machine(ints);
        } else if self.d.line153_compare_pending {
            self.d.line153_compare_pending = false;
            self.d.ly_for_comparison = LAST_LINE.into();
        } else {
            // Nothing runs in the second half of a dot.
        }
        self.cgb_c_after_unit();
        self.advance_wy_unit();
    }
}
