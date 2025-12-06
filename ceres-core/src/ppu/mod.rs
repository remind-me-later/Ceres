mod color_palette;
mod fetcher;
mod fifo;
mod oam;
mod rgba_buf;
mod sprite;
mod vram;

use core::mem;

use crate::interrupts::Interrupts;
pub use oam::Oam;
pub use vram::Vram;
use {
    self::color_palette::ColorPalette,
    crate::CgbMode,
    fetcher::{FetcherState, SpriteFetcherState},
    fifo::PixelFifo,
    rgba_buf::RgbaBuf,
    sprite::SpriteBuffer,
};

// =============================================================================
// PPU Timing Constants
// Reference: SameBoy display.c (https://github.com/LIJI32/SameBoy)
//
// Each line is 456 cycles. Without scrolling, objects or a window:
//   Mode 2 - 80  cycles / OAM Transfer
//   Mode 3 - 172 cycles / Rendering
//   Mode 0 - 204 cycles / HBlank
//   Mode 1 is VBlank
// =============================================================================

/// Length of OAM scan (Mode 2) in T-cycles.
/// SameBoy: #define MODE2_LENGTH (80)
#[allow(dead_code)]
const MODE2_LENGTH: i32 = 80;

/// Total T-cycles per scanline.
/// SameBoy: #define LINE_LENGTH (456)
const LINE_LENGTH: i32 = 456;

/// Number of visible scanlines.
/// SameBoy: #define LINES (144)
pub const LINES: u8 = 144;

/// Screen width in pixels.
/// SameBoy: #define WIDTH (160)
pub const WIDTH: u8 = 160;

/// Total T-cycles per frame.
/// SameBoy: #define LCDC_PERIOD 70224
/// Calculated as: VIRTUAL_LINES (154) * LINE_LENGTH (456)
#[allow(dead_code)]
const LCDC_PERIOD: u32 = 70224;

/// Total scanlines per frame (visible + vblank).
/// SameBoy: #define VIRTUAL_LINES (LCDC_PERIOD / LINE_LENGTH) // = 154
const VIRTUAL_LINES: u8 = 154;

/// Base Mode 3 (Drawing) length without sprites/window/scrolling.
/// SameBoy: Mode 3 - 172 cycles / Rendering (base case)
const MODE3_BASE_LENGTH: i32 = 172;

/// Base Mode 0 (HBlank) length.
/// SameBoy: Mode 0 - 204 cycles / HBlank (base case)
#[allow(dead_code)]
const MODE0_BASE_LENGTH: i32 = 204;

// Legacy aliases for compatibility
pub const PX_WIDTH: u8 = WIDTH;
pub const PX_HEIGHT: u8 = LINES;

// =============================================================================
// LCDC Register Bits
// Reference: SameBoy gb.h GB_LCDC_* constants
// =============================================================================

/// LCDC bit 0: BG/Window enable (DMG) / Master priority (CGB).
/// SameBoy: GB_LCDC_BG_EN = 1
const LCDC_BG_EN: u8 = 0x01;

/// LCDC bit 1: OBJ (sprite) enable.
/// SameBoy: GB_LCDC_OBJ_EN = 2
const LCDC_OBJ_EN: u8 = 0x02;

/// LCDC bit 2: OBJ size (0=8x8, 1=8x16).
/// SameBoy: GB_LCDC_OBJ_SIZE = 4
const LCDC_OBJ_SIZE: u8 = 0x04;

/// LCDC bit 3: BG tile map select (0=9800-9BFF, 1=9C00-9FFF).
/// SameBoy: GB_LCDC_BG_MAP = 8
const LCDC_BG_MAP: u8 = 0x08;

/// LCDC bit 4: BG/Window tile data select (0=8800-97FF, 1=8000-8FFF).
/// SameBoy: GB_LCDC_TILE_SEL = 0x10
const LCDC_TILE_SEL: u8 = 0x10;

/// LCDC bit 5: Window enable.
/// SameBoy: GB_LCDC_WIN_ENABLE = 0x20
const LCDC_WIN_ENABLE: u8 = 0x20;

/// LCDC bit 6: Window tile map select (0=9800-9BFF, 1=9C00-9FFF).
/// SameBoy: GB_LCDC_WIN_MAP = 0x40
const LCDC_WIN_MAP: u8 = 0x40;

/// LCDC bit 7: LCD enable.
/// SameBoy: GB_LCDC_ENABLE = 0x80
const LCDC_ENABLE: u8 = 0x80;

// =============================================================================
// STAT Register Bits
// =============================================================================

/// STAT bits 0-1: Mode flag (read-only).
const STAT_MODE: u8 = 0x03;

/// STAT bit 2: LY=LYC coincidence flag (read-only).
const STAT_LYC: u8 = 0x04;

/// STAT bit 3: Mode 0 (HBlank) interrupt enable.
const STAT_IF_HBLANK: u8 = 0x08;

/// STAT bit 4: Mode 1 (VBlank) interrupt enable.
const STAT_IF_VBLANK: u8 = 0x10;

/// STAT bit 5: Mode 2 (OAM scan) interrupt enable.
const STAT_IF_OAM: u8 = 0x20;

/// STAT bit 6: LY=LYC interrupt enable.
const STAT_IF_LYC: u8 = 0x40;

/// LCD startup state machine states.
/// Reference: SameBoy display.c GB_display_run() states 2, 34, 37, 38
///
/// When LCD is enabled (LCDC bit 7 set), the PPU goes through a specific
/// startup sequence before normal rendering begins.
/// Each phase has a countdown timer for its duration.
/// Total: 76 + 2 + 2 + 3 = 83 cycles before Mode 3 starts.
///
/// SameBoy timing:
/// - State 2 (76 cycles): MODE2_LENGTH - 4, Mode 0 in STAT
/// - State 34 (2 cycles): OAM write blocked
/// - State 37 (2 cycles): STAT = Mode 3, OAM fully blocked
/// - State 38 (3 cycles): CGB palettes blocked, then enter Mode 3
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StartupState {
    /// LCD is off or startup complete.
    #[default]
    Inactive,
    /// Phase 1: Initial wait (76 cycles). Mode 0 in STAT, memory access unblocked.
    /// SameBoy: State 2, duration = MODE2_LENGTH - 4 = 76
    Phase1(u8),
    /// Phase 2: Wait 2 cycles. OAM write access blocked.
    /// SameBoy: State 34
    Phase2(u8),
    /// Phase 3: Wait 2 cycles. STAT = Mode 3, OAM/VRAM blocked.
    /// SameBoy: State 37
    Phase3(u8),
    /// Phase 4: Wait 3 cycles. CGB palettes blocked, then enter Mode 3 rendering.
    /// SameBoy: State 38 + mode_3_start
    Phase4(u8),
}

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

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Order follows the state machine transitions"
)]
#[derive(Clone, Copy, Debug, Default)]
pub enum Mode {
    #[default]
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

impl Mode {
    /// Returns the base duration in T-cycles for this PPU mode.
    ///
    /// Reference: SameBoy display.c timing comments:
    /// - Mode 2 (OAM scan): 80 cycles
    /// - Mode 3 (Drawing): 172 cycles base (varies with sprites/scroll/window)
    /// - Mode 0 (HBlank): 204 cycles base (varies as Mode 3 varies)
    /// - Mode 1 (VBlank): 456 cycles per line (10 lines total)
    pub fn dots(self, _scroll_x: u8) -> i32 {
        // SameBoy: MODE2_LENGTH (80), but we use a slightly adjusted value
        // due to differences in when state transitions occur
        const OAM_SCAN_DOTS: i32 = 89;
        // SameBoy: Mode 3 - 172 cycles base (varies with content)
        const DRAWING_DOTS: i32 = MODE3_BASE_LENGTH;
        // HBlank fills remaining time: LINE_LENGTH - MODE2_LENGTH - MODE3
        // SameBoy: Mode 0 - 204 cycles base
        const HBLANK_DOTS: i32 = 200;
        // VBlank is LINE_LENGTH per line
        // SameBoy: LINE_LENGTH (456)
        const VBLANK_DOTS: i32 = LINE_LENGTH;

        // Note: SCX handling affects Mode 3 duration via pixel discarding,
        // not directly through this function.
        match self {
            Self::OamScan => OAM_SCAN_DOTS,
            Self::Drawing => DRAWING_DOTS,
            Self::HBlank => HBLANK_DOTS,
            Self::VBlank => VBLANK_DOTS,
        }
    }
}

#[expect(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct Ppu {
    bcp: ColorPalette,
    bgp: u8,
    color_correction_mode: ColorCorrectionMode,
    delay_one_frame: bool,
    /// LCD startup state machine state (includes phase countdown).
    startup_state: StartupState,
    lcdc: u8,
    ly: u8,
    /// LY value used for LYC comparison (may differ from displayed LY during transitions)
    ly_for_comparison: u8,
    lyc: u8,
    mode: Mode,
    oam: Oam,
    obp0: u8,
    obp1: u8,
    ocp: ColorPalette,
    opri: bool,
    remaining_dots_in_mode: i32,
    rgb_buf: RgbaBuf,
    rgba_buf_present: RgbaBuf,
    scx: u8,
    scy: u8,
    stat: u8,
    /// Internal STAT interrupt line - OR of all enabled STAT sources.
    /// Used to implement edge-triggered interrupt behavior.
    stat_interrupt_line: bool,
    /// Mode used for interrupt purposes (can differ from STAT mode bits by 1-2 cycles).
    /// SameBoy uses this to fire Mode 2 interrupt slightly before STAT shows Mode 2.
    /// Value of -1 (represented as None) means no mode-based interrupt should fire.
    mode_for_interrupt: Option<Mode>,
    vram: Vram,
    win_in_frame: bool,
    win_in_ly: bool,
    win_skipped: u8,
    wx: u8,
    wy: u8,
    /// Pending LY value (delayed write for sub-cycle accuracy).
    ly_pending: u8,
    /// Cycles until pending LY write takes effect (0 = no pending write).
    ly_write_delay: u8,

