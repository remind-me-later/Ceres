mod color_palette;
mod display;
mod oam;
mod oam_bug;
mod rgba_buf;
mod vram;

use crate::interrupts::Interrupts;
pub use oam::Oam;
pub use vram::Vram;
use {self::color_palette::ColorPalette, crate::CgbMode, crate::Model, rgba_buf::RgbaBuf};

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;

// LCDC bits
const LCDC_ON_B: u8 = 0x80;

// STAT bits
/// Dots per line.
const LINE_CYCLES: i32 = 456;
const STAT_MODE_B: u8 = 0x3;
const STAT_LYC_B: u8 = 0x4;
const STAT_IF_HBLANK_B: u8 = 0x8;
const STAT_IF_VBLANK_B: u8 = 0x10;
const STAT_IF_OAM_B: u8 = 0x20;
const STAT_IF_LYC_B: u8 = 0x40;

#[non_exhaustive]
#[derive(Clone, Copy, Default)]
pub enum ColorCorrectionMode {
    CorrectCurves,
    Disabled,
    LowContrast,
    #[default]
    ModernBalanced,
    ModernBoostContrast,
    ReduceContrast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

pub struct Ppu {
    model: Model,
    bcp: ColorPalette,
    bgp: u8,
    color_correction_mode: ColorCorrectionMode,
    /// CGB operating mode as last seen by the dot clock.
    cgb_mode: CgbMode,
    /// Whether the CPU is running in double speed (set by the dot clock).
    double_speed: bool,
    lcdc: u8,
    ly: u8,
    lyc: u8,
    oam: Oam,
    obp0: u8,
    obp1: u8,
    ocp: ColorPalette,
    opri: bool,
    rgb_buf: RgbaBuf,
    rgba_buf_present: RgbaBuf,
    scx: u8,
    scy: u8,
    /// STAT as SameBoy keeps it: mode bits, LYC flag and the enable bits.
    stat: u8,
    vram: Vram,
    wx: u8,
    wy: u8,
    d: display::Display,
    /// Half-dots the display already ran ahead of the CPU (see
    /// `Gb::ack_interrupt`) and must not run again.
    skip_units: u8,
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            model: Model::default(),
            bcp: ColorPalette::default(),
            bgp: 0,
            color_correction_mode: ColorCorrectionMode::default(),
            cgb_mode: CgbMode::default(),
            double_speed: false,
            lcdc: 0,
            ly: 0,
            lyc: 0,
            oam: Oam::default(),
            obp0: 0,
            obp1: 0,
            ocp: ColorPalette::default(),
            opri: false,
            rgb_buf: RgbaBuf::default(),
            rgba_buf_present: RgbaBuf::default(),
            scx: 0,
            scy: 0,
            stat: Mode::HBlank as u8,
            vram: Vram::default(),
            wx: 0,
            wy: 0,
            d: display::Display::default(),
            skip_units: 0,
        }
    }
}

// IO
impl Ppu {
    #[must_use]
    pub fn new(model: Model) -> Self {
        // Not deterministic on real hardware, but 00 (CGB) and FF (DMG) are
        // by far the most common power-on values.
        let obp = if model.is_cgb_hardware() { 0x00 } else { 0xFF };
        let mut ppu = Self {
            model,
            obp0: obp,
            obp1: obp,
            ..Self::default()
        };
        if model.is_cgb_hardware() {
            ppu.bcp.init_compat_palette();
            ppu.ocp.init_compat_palette();
        }
        ppu
    }

    #[must_use]
    pub const fn bcp(&self) -> &ColorPalette {
        &self.bcp
    }

    #[must_use]
    pub const fn bcp_mut(&mut self) -> &mut ColorPalette {
        &mut self.bcp
    }

    /// The HBlank HDMA request has been raised (the STAT mode bits turn to 0
    /// a couple of dots before).
    #[must_use]
    pub const fn hdma_period(&self) -> bool {
        matches!(self.mode(), Mode::HBlank) && self.d.hblank_hdma_pending() == 0
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        match self.stat & STAT_MODE_B {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            _ => Mode::Drawing,
        }
    }

    #[must_use]
    pub const fn ocp(&self) -> &ColorPalette {
        &self.ocp
    }

    #[must_use]
    pub const fn ocp_mut(&mut self) -> &mut ColorPalette {
        &mut self.ocp
    }

    #[must_use]
    pub const fn pixel_data_rgba(&self) -> &[u8] {
        self.rgba_buf_present.pixel_data()
    }

    #[must_use]
    pub const fn read_bgp(&self) -> u8 {
        self.bgp
    }

    #[must_use]
    pub const fn read_lcdc(&self) -> u8 {
        self.lcdc
    }

    /// LY as the CPU reads it.
    #[must_use]
    pub fn cpu_read_ly(&self) -> u8 {
        let Some((ly, t)) = self.line_position() else {
            return self.ly;
        };
        let ds = self.double_speed();
        if ly == 153 {
            if !ds || t <= 2 * LINE_CYCLES - 2 {
                0
            } else {
                153
            }
        } else if t <= 6 + 4 * i32::from(ds) {
            let next = ly + 1;
            if t == 6 + 4 * i32::from(ds) {
                ly & next
            } else {
                next
            }
        } else {
            ly
        }
    }

