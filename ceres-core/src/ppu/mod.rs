mod color_palette;
mod display;
mod draw;
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

    #[must_use]
    pub const fn read_ly(&self) -> u8 {
        self.ly
    }

    /// `position_in_line` as a signed value (-16..=160).
    #[must_use]
    pub const fn fifo_position(&self) -> i16 {
        let p = self.d.position_in_line;
        if p >= 240 { p as i16 - 256 } else { p as i16 }
    }

    #[inline]
    pub const fn set_tile_sel_glitch(&mut self, active: bool) {
        self.d.tile_sel_glitch = active;
    }

    /// Set while a CPU write to WX is landing (SameBoy's `wx_just_changed`).
    #[inline]
    pub const fn set_wx_just_changed(&mut self, active: bool) {
        self.d.wx_just_changed = active;
    }

    #[must_use]
    pub const fn is_fetching_sprite(&self) -> bool {
        self.d.during_object_fetch
    }

    /// Whether `HBlank` was entered since the last call.
    pub const fn take_hblank_hdma_edge(&mut self) -> bool {
        core::mem::replace(&mut self.d.hblank_hdma_edge, false)
    }

    /// Whether the LCD was switched off in a non-zero mode since the last call.
    pub const fn take_lcd_off_hdma_edge(&mut self) -> bool {
        core::mem::replace(&mut self.d.lcd_off_hdma_edge, false)
    }

    /// SameBoy's `display_state`, for state-dependent register-write hacks.
    #[must_use]
    pub const fn display_state(&self) -> u8 {
        self.d.state
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
    pub fn tick_t_cycle(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        self.cgb_mode = cgb_mode;
        self.double_speed = double_speed;
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }
        self.run_display(ints);
    }

    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.color_correction_mode = mode;
    }

    pub fn write_lcdc(&mut self, val: u8, _ints: &mut Interrupts, _is_cgb: bool) {
        let was_on = self.lcdc & LCDC_ON_B != 0;
        let is_on = val & LCDC_ON_B != 0;

        if is_on && !was_on {
            self.d.restart();
        } else if !is_on && was_on {
            self.lcd_off();
            self.rgba_buf_present.clear();
        }

        // Disabling objects while an object is being fetched aborts the
        // fetch on non-CGB hardware.
        self.abort_object_fetch_on_obj_disable(val);

        self.lcdc = val;
        self.d.fetch_obj_size = val & 0x04 != 0;
        self.d
            .schedule_wy_check(self.model.is_cgb_hardware(), self.double_speed);
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
        // `line_clock` restarts on every visible line and keeps running
        // through VBlank, starting from line 144.
        let base = i32::from(line.saturating_sub(144)) * LINE_LENGTH_DOTS;
        for _ in 0..(2 * 154 * 456) {
            if self.d.current_line == line && self.d.line_clock - base >= dot {
                break;
            }
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
        self.d
            .schedule_wy_check(self.model.is_cgb_hardware(), self.double_speed);
    }

    /// CGB palette RAM is blocked from the CPU while the PPU reads it.
    #[inline]
    #[must_use]
    pub const fn is_cgb_palettes_accessible(&self) -> bool {
        !self.d.cgb_palettes_blocked
    }

    /// STOP-mode hooks (the PPU engine does not distinguish STOP yet).
    #[inline]
    pub const fn enter_stop_mode(&mut self) {}

    #[inline]
    pub const fn leave_stop_mode(&mut self) {}
}

const LINE_LENGTH_DOTS: i32 = display::LINE_LENGTH;