    /// Background/window pixel FIFO (8-pixel capacity).
    bg_fifo: PixelFifo,
    /// Sprite (OAM) pixel FIFO (8-pixel capacity).
    oam_fifo: PixelFifo,
    /// Background fetcher state machine state.
    fetcher_state: FetcherState,
    /// Current tile X coordinate being fetched.
    fetcher_tile_x: u8,
    /// Current tile index being fetched.
    current_tile: u8,
    /// Current tile attributes (CGB only).
    current_tile_attrs: u8,
    /// Tile data bytes (low and high).
    current_tile_data: [u8; 2],
    /// Sprites visible on current scanline.
    sprite_buffer: SpriteBuffer,
    /// Current dot within scanline (0-455).
    dots_in_line: u16,
    /// LCD X position being rendered (-8 to 167, negative = scroll discard phase).
    position_in_line: i16,
    /// Actual LCD X coordinate (0-159).
    lcd_x: u8,
    /// Window has been triggered on this scanline.
    window_triggered: bool,
    /// Window internal line counter.
    window_line: u8,
    /// OAM read access blocked.
    oam_read_blocked: bool,
    /// OAM write access blocked.
    oam_write_blocked: bool,
    /// VRAM read access blocked.
    vram_read_blocked: bool,
    /// VRAM write access blocked.
    vram_write_blocked: bool,
    /// CGB palette access blocked.
    cgb_palettes_blocked: bool,
    /// Cached tile index address (calculated in T1, used in T2).
    fetcher_tile_index_addr: u16,
    /// Cached tile data address (calculated in T1, used in T2).
    fetcher_tile_data_addr: u16,
    /// OAM scan index (0-39, which OAM entry is being checked).
    oam_scan_index: u8,
    /// Last X coordinate where sprites were fetched (to prevent re-triggering).
    last_fetched_x: i16,
    /// Window is currently being fetched (used for SCX edge case).
    window_is_being_fetched: bool,
    /// Window activation delay counter (cycles to wait after window activates).
    window_activation_delay: u8,
    /// Line has fractional scrolling (SCX & 7 != 0).
    line_has_fractional_scrolling: bool,
    /// Sprite fetcher state machine state.
    sprite_fetcher_state: SpriteFetcherState,
    /// Sprite fetcher sub-cycle counter.
    sprite_fetcher_step: u8,
    /// Current sprite being fetched (index into sprite_buffer).
    current_sprite_index: usize,
    /// Current sprite's tile data low byte.
    sprite_tile_data_low: u8,
    /// Current sprite's tile data high byte.
    sprite_tile_data_high: u8,
    /// Current sprite's calculated VRAM address.
    sprite_tile_address: u16,
    /// Current sprite's VRAM bank (CGB).
    sprite_vram_bank: u8,
    /// Current sprite's palette.
    sprite_palette: u8,
    /// Current sprite's priority value.
    sprite_priority: u8,
    /// Current sprite's BG priority flag.
    sprite_bg_priority: bool,
    /// Current sprite's X-flip flag.
    sprite_x_flip: bool,
    /// First frame after LCD was enabled (special timing for line 0).
    first_frame_after_lcd_on: bool,
    /// HBlank interrupt pending (fires on first tick of HBlank, after state 22 delay).
    hblank_interrupt_pending: bool,
}

// IO
impl Ppu {
    #[must_use]
    pub const fn bcp(&self) -> &ColorPalette {
        &self.bcp
    }

    #[must_use]
    pub const fn bcp_mut(&mut self) -> &mut ColorPalette {
        &mut self.bcp
    }

    /// Update the STAT interrupt line and fire interrupt on rising edge.
    /// This implements proper STAT IRQ blocking - only fires when line transitions low->high.
    fn update_stat(&mut self, ints: &mut Interrupts) {
        let previous_line = self.stat_interrupt_line;

        // Update LY=LYC coincidence flag based on comparison value
        self.stat &= !STAT_LYC;
        if self.ly_for_comparison == self.lyc {
            self.stat |= STAT_LYC;
        }

        // Compute new STAT interrupt line state from all enabled sources
        let mut new_line = false;

        // LY=LYC coincidence interrupt
        if (self.stat & STAT_IF_LYC != 0) && (self.stat & STAT_LYC != 0) {
            new_line = true;
        }

        // Mode-based interrupts use mode_for_interrupt (which can differ from STAT bits)
        // SameBoy: mode_for_interrupt can be set to 2 at the end of HBlank, before STAT changes
        // If mode_for_interrupt is None, fall back to the actual STAT mode bits
        let interrupt_mode = self.mode_for_interrupt.unwrap_or_else(|| self.mode());
        match interrupt_mode {
            Mode::HBlank if self.stat & STAT_IF_HBLANK != 0 => new_line = true,
            Mode::VBlank if self.stat & STAT_IF_VBLANK != 0 => new_line = true,
            Mode::OamScan if self.stat & STAT_IF_OAM != 0 => new_line = true,
            _ => {}
        }

        self.stat_interrupt_line = new_line;

        // Only fire interrupt on rising edge (low -> high transition)
        if new_line && !previous_line {
            ints.request_lcd();
        }
    }

    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
        self.mode = mode;

        // For OamScan, delay STAT update (handled in tick_oam_scan)
        if !matches!(mode, Mode::OamScan) {
            self.set_mode_stat(mode);
        }

        // mode_for_interrupt will fallback to actual mode via unwrap_or_else
        // Only set explicitly when we need to differ from STAT mode bits
        self.mode_for_interrupt = None;
        self.remaining_dots_in_mode += self.mode().dots(self.scx);

        if matches!(mode, Mode::Drawing) {
            self.last_fetched_x = -1;
        }

        // VBlank always fires its own interrupt (separate from STAT)
        if matches!(mode, Mode::VBlank) {
            ints.request_vblank();
            self.win_skipped = 0;
            self.win_in_frame = false;

            // DMG/MGB/SGB quirk: When entering VBlank at line LINES (144), the OAM interrupt
            // (STAT bit 5) also triggers a STAT interrupt if enabled.
            // This is because the Mode 2 condition is briefly asserted at VBlank entry.
            // See: vblank_stat_intr-GS test
            // SameBoy: LINES (144)
            if self.ly == LINES && !self.stat_interrupt_line && (self.stat & STAT_IF_OAM != 0) {
                ints.request_lcd();
            }
        }