    /// STAT as the CPU reads it.
    #[must_use]
    pub fn cpu_read_stat(&self) -> u8 {
        let stat = self.read_stat();
        let Some((ly, t)) = self.line_position() else {
            return stat;
        };
        // The line the LY=LYC flag compares with, and the time left until
        // that comparison changes again.
        let ds = i32::from(self.double_speed());
        let line_time = LINE_CYCLES << ds;
        let (cmp_ly, left) = if ly == 153 {
            let left = t - (line_time - 6 - 6 * ds);
            if left <= 0 {
                (0, left + line_time)
            } else {
                (153, left)
            }
        } else {
            let left = t - (2 + 2 * ds);
            if left <= 0 {
                (ly + 1, left + line_time)
            } else {
                (ly, left)
            }
        };
        if cmp_ly == self.lyc && left > 2 {
            stat | STAT_LYC_B
        } else {
            stat & !STAT_LYC_B
        }
    }

    /// The line and the cycles left until it ends, on gambatte's clock (a
    /// CGB in single speed counts dots, in double speed half dots); `None`
    /// where the line timing is not the regular one.
    fn line_position(&self) -> Option<(u8, i32)> {
        if !self.gambatte_stat() || self.lcdc & LCDC_ON_B == 0 || !self.d.line_clock_valid() {
            return None;
        }
        // Half dots since this line began, counted at the read; LY changes
        // 19 half dots into the line on gambatte's clock.
        let lc = self.d.line_clock().rem_euclid(LINE_CYCLES);
        let tau = 2 * lc + i32::from(!self.d.half_dot());
        let line = self.d.current_line();
        let line = if tau >= 4 { line } else { (line + 1) % 154 };
        let (ly, h) = if tau < 19 {
            (
                if line == 0 { 153 } else { line - 1 },
                tau - 19 + 2 * LINE_CYCLES,
            )
        } else {
            (line, tau - 19)
        };
        let left = 2 * LINE_CYCLES - h;
        Some((
            ly,
            if self.double_speed() {
                left
            } else {
                (left + 1) / 2
            },
        ))
    }

    /// `position_in_line` as a signed value (-16..=160).
    #[must_use]
    pub const fn fifo_position(&self) -> i16 {
        let p = self.d.position_in_line();
        if p >= 240 { p as i16 - 256 } else { p as i16 }
    }

    #[inline]
    pub const fn set_tile_sel_glitch(&mut self, active: bool) {
        self.d.set_tile_sel_glitch(active);
    }

    /// Set while a CPU write to WX is landing (SameBoy's `wx_just_changed`).
    #[inline]
    pub const fn set_wx_just_changed(&mut self, active: bool) {
        self.d.set_wx_just_changed(active);
    }

    /// Set while a CPU write that turns the window on is landing.
    #[inline]
    pub const fn set_window_enable_pending(&mut self, active: bool) {
        self.d.set_window_enable_pending(active);
    }

    #[must_use]
    pub const fn is_fetching_sprite(&self) -> bool {
        self.d.fetching_object()
    }

    /// Whether `HBlank` was entered since the last call.
    pub const fn take_hblank_hdma_edge(&mut self) -> bool {
        self.d.take_hblank_hdma_edge()
    }

    /// Whether the LCD was switched off in a non-zero mode since the last call.
    pub const fn take_lcd_off_hdma_edge(&mut self) -> bool {
        self.d.take_lcd_off_hdma_edge()
    }

    /// The PPU is at the edge between HBlank and the OAM scan (SameBoy's
    /// display state 7), where some register writes behave differently.
    #[must_use]
    pub fn at_oam_scan_edge(&self) -> bool {
        self.d.state() == display::State::OamScanStart
    }

    #[must_use]
    pub const fn read_lyc(&self) -> u8 {
        self.lyc
    }

    #[must_use]
    pub const fn read_obp0(&self) -> u8 {
        self.obp0
    }

    #[must_use]
    pub const fn read_obp1(&self) -> u8 {
        self.obp1
    }

    #[must_use]
    pub const fn read_opri(&self) -> u8 {
        self.opri as u8 | 0xFE
    }

    #[must_use]
    pub const fn read_scx(&self) -> u8 {
        self.scx
    }

    #[must_use]
    pub const fn read_scy(&self) -> u8 {
        self.scy
    }

    #[must_use]
    pub const fn read_stat(&self) -> u8 {
        self.stat | 0x80
    }

    #[must_use]
    pub const fn read_wx(&self) -> u8 {
        self.wx
    }

    #[must_use]
    pub const fn read_wy(&self) -> u8 {
        self.wy
    }

    /// Advance the PPU by one dot.
    /// One CPU T-cycle: a PPU dot in single speed, half of one in double
    /// speed.
    pub fn tick_t_cycle(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        self.cgb_mode = cgb_mode;
        self.double_speed = double_speed;
        self.count_chunk_cycle();
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }
        for _ in 0..if double_speed { 1 } else { 2 } {
            if self.skip_units > 0 {
                self.skip_units -= 1;
                continue;
            }
            self.run_display(ints);
        }
    }

    /// Runs the display `cycles` T-cycles ahead of the CPU, which will not
    /// run those cycles again. Unless `any_line`, only done around the end
    /// of the frame.
    pub fn run_ahead(
        &mut self,
        ints: &mut Interrupts,
        cgb_mode: CgbMode,
        double_speed: bool,
        cycles: i32,
        any_line: bool,
    ) {
        if self.lcdc & LCDC_ON_B == 0 || (self.d.current_line() < 152 && !any_line) {
            return;
        }
        for _ in 0..cycles {
            self.tick_t_cycle(ints, cgb_mode, double_speed);
        }
        let units = cycles * if double_speed { 1 } else { 2 };
        self.skip_units = self
            .skip_units
            .saturating_add(u8::try_from(units).unwrap_or(u8::MAX));
    }

    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.color_correction_mode = mode;
    }

    pub fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts, _is_cgb: bool) {
        let was_on = self.lcdc & LCDC_ON_B != 0;
        let is_on = val & LCDC_ON_B != 0;

        if is_on && !was_on {
            // With LYC 0 the LY=LYC condition starts with the LCD, unless the
            // flag stayed set while it was off.
            if self.gambatte_stat()
                && self.stat & (STAT_IF_LYC_B | STAT_LYC_B) == STAT_IF_LYC_B
                && self.lyc == 0
            {
                ints.request_lcd();
            }
            self.d.restart();
            self.gstat_lcd_on();
        } else if !is_on && was_on {
            self.lcd_off();
            self.gstat_lcd_off();
            self.rgba_buf_present.clear();
        } else {
            // The LCD stays as it is.
        }

        // Disabling objects while an object is being fetched aborts the
        // fetch on non-CGB hardware.
        self.abort_object_fetch_on_obj_disable(val);
        if self.hw_cgb() && self.lcdc & 0x20 != 0 && val & 0x20 == 0 {
            self.cancel_window_start();
        }

        self.lcdc = val;
        self.set_obj_size_fetch(val & 0x04 != 0);
        self.d.schedule_wy_check();
    }

    pub fn write_lyc(&mut self, val: u8, ints: &mut Interrupts) {
        self.write_lyc_reg(val, ints);
    }

    pub const fn write_bgp(&mut self, val: u8) {
        self.bgp = val;
    }

    pub const fn write_obp0(&mut self, val: u8) {
        self.obp0 = val;
    }

    pub const fn write_obp1(&mut self, val: u8) {
        self.obp1 = val;
    }

    pub const fn write_opri(&mut self, val: u8) {
        self.opri = val & 1 != 0;
    }

    pub const fn write_scx(&mut self, val: u8) {
        self.scx = val;
    }

    /// Place the PPU `dot` dots into `line` (post-boot state injection). The
    /// PPU is restarted as if the LCD had just been turned on and run forward.
    pub fn set_position(&mut self, line: u8, dot: i32) {
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }
        let mut ints = Interrupts::default();
        self.d.restart();
        self.gstat_lcd_on();
        // `line_clock` restarts on every visible line and keeps running
        // through VBlank, starting from line 144.
        let base = i32::from(line.saturating_sub(144)) * LINE_LENGTH_DOTS;
        for _ in 0..(2 * 154 * 456) {
            if self.d.current_line() == line && self.d.line_clock() - base >= dot {
                break;
            }
            self.run_display(&mut ints);
        }
        // Finish the dot the loop stopped in.
        if self.d.half_dot() {
            self.run_display(&mut ints);
        }
    }

    pub const fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    pub fn write_stat(&mut self, val: u8, ints: &mut Interrupts, _is_cgb: bool) {
        self.write_stat_reg(val, ints);
    }

    pub fn write_wx(&mut self, val: u8) {
        self.wx = val;
    }

    pub fn write_wy(&mut self, val: u8) {
        self.wy = val;
        self.d.schedule_wy_check();
    }

    /// CGB palette RAM is blocked from the CPU while the PPU reads it.
    #[inline]
    #[must_use]
    pub fn is_cgb_palettes_accessible(&self) -> bool {
        !self
            .gstat_mode3_lock(80)
            .unwrap_or_else(|| self.d.cpu().cgb_palettes_blocked)
    }

    /// STOP-mode hooks (the PPU engine does not distinguish STOP yet).
    #[inline]
    pub const fn enter_stop_mode(&mut self) {
        self.block_ppu_accesses(true);
    }

    #[inline]
    pub const fn leave_stop_mode(&mut self) {
        self.block_ppu_accesses(false);
    }
}

const LINE_LENGTH_DOTS: i32 = display::LINE_LENGTH;