        if matches!(mode, Mode::OamScan) {
            self.win_in_ly = false;
        }

        // Update STAT interrupt line (fires on rising edge)
        self.update_stat(ints);
    }

    /// Schedule a delayed LY write for sub-cycle timing accuracy.
    /// The write becomes visible after `delay` T-cycles.
    /// Use delay=0 for immediate writes.
    #[inline]
    fn schedule_ly_write(&mut self, value: u8, delay: u8) {
        if delay == 0 {
            self.ly = value;
        } else {
            self.ly_pending = value;
            self.ly_write_delay = delay;
        }
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
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
        // Compute mode from dots_in_line for accurate timing
        // This matches Age's approach of calculating mode on read
        let computed_mode = self.compute_stat_mode();
        (self.stat & !STAT_MODE) | computed_mode | 0x80
    }

    /// Compute STAT mode based on current timing state.
    /// This provides more accurate mode reporting than the maintained state
    /// for the first line after LCD enable (which has special timing).
    const fn compute_stat_mode(&self) -> u8 {
        // LCD off: mode 0
        if self.lcdc & LCDC_ENABLE == 0 {
            return 0;
        }

        // First line after LCD on has special timing (Age: is_line_zero)
        // Mode 0 is shown for first 82 cycles instead of Mode 2
        // Only apply this special case for the actual first frame after LCD enable
        if self.first_frame_after_lcd_on && self.ly == 0 {
            // During startup state machine, use the stored STAT mode bits
            if !matches!(self.startup_state, StartupState::Inactive) {
                return self.stat & STAT_MODE;
            }
        }

        // For all other cases, use the stored STAT mode bits
        self.stat & STAT_MODE
    }

    #[must_use]
    pub const fn read_wx(&self) -> u8 {
        self.wx
    }

    #[must_use]
    pub const fn read_wy(&self) -> u8 {
        self.wy
    }

    pub fn run(&mut self, dots: i32, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        for _ in 0..dots {
            self.tick(ints, cgb_mode, double_speed);
        }
    }

    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.color_correction_mode = mode;
    }

    const fn set_mode_stat(&mut self, mode: Mode) {
        self.stat = (self.stat & !STAT_MODE) | mode as u8;
    }

    /// Returns true if OAM is accessible by the CPU.
    ///
    /// OAM is blocked during Mode 2 (OAM scan) and Mode 3 (drawing).
    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub const fn is_oam_read_accessible(&self) -> bool {
        // OAM is accessible when LCD is off or read not blocked
        self.lcdc & LCDC_ENABLE == 0 || !self.oam_read_blocked
    }

    /// Returns true if OAM is writable by the CPU.
    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub const fn is_oam_write_accessible(&self) -> bool {
        // OAM is writable when LCD is off or write not blocked
        self.lcdc & LCDC_ENABLE == 0 || !self.oam_write_blocked
    }

    /// Returns true if VRAM is readable by the CPU.
    ///
    /// VRAM is blocked during Mode 3 (drawing).
    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub const fn is_vram_read_accessible(&self) -> bool {
        // VRAM is accessible when LCD is off or read not blocked
        self.lcdc & LCDC_ENABLE == 0 || !self.vram_read_blocked
    }

    /// Returns true if VRAM is writable by the CPU.
    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub const fn is_vram_write_accessible(&self) -> bool {
        // VRAM is writable when LCD is off or write not blocked
        self.lcdc & LCDC_ENABLE == 0 || !self.vram_write_blocked
    }

    /// Returns true if CGB palettes (BCP/OCP) are accessible by the CPU.
    #[inline]
    #[must_use]
    pub const fn is_cgb_palettes_accessible(&self) -> bool {
        // CGB palettes are accessible when LCD is off or not blocked
        self.lcdc & LCDC_ENABLE == 0 || !self.cgb_palettes_blocked
    }

    /// Advance PPU by one T-cycle (dot).
    #[inline]
    pub fn tick(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        if self.lcdc & LCDC_ENABLE == 0 {
            return;
        }

        // Process pending LY write (for sub-cycle timing accuracy)
        if self.ly_write_delay > 0 {
            self.ly_write_delay -= 1;
            if self.ly_write_delay == 0 {
                self.ly = self.ly_pending;
            }
        }

        // Handle LCD startup state machine
        if self.startup_state != StartupState::Inactive {
            self.tick_startup(ints, cgb_mode, double_speed);
            return;
        }

        // SameBoy: cycles_for_line increment happens at different points per mode
        // For Mode 3: increment happens AFTER render/fetch work, ONLY if position != 160
        // For other modes: increment happens at start of each cycle
        match self.mode() {
            Mode::Drawing => {
                // Mode 3: Don't pre-increment - tick_drawing handles it SameBoy-style
                self.tick_drawing(ints, cgb_mode);
            }
            _ => {
                // Other modes: Pre-increment like before
                self.dots_in_line += 1;
                self.remaining_dots_in_mode -= 1;
                match self.mode() {
                    Mode::OamScan => self.tick_oam_scan(ints, cgb_mode, double_speed),
                    Mode::HBlank => self.tick_hblank(ints, double_speed),
                    Mode::VBlank => self.tick_vblank(ints),
                    Mode::Drawing => unreachable!(),
                }
            }
        }
    }

    /// Tick the LCD startup state machine.
    /// SameBoy timing for first line after LCD on:
    /// - Phase 1 (76 cycles): Mode 0 in STAT, all unblocked
    /// - Phase 2 (2 cycles): OAM write blocked
    /// - Phase 3 (2 cycles): STAT = Mode 3, OAM fully blocked, VRAM blocked (DMG), CGB palettes blocked
    /// - Phase 4 (3 cycles): VRAM fully blocked, enter Mode 3 rendering
    /// Total: 83 cycles
    fn tick_startup(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        let is_cgb = matches!(cgb_mode, CgbMode::Cgb);

        // Track dots during startup for computed STAT mode
        self.dots_in_line += 1;

        match self.startup_state {
            StartupState::Inactive => {}

            StartupState::Phase1(remaining) => {
                // Phase 1: Mode 0 in STAT, all unblocked
                if remaining <= 1 {
                    // Transition to Phase 2
                    self.startup_state = StartupState::Phase2(2);
                } else {
                    self.startup_state = StartupState::Phase1(remaining - 1);
                }
            }

            StartupState::Phase2(remaining) => {
                // Phase 2: OAM write blocked (set on first cycle of Phase2)
                if remaining == 2 {
                    self.oam_write_blocked = true;
                    // SameBoy calls GB_STAT_update here (cycle 77)
                    self.update_stat(ints);
                }
                if remaining <= 1 {
                    // Transition to Phase 3: STAT = Mode 3, OAM fully blocked, CGB palettes blocked
                    self.set_mode_stat(Mode::Drawing);
                    self.oam_read_blocked = true;
                    // VRAM blocking depends on CGB/DMG
                    if !is_cgb {
                        self.vram_read_blocked = true;
                        self.vram_write_blocked = true;
                    } else if double_speed {
                        self.vram_read_blocked = true;
                        self.vram_write_blocked = true;
                    }

                    self.update_stat(ints);
                    self.startup_state = StartupState::Phase3(2);
                } else {
                    self.startup_state = StartupState::Phase2(remaining - 1);
                }
            }

            StartupState::Phase3(remaining) => {
                // Phase 3: STAT = Mode 3, all blocked except VRAM (CGB)
                if remaining <= 1 {
                    // Transition to Phase 4: CGB palettes blocked
                    self.cgb_palettes_blocked = true;
                    self.startup_state = StartupState::Phase4(3);
                } else {
                    self.startup_state = StartupState::Phase3(remaining - 1);
                }
            }

            StartupState::Phase4(remaining) => {
                // Phase 4: All blocked, enter Mode 3 rendering
                if remaining <= 1 {
                    // Startup complete
                    // SameBoy: vram_read_blocked = true; vram_write_blocked = true; (at mode_3_start)
                    self.vram_read_blocked = true;
                    self.vram_write_blocked = true;

                    self.startup_state = StartupState::Inactive;
                    self.enter_mode3_after_startup(ints);

                    // SameBoy: After entering mode_3_start, immediately do first iteration
                    // of the Mode 3 loop (work + cycles_for_line++) before any sleep.
                    // This is important for cycle-accurate timing.
                    self.output_pixel(cgb_mode);
                    self.advance_fetcher(cgb_mode);
                    self.dots_in_line += 1;
                } else {
                    self.startup_state = StartupState::Phase4(remaining - 1);
                }
            }
        }
    }

    /// Enter Mode 3 rendering after startup sequence completes.
    fn enter_mode3_after_startup(&mut self, ints: &mut Interrupts) {
        self.mode = Mode::Drawing;
        // STAT already set to Mode 3 at dot 79
        self.remaining_dots_in_mode = Mode::Drawing.dots(self.scx);

        // Memory blocking already set during startup sequence

        self.last_fetched_x = -1;
        self.sprite_fetcher_state = SpriteFetcherState::Idle;

        // Initialize drawing state
        // SameBoy: cycles_for_line is augmented by 8 extra cycles for first line.
        // Startup duration 83 + 8 = 91.
        self.dots_in_line = 91;
        self.fetcher_state = FetcherState::GetTileT1;
        self.fetcher_tile_x = 0;
        // SameBoy: position_in_line starts at -16
        self.position_in_line = -16;
        self.lcd_x = 0;
        self.bg_fifo.clear();
        self.oam_fifo.clear();
        // Push 8 "junk" pixels to prime the FIFO (will be discarded during scroll)
        self.bg_fifo.push_bg_row(0, 0, 0, false, false);
        self.window_triggered = false;
        self.window_line = 0;
        // Reset per-line flags
        self.line_has_fractional_scrolling = false;
        self.window_is_being_fetched = false;
        self.window_activation_delay = 0;
        // Note: No OAM scan happened, so sprite_buffer stays empty for first line after LCD on

        // Clear sprite buffer and visible object count
        self.sprite_buffer.clear();

        self.update_stat(ints);
    }

    /// Tick during Mode 2 (OAM Scan).
    /// SameBoy timing:
    /// - Dot 1-2 (state 35): OAM write blocked (CGB && !double_speed)
    /// - Dot 3 (state 6): LY update, OAM read blocked (model dependent)
    /// - Dot 4 (state 7): STAT = Mode 2, OAM fully blocked
    /// - Dot 5-84: OAM scan (40 entries × 2 dots each)
    fn tick_oam_scan(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        let is_cgb = matches!(cgb_mode, CgbMode::Cgb);

        match self.dots_in_line {
            1 | 2 => {
                // State 35: OAM write blocked on CGB (non-double-speed only)
                // SameBoy: gb->oam_write_blocked = GB_is_cgb(gb) && !gb->cgb_double_speed;
                if is_cgb && !double_speed {
                    self.oam_write_blocked = true;
                }
            }
            3 => {
                // State 6: LY update, OAM read blocked
                self.ly = self.ly.wrapping_add(1);
                self.ly_for_comparison = self.ly;
                // SameBoy: gb->oam_read_blocked = !gb->cgb_double_speed || gb->model >= GB_MODEL_CGB_D;
                // For simplicity, block OAM read unless in double-speed mode
                self.oam_read_blocked = !double_speed;

                // SameBoy: "The OAM STAT interrupt occurs 1 T-cycle before STAT
                // actually changes, except on line 0. PPU glitch?"
                if self.ly != 0 {
                    self.mode_for_interrupt = Some(Mode::OamScan);
                    // STAT mode bits stay at 0 (HBlank) but interrupt uses mode 2
                }
                self.update_stat(ints);
            }
            4 => {
                // State 7: STAT = Mode 2, OAM fully blocked
                self.set_mode_stat(Mode::OamScan);
                self.oam_read_blocked = true;
                self.oam_write_blocked = true;
                self.ly_for_comparison = self.ly;

                // SameBoy: After STAT update, mode_for_interrupt is set to -1
                // (meaning no mode-based interrupt) to prevent double-firing
                self.mode_for_interrupt = Some(Mode::OamScan);
                self.update_stat(ints);
                self.mode_for_interrupt = None;
            }
            5..=84 => {
                // OAM scan: check one entry every 2 dots
                let scan_dot = self.dots_in_line - 4; // 1-80
                if scan_dot % 2 == 0 && self.oam_scan_index < 40 {
                    self.scan_oam_entry();
                    self.oam_scan_index += 1;
                }
            }
            85 => {
                // State 10: Transition to Mode 3 (3 cycles)
                // SameBoy: STAT = Mode 3, OAM/VRAM blocked, CGB palettes unblocked
                self.set_mode_stat(Mode::Drawing);
                self.mode_for_interrupt = Some(Mode::Drawing);

                self.oam_read_blocked = true;
                self.oam_write_blocked = true;
                self.vram_read_blocked = true;
                self.vram_write_blocked = true;
                self.cgb_palettes_blocked = false;

                self.update_stat(ints);
            }
            86 | 87 => {
                // Wait in State 10
            }
            88 => {
                // State 32: Transition to Mode 3 (2 cycles)
                // SameBoy: CGB palettes blocked
                self.cgb_palettes_blocked = true;
            }
            _ => {}
        }

        if self.remaining_dots_in_mode <= 0 {
            self.enter_mode(Mode::Drawing, ints);
            // SameBoy: All memory access is blocked during Mode 3
            self.oam_read_blocked = true;
            self.oam_write_blocked = true;
            self.vram_read_blocked = true;
            self.vram_write_blocked = true;
            self.cgb_palettes_blocked = true;
            self.fetcher_state = FetcherState::GetTileT1;
            self.fetcher_tile_x = 0;
            // SameBoy: position_in_line starts at -16. The SCX alignment algorithm
            // in output_pixel will handle jumping to -8 when (position_in_line & 7) == (SCX & 7).
            self.position_in_line = -16;
            self.lcd_x = 0;
            self.bg_fifo.clear();
            self.oam_fifo.clear();
            // Push 8 "junk" pixels to prime the FIFO (will be discarded during scroll)
            self.bg_fifo.push_bg_row(0, 0, 0, false, false);
            // Reset per-line flags
            self.line_has_fractional_scrolling = false;
            self.window_is_being_fetched = false;
            self.window_activation_delay = 0;

            // SameBoy: Immediately do first Mode 3 iteration after entering mode_3_start
            // This is important for cycle-accurate timing.
            self.output_pixel(cgb_mode);
            self.advance_fetcher(cgb_mode);
            self.dots_in_line += 1;
        }
    }
    /// Scan a single OAM entry during Mode 2.
    fn scan_oam_entry(&mut self) {
        let idx = self.oam_scan_index as usize * 4;
        let oam_bytes = self.oam.bytes();

        let y = oam_bytes[idx];
        let x = oam_bytes[idx + 1];
        let tile = oam_bytes[idx + 2];
        let flags = oam_bytes[idx + 3];

        // Sprite height (8 or 16 pixels)
        let height = if self.lcdc & LCDC_OBJ_SIZE != 0 {
            16
        } else {
            8
        };

        // Check if sprite is on this scanline
        // Sprite Y is offset by 16, so visible range is Y-16 to Y-16+height-1
        let sprite_top = y.wrapping_sub(16);
        let sprite_bottom = sprite_top.wrapping_add(height);

        if self.ly >= sprite_top && self.ly < sprite_bottom {
            self.sprite_buffer.add(sprite::SpriteEntry {
                y,
                x,
                tile,
                flags,
                oam_index: self.oam_scan_index,
            });
        }
    }

    /// Tick during Mode 3 (Drawing).
    /// SameBoy: cycles_for_line++ happens AFTER render/fetch, only if position != 160
    fn tick_drawing(&mut self, _ints: &mut Interrupts, cgb_mode: CgbMode) {
        // Check for window activation
        self.check_window_trigger(cgb_mode);

        // Handle window activation delay (1 cycle stall after window triggers)
        if self.window_activation_delay > 0 {
            self.window_activation_delay -= 1;
            // SameBoy: cycles_for_line++ still happens during window stall
            self.dots_in_line += 1;
            return;
        }

        // Check for sprites at current position
        // SameBoy's x_for_object_match: position_in_line + 8, clamped to 0 if overflow
        let match_x = {
            let raw = self.position_in_line.wrapping_add(8) as u8;
            // If raw > 240 (i.e., position_in_line was < -8), use 0
            if raw > 240 { 0 } else { raw }
        };

        // Handle sprite fetch state machine
        if self.sprite_fetcher_state != SpriteFetcherState::Idle {
            self.tick_sprite_fetcher(cgb_mode);
            // SameBoy: cycles_for_line++ happens during sprite fetch too
            self.dots_in_line += 1;
            return;
        }

        // Check if we should start fetching sprites
        let sprites_enabled = self.lcdc & LCDC_OBJ_EN != 0 || matches!(cgb_mode, CgbMode::Cgb);
        if sprites_enabled && i16::from(match_x) != self.last_fetched_x {
            // Find next sprite to fetch at this X position
            if let Some(sprite_idx) = self.find_next_sprite_at_x(match_x) {
                self.start_sprite_fetch(sprite_idx, cgb_mode);
                self.last_fetched_x = i16::from(match_x);
                // Continue with sprite fetcher on next tick
                self.tick_sprite_fetcher(cgb_mode);
                // SameBoy: cycles_for_line++
                self.dots_in_line += 1;
                return;
            }
            self.last_fetched_x = i16::from(match_x);
        }

        // SameBoy order: render_pixel_if_possible THEN advance_fetcher_state_machine
        self.output_pixel(cgb_mode);
        self.advance_fetcher(cgb_mode);

        // SameBoy: Check if position_in_line == 160 BEFORE incrementing cycles_for_line
        // If position reached 160, break out WITHOUT incrementing (this cycle becomes state 22)
        if self.position_in_line >= i16::from(WIDTH) {
            // Increment window line counter if window was active this scanline
            if self.window_triggered {
                self.window_line = self.window_line.wrapping_add(1);
            }

            // SameBoy: For DMG, STAT mode changes to 0 immediately after loop breaks
            // (lines 2090-2097 in display.c)
            self.oam_read_blocked = false;
            self.oam_write_blocked = false;
            self.vram_read_blocked = false;
            self.vram_write_blocked = false;
            self.set_mode_stat(Mode::HBlank);

            // SameBoy: cycles_for_line++ for state 22 (line 2099)
            self.dots_in_line += 1;

            // Calculate HBlank duration: LINE_LENGTH - cycles_for_line
            let hblank_dots = LINE_LENGTH - i32::from(self.dots_in_line);

            // Transition to HBlank mode
            self.mode = Mode::HBlank;
            self.cgb_palettes_blocked = false;
            self.remaining_dots_in_mode = hblank_dots;

            // SameBoy: GB_STAT_update fires AFTER state 22 sleep (line 2108)
            // Set flag so next tick (first of HBlank) fires interrupt
            self.hblank_interrupt_pending = true;
            return;
        }

        // SameBoy: cycles_for_line++ only if we didn't break out (line 2036)
        self.dots_in_line += 1;
    }

    /// Find the next sprite to fetch at the given X position.
    /// Returns the index into sprite_buffer, or None if no more sprites at this X.
    fn find_next_sprite_at_x(&self, x: u8) -> Option<usize> {
        for i in 0..self.sprite_buffer.count as usize {
            let sprite = &self.sprite_buffer.sprites[i];
            if sprite.x == x {
                return Some(i);
            }
        }
        None
    }

    /// Start fetching a sprite at the given index.
    fn start_sprite_fetch(&mut self, sprite_idx: usize, cgb_mode: CgbMode) {
        let sprite = &self.sprite_buffer.sprites[sprite_idx];

        // Calculate tile address
        let height_16 = self.lcdc & LCDC_OBJ_SIZE != 0;
        let tile_y =
            (self.ly.wrapping_sub(sprite.y.wrapping_sub(16))) & if height_16 { 0xF } else { 7 };

        // Apply Y-flip
        let tile_y = if sprite.y_flip() {
            tile_y ^ if height_16 { 0xF } else { 7 }
        } else {
            tile_y
        };

        // Calculate tile number (mask for 8x16 mode)
        let tile_num = if height_16 {
            sprite.tile & 0xFE
        } else {
            sprite.tile
        };

        // Calculate VRAM address
        let line_address = u16::from(tile_num) * 16 + u16::from(tile_y) * 2;

        // Store sprite fetch info
        self.current_sprite_index = sprite_idx;
        self.sprite_tile_address = 0x8000 + line_address;
        self.sprite_vram_bank = if matches!(cgb_mode, CgbMode::Cgb) {
            sprite.cgb_vram_bank()
        } else {
            0
        };
        self.sprite_palette = match cgb_mode {
            CgbMode::Cgb => sprite.cgb_palette(),
            _ => sprite.dmg_palette(),
        };
        self.sprite_priority = match cgb_mode {
            CgbMode::Cgb => sprite.oam_index,
            _ => 0,
        };
        self.sprite_bg_priority = sprite.bg_priority();
        self.sprite_x_flip = sprite.x_flip();

        // Start the sprite fetch state machine
        self.sprite_fetcher_state = SpriteFetcherState::WaitForBgFetcher;
        self.sprite_fetcher_step = 0;
    }

    /// Tick the sprite fetcher state machine.
    /// Implements SameBoy's sprite fetch sequence:
    /// - State 27: Wait loop until (fetcher_state >= 5 AND fifo > 0)
    /// - State 41: Extra advance (1 cycle)
    /// - "Free" advance (no cycle cost, before State 20)
    /// - State 20: OAM read (2 cycles)
    /// - State 39: VRAM low (2 cycles)
    /// - State 40: VRAM high (1 cycle)
    fn tick_sprite_fetcher(&mut self, cgb_mode: CgbMode) {
        match self.sprite_fetcher_state {
            SpriteFetcherState::Idle => {}

            SpriteFetcherState::WaitForBgFetcher => {
                // SameBoy State 27: Wait loop
                // Condition: while (fetcher_state < 5 || fifo_size == 0)
                // fetcher_state >= 5 means GetDataHighT2, PushT1, or PushT2
                let fetcher_aligned = match self.fetcher_state {
                    FetcherState::GetDataHighT2 | FetcherState::PushT1 | FetcherState::PushT2 => {
                        true
                    }
                    _ => false,
                };
                let fifo_not_empty = self.bg_fifo.size() > 0;

                if fetcher_aligned && fifo_not_empty {
                    // Exit wait loop, enter State 41 (1 cycle for advance)
                    self.advance_fetcher(cgb_mode); // State 41's advance
                    self.sprite_fetcher_state = SpriteFetcherState::State41Advance;
                } else {
                    // Matches SameBoy State 27 (Loop body advance)
                    self.advance_fetcher(cgb_mode);
                    // Stay in WaitForBgFetcher
                }
            }

            SpriteFetcherState::State41Advance => {
                // SameBoy State 41: 1 cycle, then do "free" advance and transition to OAM read
                self.advance_fetcher(cgb_mode); // "Free" advance (happens this cycle before State 20)
                self.sprite_fetcher_state = SpriteFetcherState::GetTileAndFlags;
                self.sprite_fetcher_step = 0;
            }

            SpriteFetcherState::GetTileAndFlags => {
                // SameBoy State 20: OAM read (2 cycles)
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 2 {
                    self.sprite_fetcher_state = SpriteFetcherState::GetDataLow;
                    self.sprite_fetcher_step = 0;
                }
            }

            SpriteFetcherState::GetDataLow => {
                // SameBoy State 39: VRAM low read (2 cycles)
                if self.sprite_fetcher_step == 0 {
                    self.sprite_tile_data_low = self
                        .vram
                        .vram_at_bank(self.sprite_tile_address, self.sprite_vram_bank);
                }
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 2 {
                    self.sprite_fetcher_state = SpriteFetcherState::GetDataHighAndPush;
                    self.sprite_fetcher_step = 0;
                }
            }

            SpriteFetcherState::GetDataHighAndPush => {
                // SameBoy State 40: VRAM high read (1 cycle), then overlay
                self.sprite_tile_data_high = self
                    .vram
                    .vram_at_bank(self.sprite_tile_address + 1, self.sprite_vram_bank);

                // Overlay sprite onto OAM FIFO
                self.oam_fifo.overlay_object_row(
                    self.sprite_tile_data_low,
                    self.sprite_tile_data_high,
                    self.sprite_palette,
                    self.sprite_bg_priority,
                    self.sprite_priority,
                    self.sprite_x_flip,
                );

                // Check for more sprites at the same X position
                let current_x = self.sprite_buffer.sprites[self.current_sprite_index].x;
                let next_sprite = self.find_next_sprite_after(self.current_sprite_index, current_x);

                if let Some(next_idx) = next_sprite {
                    // Restart fetch for next sprite
                    self.start_sprite_fetch(next_idx, cgb_mode);
                } else {
                    // Done fetching sprites
                    self.sprite_fetcher_state = SpriteFetcherState::Idle;
                }
            }
        }
    }

    /// Find the next sprite after the given index at the same X position.
    fn find_next_sprite_after(&self, after_idx: usize, x: u8) -> Option<usize> {
        for i in (after_idx + 1)..self.sprite_buffer.count as usize {
            let sprite = &self.sprite_buffer.sprites[i];
            if sprite.x == x {
                return Some(i);
            }
        }
        None
    }

    /// Check if window should be activated at the current position.
    fn check_window_trigger(&mut self, cgb_mode: CgbMode) {
        // Window already triggered for this scanline
        if self.window_triggered {
            return;
        }

        // Check if window is enabled
        let window_enabled = match cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => {
                self.lcdc & (LCDC_BG_EN | LCDC_WIN_ENABLE) == (LCDC_BG_EN | LCDC_WIN_ENABLE)
            }
            CgbMode::Cgb => self.lcdc & LCDC_WIN_ENABLE != 0,
        };

        if !window_enabled {
            return;
        }

        // Check WY condition (window Y trigger) - must have been triggered on or before current line
        if self.ly < self.wy {
            return;
        }

        let is_cgb = matches!(cgb_mode, CgbMode::Cgb);
        let pos = self.position_in_line;

        // SameBoy: WX=0 has special handling
        if self.wx == 0 {
            let should_activate = if pos == -7i16 as i16 {
                true
            } else if pos == -16i16 as i16 && (self.scx & 7) != 0 {
                true
            } else {
                // position_in_line >= -15 && position_in_line <= -8
                pos >= -15 && pos <= -8
            };

            if should_activate {
                self.activate_window(is_cgb);
            }
        }
        // SameBoy: WX < 166 (or 167 on CGB) - normal window trigger
        else if self.wx < 166 + u8::from(is_cgb) {
            // Window activates when position_in_line + 7 == WX
            if pos + 7 == i16::from(self.wx) {
                self.activate_window(is_cgb);
            }
        }
        // SameBoy: WX=166 on DMG - special case, increment window_y but don't fully trigger
        else if !is_cgb && self.wx == 166 && pos + 7 == i16::from(self.wx) {
            // Just increment window line counter without full window activation
            self.window_line = self.window_line.wrapping_add(1);
        }
    }

    /// Activate window rendering.
    fn activate_window(&mut self, is_cgb: bool) {
        self.window_triggered = true;
        self.win_in_frame = true;
        self.window_is_being_fetched = true;

        // Clear BG FIFO and restart fetcher for window
        self.bg_fifo.clear();
        self.fetcher_state = FetcherState::GetTileT1;
        self.fetcher_tile_x = 0;

        // Window activation incurs a delay before fetcher starts
        // SameBoy line 1917-1919: WX=0 with (SCX & 7) != 0 on DMG adds extra 1 cycle
        if self.wx == 0 && (self.scx & 7) != 0 && !is_cgb {
            self.window_activation_delay = 2; // 1 base + 1 extra for this case
        } else if (self.scx & 7) != 0 && !is_cgb {
            // Non-zero SCX also seems to add 1 extra cycle for window activation
            self.window_activation_delay = 2;
        } else {
            self.window_activation_delay = 1;
        }
    }

    /// Advance the background/window fetcher state machine.
    /// Uses T1/T2 states matching SameBoy:
    /// - T1: Calculate addresses/setup
    /// - T2: Perform VRAM read
    fn advance_fetcher(&mut self, cgb_mode: CgbMode) {
        match self.fetcher_state {
            FetcherState::GetTileT1 => {
                // T1: Calculate tile map address
                let tile_map = if self.window_triggered {
                    self.win_tile_map_addr()
                } else {
                    self.bg_tile_map_addr()
                };

                let y = if self.window_triggered {
                    self.window_line
                } else {
                    self.ly.wrapping_add(self.scy)
                };

                let x = if self.window_triggered {
                    self.fetcher_tile_x
                } else {
                    self.fetcher_tile_x.wrapping_add(self.scx / 8) & 0x1F
                };

                // Cache address for T2
                self.fetcher_tile_index_addr = tile_map + u16::from(y / 8) * 32 + u16::from(x);
                self.fetcher_state = FetcherState::GetTileT2;
            }
            FetcherState::GetTileT2 => {
                // T2: Read tile index and attributes from VRAM
                self.current_tile = self.vram.vram_at_bank(self.fetcher_tile_index_addr, 0);
                self.current_tile_attrs = match cgb_mode {
                    CgbMode::Cgb => self.vram.vram_at_bank(self.fetcher_tile_index_addr, 1),
                    _ => 0,
                };

                self.fetcher_state = FetcherState::GetDataLowT1;
            }
            FetcherState::GetDataLowT1 => {
                // T1: Calculate tile data address
                self.fetcher_tile_data_addr = self.calculate_tile_data_addr();
                self.fetcher_state = FetcherState::GetDataLowT2;
            }
            FetcherState::GetDataLowT2 => {
                // T2: Read low byte from VRAM
                self.current_tile_data[0] = self.read_tile_byte(self.fetcher_tile_data_addr);
                self.fetcher_state = FetcherState::GetDataHighT1;
            }
            FetcherState::GetDataHighT1 => {
                // T1: Address already calculated, just advance
                self.fetcher_state = FetcherState::GetDataHighT2;
            }
            FetcherState::GetDataHighT2 => {
                // T2: Read high byte from VRAM
                self.current_tile_data[1] = self.read_tile_byte(self.fetcher_tile_data_addr + 1);
                self.fetcher_state = FetcherState::PushT1;
            }
            FetcherState::PushT1 => {
                // T1: Wait cycle (FIFO push happens in T2)
                self.fetcher_state = FetcherState::PushT2;
            }
            FetcherState::PushT2 => {
                // SameBoy: Only push when FIFO is empty (fifo_size == 0)
                // Line 1084: if (fifo_size(&gb->bg_fifo) > 0) break;
                if self.bg_fifo.is_empty() {
                    let palette = self.current_tile_attrs & 0x07;
                    let bg_priority = self.current_tile_attrs & 0x80 != 0;
                    let flip_x = self.current_tile_attrs & 0x20 != 0;

                    self.bg_fifo.push_bg_row(
                        self.current_tile_data[0],
                        self.current_tile_data[1],
                        palette,
                        bg_priority,
                        flip_x,
                    );

                    if self.window_triggered {
                        self.fetcher_tile_x = self.fetcher_tile_x.wrapping_add(1) & 0x1F;
                    }
                    self.fetcher_state = FetcherState::GetTileT1;
                }
                // SameBoy: stays in PUSH state if FIFO not empty
            }
        }
    }

    /// Calculate tile data address for current tile.
    fn calculate_tile_data_addr(&self) -> u16 {
        let y = if self.window_triggered {
            self.window_line
        } else {
            self.ly.wrapping_add(self.scy)
        };

        let y_offset = if self.current_tile_attrs & 0x40 != 0 {
            // Y flip
            7 - (y & 7)
        } else {
            y & 7
        };

        let tile_num = self.current_tile;
        let base = if self.lcdc & LCDC_TILE_SEL == 0 {
            // 0x8800-0x97FF, signed addressing
            #[expect(clippy::cast_possible_wrap)]
            let signed_tile = tile_num as i8;
            #[expect(clippy::cast_sign_loss)]
            let offset = (i16::from(signed_tile) + 128) as u16;
            0x8800 + offset * 16
        } else {
            // 0x8000-0x8FFF, unsigned addressing
            0x8000 + u16::from(tile_num) * 16
        };

        base + u16::from(y_offset) * 2
    }

    /// Read a tile data byte, respecting CGB VRAM bank.
    const fn read_tile_byte(&self, addr: u16) -> u8 {
        let bank = (self.current_tile_attrs >> 3) & 1;
        self.vram.vram_at_bank(addr, bank)
    }

    /// Output a pixel to the LCD buffer.
    /// Implements SameBoy's render_pixel_if_possible logic.
    fn output_pixel(&mut self, cgb_mode: CgbMode) {
        // SameBoy line 667: FIFO empty check FIRST, before anything else
        if self.bg_fifo.is_empty() {
            return;
        }

        // SameBoy line 674: Pop from BG FIFO
        let bg_pixel = self.bg_fifo.pop();
        // SameBoy line 678-684: Only pop from OAM FIFO if it has items
        let sprite_pixel = if !self.oam_fifo.is_empty() {
            self.oam_fifo.pop()
        } else {
            fifo::FifoItem::default()
        };

        // SameBoy lines 686-704: Handle position_in_line alignment for SCX.
        // (position_in_line + 16 < 8) is equivalent to (position_in_line < -8) in unsigned logic.
        // When position_in_line is in the range [-16, -9], we're in the "fractional scrolling" phase.
        #[expect(clippy::cast_sign_loss)]
        let unsigned_pos_plus_16 = (self.position_in_line + 16) as u8;
        if unsigned_pos_plus_16 < 8 {
            // SameBoy edge case: position_in_line == -17 wraps to -16
            if self.position_in_line == -17 {
                self.position_in_line = -16;
            } else if (self.position_in_line & 7) as u8 == (self.scx & 7) {
                // When (position_in_line & 7) == (SCX & 7), jump to -8
                self.position_in_line = -8;
            } else if self.window_is_being_fetched
                && (self.position_in_line & 7) as u8 == 6
                && (self.scx & 7) == 7
            {
                // SameBoy edge case: window fetch with specific SCX alignment
                self.position_in_line = -8;
            } else if self.position_in_line == -9 {
                // SameBoy edge case: -9 wraps back to -16
                self.position_in_line = -16;
                return;
            } else {
                self.line_has_fractional_scrolling = true;
            }
        }

        // SameBoy line 706
        self.window_is_being_fetched = false;

        // SameBoy lines 709-711: Drop pixels when outside visible range.
        // In SameBoy, position_in_line is uint8_t, so `>= 160` covers:
        // - 160+ (end of line, after all 160 visible pixels)
        // - 240-255 (negative values -16 to -1 in signed interpretation)
        // In Ceres with i16, we need to check both ranges explicitly.
        #[expect(clippy::cast_sign_loss)]
        let position_u8 = self.position_in_line as u8;
        if position_u8 >= 160 {
            // Discard phase - just increment position, pixel already popped
            self.position_in_line += 1;
            return;
        }

        // Normal rendering
        let (color, palette, is_sprite) = self.mix_pixels(bg_pixel, sprite_pixel, cgb_mode);

        let rgb = if is_sprite {
            self.sprite_color_to_rgb(color, palette, cgb_mode)
        } else {
            self.bg_color_to_rgb(color, palette, cgb_mode)
        };

        let idx = u32::from(self.ly) * u32::from(WIDTH) + u32::from(self.lcd_x);
        self.rgb_buf.set_px(idx, rgb);

        self.lcd_x += 1;
        self.position_in_line += 1;
    }

    /// Mix background and sprite pixels according to priority rules.
    fn mix_pixels(
        &self,
        bg_pixel: fifo::FifoItem,
        sprite_pixel: fifo::FifoItem,
        cgb_mode: CgbMode,
    ) -> (u8, u8, bool) {
        // Check if BG is enabled (LCDC bit 0)
        // On DMG: When disabled, BG renders as color 0 (white)
        // On CGB: This bit acts as master priority, not enable/disable
        let bg_enabled = self.lcdc & LCDC_BG_EN != 0 || matches!(cgb_mode, CgbMode::Cgb);
        let effective_bg_color = if bg_enabled { bg_pixel.color } else { 0 };

        // Check if sprites are enabled (LCDC bit 1)
        // On DMG: Sprites are disabled if bit is 0
        // On CGB: Sprites are ALWAYS processed regardless of this bit (checked at pop time)
        let sprites_enabled = self.lcdc & LCDC_OBJ_EN != 0;

        // If sprites are disabled, use background
        // On CGB, this check still applies - sprites won't render if OBJ bit is 0
        if !sprites_enabled {
            return (effective_bg_color, bg_pixel.palette, false);
        }

        // Transparent sprite pixel - use background
        if sprite_pixel.color == 0 {
            return (effective_bg_color, bg_pixel.palette, false);
        }

        // Check sprite priority
        let sprite_behind_bg = sprite_pixel.bg_priority;
        let bg_priority = bg_pixel.bg_priority;
        let bg_opaque = effective_bg_color != 0;

        // Determine if sprite should be hidden behind BG
        let hide_sprite = match cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => {
                // DMG: BG/OBJ priority from sprite attribute
                sprite_behind_bg && bg_opaque
            }
            CgbMode::Cgb => {
                // CGB: LCDC bit 0 acts as master priority
                if self.lcdc & LCDC_BG_EN == 0 {
                    // Master priority off - sprites always on top
                    false
                } else {
                    // Master priority on - check both BG priority and sprite priority
                    (bg_priority || sprite_behind_bg) && bg_opaque
                }
            }
        };

        if hide_sprite {
            (effective_bg_color, bg_pixel.palette, false)
        } else {
            (sprite_pixel.color, sprite_pixel.palette, true)
        }
    }

    /// Convert background color to RGB.
    fn bg_color_to_rgb(&self, color: u8, palette: u8, cgb_mode: CgbMode) -> (u8, u8, u8) {
        match cgb_mode {
            CgbMode::Dmg => {
                let shade = (self.bgp >> (color * 2)) & 3;
                Self::mono_rgb(shade)
            }
            CgbMode::Compat => {
                let shade = (self.bgp >> (color * 2)) & 3;
                self.bcp.rgb(palette, shade, self.color_correction_mode)
            }
            CgbMode::Cgb => self.bcp.rgb(palette, color, self.color_correction_mode),
        }
    }

    /// Convert sprite color to RGB.
    fn sprite_color_to_rgb(&self, color: u8, palette: u8, cgb_mode: CgbMode) -> (u8, u8, u8) {
        match cgb_mode {
            CgbMode::Dmg => {
                let pal = if palette == 0 { self.obp0 } else { self.obp1 };
                let shade = (pal >> (color * 2)) & 3;
                Self::mono_rgb(shade)
            }
            CgbMode::Compat => {
                let pal = if palette == 0 { self.obp0 } else { self.obp1 };
                let shade = (pal >> (color * 2)) & 3;
                self.ocp.rgb(0, shade, self.color_correction_mode)
            }
            CgbMode::Cgb => self.ocp.rgb(palette, color, self.color_correction_mode),
        }
    }

    /// Get background tile map address.
    #[inline]
    const fn bg_tile_map_addr(&self) -> u16 {
        if self.lcdc & LCDC_BG_MAP != 0 {
            0x9C00
        } else {
            0x9800
        }
    }

    /// Get window tile map address.
    #[inline]
    const fn win_tile_map_addr(&self) -> u16 {
        if self.lcdc & LCDC_WIN_MAP != 0 {
            0x9C00
        } else {
            0x9800
        }
    }

    /// Monochrome palette lookup.
    #[must_use]
    const fn mono_rgb(shade: u8) -> (u8, u8, u8) {
        color_palette::GRAYSCALE_PALETTE[shade as usize]
    }

    /// Tick during Mode 0 (HBlank).
    fn tick_hblank(&mut self, ints: &mut Interrupts, _double_speed: bool) {
        // SameBoy: GB_STAT_update fires AFTER state 22 (1 cycle after STAT changes to 0)
        // Fire the pending HBlank interrupt on first tick of HBlank
        if self.hblank_interrupt_pending {
            self.hblank_interrupt_pending = false;
            self.update_stat(ints);
        }

        // SameBoy quirk: Mode 2 interrupt fires near the end of HBlank,
        // about 1 cycle before the line actually changes.
        // Fire the interrupt early while STAT still shows Mode 0.
        // SameBoy: LINES (144) - 1 = 143
        if self.remaining_dots_in_mode == 1 && self.ly < LINES - 1 {
            let next_line = self.ly + 1;
            if next_line != 0 {
                self.mode_for_interrupt = Some(Mode::OamScan);
                self.update_stat(ints);
                // Clear mode_for_interrupt so it doesn't interfere with stat_irq_blocking
                self.mode_for_interrupt = None;
            }
        }

        if self.remaining_dots_in_mode <= 0 {
            self.dots_in_line = 0;

            // Clear first frame flag when leaving line 0
            if self.ly == 0 && self.first_frame_after_lcd_on {
                self.first_frame_after_lcd_on = false;
            }

            // SameBoy: LINES (144) - 1 = 143
            if self.ly + 1 > LINES - 1 {
                self.ly += 1;
                self.ly_for_comparison = self.ly;
                self.enter_mode(Mode::VBlank, ints);
            } else {
                // Reset for next scanline
                self.oam_scan_index = 0;
                self.sprite_buffer.clear();
                // SameBoy: OAM read blocked during Mode 2, but writes allowed initially
                self.oam_read_blocked = true;
                self.oam_write_blocked = false;
                self.vram_read_blocked = false;
                self.vram_write_blocked = false;
                self.cgb_palettes_blocked = false;
                self.window_triggered = false;

                self.enter_mode(Mode::OamScan, ints);
            }
        }
    }

    /// Tick during Mode 1 (VBlank).
    /// SameBoy: Lines 144-153 (LINES to VIRTUAL_LINES - 1)
    fn tick_vblank(&mut self, ints: &mut Interrupts) {
        if self.remaining_dots_in_mode <= 0 {
            let new_ly = self.ly + 1;

            // SameBoy: VIRTUAL_LINES (154) - 1 = 153
            if new_ly > VIRTUAL_LINES - 1 {
                self.ly = new_ly;
                self.ly_for_comparison = new_ly;
                self.finish_frame_and_start_new(ints);
            } else {
                // SameBoy VBlank timing (lines 2152-2215):
                // At start of each VBlank line: ly_for_comparison = -1, sleep 2
                // Then: LY = current_line
                // Test line_153_ly_a expects LY=152 at certain cycle,
                // increasing delay to 4 to shift LY update later
                let delay = 4;
                self.schedule_ly_write(new_ly, delay);
                self.ly_for_comparison = new_ly;
                self.remaining_dots_in_mode += Mode::VBlank.dots(self.scx);
                self.update_stat(ints);
            }
        }
    }

    /// Complete the current frame and start a new one.
    fn finish_frame_and_start_new(&mut self, ints: &mut Interrupts) {
        self.ly = 0xFF;
        self.ly_for_comparison = 0xFF;
        self.ly_write_delay = 0; // Clear any pending LY write
        self.dots_in_line = 0;

        // Present frame
        if self.delay_one_frame {
            self.delay_one_frame = false;
        } else {
            self.rgba_buf_present = mem::take(&mut self.rgb_buf);
        }

        // Reset for new frame
        self.oam_scan_index = 0;
        self.sprite_buffer.clear();
        self.oam_read_blocked = true;
        self.oam_write_blocked = false;
        self.vram_read_blocked = false;
        self.vram_write_blocked = false;
        self.cgb_palettes_blocked = false;
        self.window_line = 0;
        self.window_triggered = false;
        self.enter_mode(Mode::OamScan, ints);
    }

    pub const fn write_bgp(&mut self, val: u8) {
        self.bgp = val;
    }

    pub fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts) {
        // turn off
        if val & LCDC_ENABLE == 0 && self.lcdc & LCDC_ENABLE != 0 {
            // FIXME: breaks 'alone in the dark' and the menu fade out in 'Links awakening' among others
            // debug_assert!(
            //     matches!(self.mode(), Mode::VBlank),
            //     "current mode = {:?}, dots = {}, ly = {}",
            //     self.mode(),
            //     self.remaining_dots_in_mode,
            //     self.ly
            // );

            self.ly = 0;
            self.ly_for_comparison = 0;
            self.ly_write_delay = 0; // Clear any pending LY write
            let mode = Mode::HBlank;
            self.mode = mode;
            self.set_mode_stat(mode);
            self.remaining_dots_in_mode = mode.dots(self.scx);
            self.rgba_buf_present.clear();

            // Reset startup state
            self.startup_state = StartupState::Inactive;
            self.first_frame_after_lcd_on = false;

            // Unblock all memory access when LCD is off
            self.oam_read_blocked = false;
            self.oam_write_blocked = false;
            self.vram_read_blocked = false;
            self.vram_write_blocked = false;
            self.cgb_palettes_blocked = false;

            // When LCD turns off:
            // - LY=LYC coincidence flag is RETAINED (not cleared)
            // - The comparison clock stops, so LYC changes won't update the flag
            // - The STAT interrupt line state should reflect the frozen coincidence
            //   to prevent spurious interrupts when LCD turns back on
            // If LY=LYC coincidence is set and LYC interrupt is enabled,
            // the interrupt line should stay high
            self.stat_interrupt_line =
                (self.stat & STAT_LYC != 0) && (self.stat & STAT_IF_LYC != 0);
        }

        // turn on
        if val & LCDC_ENABLE != 0 && self.lcdc & LCDC_ENABLE == 0 {
            self.ly = 0;
            self.ly_for_comparison = 0;
            self.ly_write_delay = 0; // Clear any pending LY write
            let mode = Mode::HBlank;
            self.mode = mode;
            self.set_mode_stat(mode);
            self.remaining_dots_in_mode = mode.dots(self.scx);
            // Comparison clock restarts - update coincidence and check for interrupt
            self.update_stat(ints);

            // Start LCD startup state machine with Phase 1 (76 cycles)
            // Total: 76 + 2 + 1 + 1 = 80 cycles
            self.startup_state = StartupState::Phase1(76);
            self.oam_read_blocked = false;
            self.oam_write_blocked = false;
            self.vram_read_blocked = false;
            self.vram_write_blocked = false;
            self.cgb_palettes_blocked = false;

            // First line after LCD on has no OAM scan
            self.sprite_buffer.clear();

            // Mark as first frame for special timing
            self.first_frame_after_lcd_on = true;
            self.dots_in_line = 0;

            self.delay_one_frame = true;
        }

        self.lcdc = val;
    }

    pub fn write_lyc(&mut self, val: u8, ints: &mut Interrupts) {
        self.lyc = val;
        // LYC change may affect coincidence - update STAT line if LCD is on
        if self.lcdc & LCDC_ENABLE != 0 {
            self.update_stat(ints);
        }
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

    pub const fn write_scy(&mut self, val: u8) {
        self.scy = val;
    }

    pub fn write_stat(&mut self, val: u8, ints: &mut Interrupts) {
        let ly_equals_lyc = self.stat & STAT_LYC;
        let mode: u8 = self.mode() as u8;

        self.stat = val;
        self.stat &= !(STAT_LYC | STAT_MODE);
        self.stat |= ly_equals_lyc | mode;

        // STAT write may change which interrupts are enabled - update line if LCD is on
        if self.lcdc & LCDC_ENABLE != 0 {
            self.update_stat(ints);
        }
    }

    pub const fn write_wx(&mut self, val: u8) {
        self.wx = val;
    }

    pub const fn write_wy(&mut self, val: u8) {
        self.wy = val;
    }
}
