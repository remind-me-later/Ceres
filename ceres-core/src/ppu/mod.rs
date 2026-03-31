mod color_palette;
mod fetcher;
mod fifo;
mod oam;
mod rgba_buf;
mod sprite;
mod state;
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
    state::{HBlankStage, Line0Stage, Line153Stage, OamScanStage, PpuPhase, VBlankStage},
};

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;
// Aliases for lib.rs compatibility
pub const WIDTH: u8 = PX_WIDTH;
pub const LINES: u8 = PX_HEIGHT;

// LCDC bits
const LCDC_BG_B: u8 = 0x1;
const LCDC_OBJ_B: u8 = 0x2;
const LCDC_OBJL_B: u8 = 0x4;
const LCDC_BG_AREA: u8 = 0x8;
const LCDC_BG_SIGNED: u8 = 0x10;
const LCDC_WIN_B: u8 = 0x20;
const LCDC_WIN_AREA: u8 = 0x40;
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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum FrameSkipState {
    #[default]
    Normal,
    LcdTurnedOn,
    FirstFrameRendered,
}

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Order follows the state machine transitions"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

#[expect(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct Ppu {
    bcp: ColorPalette,
    bgp: u8,
    color_correction_mode: ColorCorrectionMode,
    frame_skip_state: FrameSkipState,
    /// Current PPU phase (state machine).
    phase: PpuPhase,
    lcdc: u8,
    ly: u8,
    /// LY value used for LYC comparison (may differ from displayed LY during transitions)
    /// u16 to distinguish between valid u8 lines and "none" (0xFFFF)
    ly_for_comparison: u16,
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
    stat: u8,
    /// Internal STAT interrupt line - OR of all enabled STAT sources.
    /// Used to implement edge-triggered interrupt behavior.
    stat_interrupt_line: bool,
    /// Mode used for interrupt purposes (can differ from STAT mode bits by 1-2 cycles).
    /// Required to fire Mode 2 interrupt slightly before STAT shows Mode 2.
    /// None means no mode-based interrupt should fire.
    mode_for_interrupt: Option<Mode>,
    vram: Vram,
    wy_triggered: bool,
    wx: u8,
    wy: u8,
    is_frozen: bool,

    /// Background/window pixel FIFO (8-pixel capacity).
    bg_fifo: PixelFifo,
    /// Sprite (OAM) pixel FIFO (8-pixel capacity).
    oam_fifo: PixelFifo,
    /// Background fetcher state machine state.
    fetcher_state: FetcherState,
    /// Background fetcher sub-cycle counter (for 8MHz timing).
    fetcher_step: u8,
    /// Current window tile X coordinate being fetched.
    window_tile_x: u8,
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
    wx_triggered: bool,
    /// Window internal line counter.
    window_y: u8,
    /// OAM read access blocked.
    oam_read_blocked: bool,
    /// OAM write access blocked.
    oam_write_blocked: bool,
    /// VRAM read access blocked.
    vram_read_blocked: bool,
    /// VRAM write access blocked.
    vram_write_blocked: bool,
    /// VRAM PPU blocked (CGB Stop mode conflict).
    vram_ppu_blocked: bool,
    /// CGB palette access blocked.
    cgb_palettes_blocked: bool,
    current_line: u8,
    /// Cached tile index address (calculated in T1, used in T2).
    fetcher_tile_index_addr: u16,
    /// Cached tile data address (calculated in T1, used in T2).
    fetcher_tile_data_addr: u16,
    /// OAM scan index (0-39, which OAM entry is being checked).
    oam_scan_index: u8,
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

    /// The line-0 startup sequence (after LCD-off → LCD-on) consumes 8 fewer T-cycles of HBlank
    /// than a normal line (SameBoy display.c line ~1690: `cycles_for_line += 8`).
    /// This flag causes the HBlank Remainder stage to trigger PreEnd 16 ticks (8 T-cycles) early
    /// so that line 0 ends 16 half-clocks sooner, shifting line 1's OAM-scan blocking events
    /// 16 half-clocks later in absolute CPU time (matching the `lcdoffset1` Gambatte test ROMs).
    first_line_short: bool,

    // Bus contention state
    ext_dma_active: bool,
    ext_dma_src: u16,
    ext_dma_dst: u8,
    ext_hdma_active: bool,
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

        // Internal coincidence match used for interrupt logic
        let coincidence_match = if self.ly_for_comparison != 0xFFFF {
            self.ly_for_comparison == u16::from(self.lyc)
        } else {
            false
        };

        // Compute new STAT interrupt line state from all enabled sources.
        // Note: coincidence interrupt is triggered by internal match.
        let lyc_int = (self.stat & STAT_IF_LYC_B != 0) && coincidence_match;
        let mode_int = match self.mode_for_interrupt {
            Some(Mode::HBlank) => self.stat & STAT_IF_HBLANK_B != 0,
            Some(Mode::VBlank) => self.stat & STAT_IF_VBLANK_B != 0,
            Some(Mode::OamScan) => self.stat & STAT_IF_OAM_B != 0,
            _ => false,
        };

        let new_line = lyc_int || mode_int;

        self.stat_interrupt_line = new_line;

        // Only fire interrupt on rising edge (low -> high transition)
        if new_line && !previous_line {
            ints.request_lcd();
        }

        // FINALLY update the STAT register bits for CPU visibility.
        // This ensures that if an interrupt fires, an IMMEDIATE CPU read of STAT
        // might still see the OLD coincidence flag value (if requested right at the edge).
        if self.ly_for_comparison != 0xFFFF {
            self.stat &= !STAT_LYC_B;
            if coincidence_match {
                self.stat |= STAT_LYC_B;
            }
        }
    }

    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
        match mode {
            Mode::OamScan => {
                self.phase = PpuPhase::OamScan(OamScanStage::default());
                // Don't clear mode_for_interrupt here to allow continuous STAT line
                // across HBlank -> OamScan transition for proper IRQ blocking.
            }
            Mode::VBlank => self.phase = PpuPhase::VBlank(VBlankStage::default()),
            Mode::Drawing => {
                self.phase = PpuPhase::Drawing;
                self.set_mode_stat(mode);
                self.mode_for_interrupt = Some(mode);
                self.update_stat(ints);
            }
            Mode::HBlank => {
                self.phase = PpuPhase::HBlank(HBlankStage::default());
                self.set_mode_stat(mode);
                self.mode_for_interrupt = Some(mode);
                self.update_stat(ints);
            }
        }
    }

    #[cfg(test)]
    #[must_use]
    pub const fn dots_in_line(&self) -> u16 {
        self.dots_in_line
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        match self.stat & STAT_MODE_B {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            3 => Mode::Drawing,
            _ => unreachable!(),
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
        // Compute mode from dots_in_line for accurate timing.
        let computed_mode = self.compute_stat_mode();
        (self.stat & !STAT_MODE_B) | computed_mode | 0x80
    }

    /// Compute STAT mode based on current timing state.
    /// This provides more accurate mode reporting than the maintained state
    /// for the first line after LCD enable (which has special timing).
    const fn compute_stat_mode(&self) -> u8 {
        // LCD off: mode 0
        if self.lcdc & LCDC_ON_B == 0 || self.is_frozen {
            return 0;
        }

        // First 82 cycles show Mode 0 (164 ticks)
        // Only apply this special case for the actual first frame after LCD enable
        if matches!(self.frame_skip_state, FrameSkipState::LcdTurnedOn) && self.ly == 0 {
            // During startup state machine, use the stored STAT mode bits
            if matches!(self.phase, PpuPhase::Line0Startup(_)) {
                return self.stat & STAT_MODE_B;
            }

            // First 82 cycles show Mode 0 (164 ticks)
            if self.dots_in_line < 164 {
                return 0;
            }
        }

        // For all other cases, use the stored STAT mode bits
        self.stat & STAT_MODE_B
    }

    #[must_use]
    pub const fn read_wx(&self) -> u8 {
        self.wx
    }

    #[must_use]
    pub const fn read_wy(&self) -> u8 {
        self.wy
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn sprite_buffer_len(&self) -> usize {
        self.sprite_buffer.count as usize
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn position_in_line(&self) -> i16 {
        self.position_in_line
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn bg_fifo_size(&self) -> usize {
        self.bg_fifo.size() as usize
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn fetcher_state(&self) -> FetcherState {
        self.fetcher_state
    }

    pub fn run(
        &mut self,
        dots: i32,
        ints: &mut Interrupts,
        cgb_mode: CgbMode,
        double_speed: bool,
        dma_active: bool,
        dma_src: u16,
        dma_dst: u8,
        hdma_active: bool,
    ) {
        self.ext_dma_active = dma_active;
        self.ext_dma_src = dma_src;
        self.ext_dma_dst = dma_dst;
        self.ext_hdma_active = hdma_active;

        for _ in 0..dots {
            self.tick(ints, cgb_mode, double_speed);
        }
    }

    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.color_correction_mode = mode;
    }

    fn set_mode_stat(&mut self, mode: Mode) {
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
    }

    /// Returns true if CGB palettes (BCP/OCP) are accessible by the CPU.
    #[inline]
    #[must_use]
    pub const fn is_cgb_palettes_accessible(&self) -> bool {
        // CGB palettes are accessible when LCD is off or not blocked
        self.lcdc & LCDC_ON_B == 0 || !self.cgb_palettes_blocked
    }

    /// Advance PPU by one T-cycle (dot).
    #[inline]
    pub fn tick(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        if self.lcdc & LCDC_ON_B == 0 || self.is_frozen {
            return;
        }

        match self.phase {
            PpuPhase::Line0Startup(stage) => {
                self.tick_line0(stage, ints, cgb_mode);
            }
            PpuPhase::Line153(stage) => {
                self.tick_line153(stage, ints);
            }
            PpuPhase::OamScan(_) => {
                // OAM scan is driven by its own state machine
                self.dots_in_line += 1;
                self.tick_oam_scan(ints, cgb_mode, double_speed);
            }
            PpuPhase::HBlank(_) => {
                self.dots_in_line += 1;
                self.tick_hblank(ints, double_speed);
            }
            PpuPhase::VBlank(_) => {
                self.dots_in_line += 1;
                self.tick_vblank(ints);
            }
            PpuPhase::Drawing => {
                self.dots_in_line += 1;
                self.tick_drawing(ints, cgb_mode);
            }
            PpuPhase::LcdOff => {
                // LCD is off, nothing to do
            }
        }
    }

    /// Tick Line 153 state machine.
    fn tick_line153(&mut self, stage: Line153Stage, ints: &mut Interrupts) {
        // Track dots (Line 153 is part of VBlank mode effectively, but distinct phase)
        self.dots_in_line += 1;

        match stage {
            Line153Stage::LycReset { remaining } => {
                // State 19: 2 cycles (4 ticks)
                if remaining == 4 {
                    self.ly_for_comparison = 0xFFFF;
                    self.stat &= !STAT_LYC_B; // Clear coincidence flag when comparison disabled
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    self.phase = PpuPhase::Line153(Line153Stage::Ly153 { remaining: 4 });
                } else {
                    self.phase = PpuPhase::Line153(Line153Stage::LycReset {
                        remaining: remaining - 1,
                    });
                }
            }

            Line153Stage::Ly153 { remaining } => {
                // State 14: 2 cycles (4 ticks)
                if remaining == 4 {
                    self.ly = 153;
                    self.ly_for_comparison = 153;
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    self.phase = PpuPhase::Line153(Line153Stage::Ly0 { remaining: 4 });
                } else {
                    self.phase = PpuPhase::Line153(Line153Stage::Ly153 {
                        remaining: remaining - 1,
                    });
                }
            }

            Line153Stage::Ly0 { remaining } => {
                // State 15: 2 cycles (4 ticks)
                if remaining == 4 {
                    self.ly = 0;
                    // LYC comparison for LY=0 happens now?
                    // Actually, let's just set it.
                    self.ly_for_comparison = 0;
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    self.phase = PpuPhase::Line153(Line153Stage::LycTransition { remaining: 8 });
                } else {
                    self.phase = PpuPhase::Line153(Line153Stage::Ly0 {
                        remaining: remaining - 1,
                    });
                }
            }

            Line153Stage::LycTransition { remaining } => {
                // State 16: 4 cycles (8 ticks)
                if remaining <= 1 {
                    self.phase = PpuPhase::Line153(Line153Stage::LycSideEffect { remaining: 24 });
                } else {
                    self.phase = PpuPhase::Line153(Line153Stage::LycTransition {
                        remaining: remaining - 1,
                    });
                }
            }

            Line153Stage::LycSideEffect { remaining } => {
                // State 29: 12 cycles (24 ticks)
                if remaining == 24 {
                    self.ly_for_comparison = 0;
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    // Total used: 4+4+4+8+24 = 44 ticks.
                    // Remainder = 912 - 44 = 868.
                    self.phase = PpuPhase::Line153(Line153Stage::Remainder { remaining: 868 });
                } else {
                    self.phase = PpuPhase::Line153(Line153Stage::LycSideEffect {
                        remaining: remaining - 1,
                    });
                }
            }

            Line153Stage::Remainder { remaining } => {
                // State 17: Reports Mode 0 at the very end (DMG quirk)
                if remaining == 4 {
                    self.set_mode_stat(Mode::HBlank);
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    // End of frame
                    self.finish_frame_and_start_new(ints);
                    // finish_frame sets phase to OamScan
                } else {
                    self.phase = PpuPhase::Line153(Line153Stage::Remainder {
                        remaining: remaining - 1,
                    });
                }
            }
        }
    }

    /// Tick the LCD startup state machine (Line 0).
    /// Timing for first line after LCD on (8MHz ticks):
    /// - InitialMode0 (152 ticks): Mode 0 in STAT, all unblocked
    /// - OamWriteBlock (4 ticks): OAM write blocked
    /// - StatMode3 (4 ticks): STAT = Mode 3, OAM fully blocked, VRAM blocked (DMG), CGB palettes blocked
    /// - PalettesBlock (6 ticks): VRAM fully blocked, enter Mode 3 rendering
    ///
    /// Total: 166 ticks
    fn tick_line0(&mut self, stage: Line0Stage, ints: &mut Interrupts, cgb_mode: CgbMode) {
        let is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);

        // Track dots during startup for computed STAT mode
        self.dots_in_line += 1;

        match stage {
            Line0Stage::InitialMode0 { remaining } => {
                // Phase 1: Mode 0 in STAT, all unblocked
                if remaining <= 1 {
                    // Transition to Phase 2: OAM write blocked (set on first cycle of Phase2)
                    self.oam_write_blocked = true;
                    self.phase = PpuPhase::Line0Startup(Line0Stage::OamWriteBlock { remaining: 4 });
                } else {
                    self.phase = PpuPhase::Line0Startup(Line0Stage::InitialMode0 {
                        remaining: remaining - 1,
                    });
                }
            }

            Line0Stage::OamWriteBlock { remaining } => {
                if remaining <= 1 {
                    // Transition to Phase 3: STAT = Mode 3, OAM fully blocked, CGB palettes blocked
                    self.set_mode_stat(Mode::Drawing);
                    self.oam_read_blocked = true;
                    // VRAM blocking depends on CGB/DMG
                    if !is_cgb {
                        self.vram_read_blocked = true;
                        self.vram_write_blocked = true;
                    }
                    self.cgb_palettes_blocked = true;
                    self.update_stat(ints);
                    self.phase = PpuPhase::Line0Startup(Line0Stage::StatMode3 { remaining: 4 });
                } else {
                    self.phase = PpuPhase::Line0Startup(Line0Stage::OamWriteBlock {
                        remaining: remaining - 1,
                    });
                }
            }

            Line0Stage::StatMode3 { remaining } => {
                // Phase 3: STAT = Mode 3, all blocked except VRAM (CGB)
                if remaining <= 1 {
                    // Transition to Phase 4: VRAM fully blocked
                    self.vram_read_blocked = true;
                    self.vram_write_blocked = true;
                    self.phase = PpuPhase::Line0Startup(Line0Stage::PalettesBlock { remaining: 6 });
                } else {
                    self.phase = PpuPhase::Line0Startup(Line0Stage::StatMode3 {
                        remaining: remaining - 1,
                    });
                }
            }

            Line0Stage::PalettesBlock { remaining } => {
                // Phase 4: All blocked, enter Mode 3 rendering
                if remaining <= 1 {
                    // Startup complete
                    self.enter_mode3_after_startup(ints);
                    self.phase = PpuPhase::Drawing;
                } else {
                    self.phase = PpuPhase::Line0Startup(Line0Stage::PalettesBlock {
                        remaining: remaining - 1,
                    });
                }
            }
        }
    }

    /// Enter Mode 3 rendering after startup sequence completes.
    fn enter_mode3_after_startup(&mut self, ints: &mut Interrupts) {
        self.phase = PpuPhase::Drawing;
        self.mode_for_interrupt = Some(Mode::Drawing);
        // STAT already set to Mode 3 at dot 79

        // Memory blocking already set during startup sequence

        self.sprite_fetcher_state = SpriteFetcherState::Idle;

        // Initialize drawing state.
        // cycles_for_line is augmented by 8 extra cycles for first line (16 ticks).
        // Startup duration 166 + 16 = 182.
        self.dots_in_line = 182;
        self.fetcher_state = FetcherState::GetTileT1;
        self.fetcher_step = 0;
        self.window_tile_x = 0;
        self.position_in_line = -16 - (self.scx & 7) as i16;
        self.lcd_x = 0;
        self.bg_fifo.clear();
        self.oam_fifo.clear();
        // Push 8 "junk" pixels to prime the FIFO (will be discarded during scroll)
        self.bg_fifo.push_bg_row(0, 0, 0, false, false);
        self.wx_triggered = false;
        // window_y starts at -1 (0xFF), incremented when window activates.
        self.window_y = 0xFF;
        // Reset per-line flags
        self.line_has_fractional_scrolling = false;
        self.window_is_being_fetched = false;
        self.window_activation_delay = 0;
        // Note: No OAM scan happened, so sprite_buffer stays empty for first line after LCD on

        // Clear sprite buffer and visible object count
        self.sprite_buffer.clear();

        // The startup line (line 0 after LCD-on) ends 8 T-cycles (16 half-clocks) earlier than a
        // normal line (SameBoy: cycles_for_line += 8).  Set a flag so tick_hblank triggers PreEnd
        // 16 ticks early, shortening line 0's HBlank by the correct amount.
        self.first_line_short = true;

        self.update_stat(ints);
    }

    /// Tick during Mode 2 (OAM Scan) using hierarchical state machine.
    /// Timing (all in ticks = 8MHz half-cycles):
    /// - Entry (State 35): 4 ticks - OAM write blocked on CGB (non-double-speed)
    /// - LyUpdate (State 6): 2 ticks - LY update, OAM read blocked
    /// - StatUpdate (State 7): 2 ticks - STAT = Mode 2, OAM fully blocked
    /// - Scan (State 8): 160 ticks - 40 OAM entries × 4 ticks each
    /// - Transition1 (State 10): 6 ticks - Mode 3 transition, VRAM blocked
    /// - Transition2 (State 32): 4 ticks - CGB palettes blocked
    fn tick_oam_scan(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        let is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);

        let PpuPhase::OamScan(stage) = self.phase else {
            return;
        };

        match stage {
            OamScanStage::Running { tick } => {
                // SameBoy-accurate timing (in 8MHz ticks)

                // Tick 0: LY update and Mode 2 interrupt pulse.
                if tick == 0 {
                    self.sprite_buffer.clear();

                    self.ly = self.current_line;
                    // LYC comparison now valid for the new line
                    self.ly_for_comparison = u16::from(self.ly);

                    // Mode 2 interrupt fires at tick 0.
                    self.set_mode_stat(Mode::OamScan);
                    self.mode_for_interrupt = Some(Mode::OamScan);
                    self.update_stat(ints);
                }

                // Tick 4: OAM read-blocking starts for non-double-speed (SameBoy State 7).
                // gambatte preread_1 / preread_2 boundary:
                //   read AT Running{tick:4} → blocked (0xFF / 0x03 masked)
                //   read AT Running{tick:3} → unblocked (real value)
                // Tick 3: OAM write blocking for CGB.
                if tick == 3 && is_cgb {
                    self.oam_write_blocked = true;
                }

                // Tick 4: OAM read-blocking starts for non-double-speed (SameBoy State 7).
                if tick == 4 {
                    self.oam_read_blocked = !double_speed;
                }

                // Tick 9 → tick 10: full OAM blocking for double-speed (SameBoy State 7
                // unconditional path).  In double-speed mode the tick-4 path only sets
                // `oam_read_blocked = false`; the unconditional block for all models
                // takes effect as we enter tick 10.
                // gambatte preread_ds_2: read AT Running{tick:10} → blocked (0x03 masked).
                if tick == 9 && double_speed {
                    self.oam_read_blocked = true;
                }

                // Tick 10: complete OAM-scan memory blocking (SameBoy State 7).
                if tick == 10 {
                    self.oam_read_blocked = true;
                    self.oam_write_blocked = true;
                    self.update_stat(ints);
                }

                // OAM Scan Loop (40 entries * 4 ticks = 160 ticks)
                if tick >= 8 && tick < 168 {
                    let scan_tick = tick - 8;
                    let entry = (scan_tick / 4) as u8;
                    let sub_tick = (scan_tick % 4) as u8;

                    if sub_tick == 0 && is_cgb {
                        self.scan_oam_entry_at(entry);
                    }
                    if sub_tick == 2 && !is_cgb {
                        self.scan_oam_entry_at(entry);
                    }

                    // Entry 37 memory unblocking (ticks 158-159)
                    if entry == 37 && sub_tick == 2 {
                        self.vram_read_blocked = !is_cgb;
                        self.vram_write_blocked = false;
                        self.cgb_palettes_blocked = false;
                        self.oam_write_blocked = is_cgb;
                    }
                }

                // STAT bits change to Mode 3 at tick 168
                if tick == 168 {
                    self.set_mode_stat(Mode::Drawing);
                    self.mode_for_interrupt = Some(Mode::Drawing);
                    self.update_stat(ints);

                    // Memory fully blocked when transitioning to Mode 3 STAT
                    self.vram_read_blocked = true;
                    self.vram_write_blocked = true;
                    self.oam_read_blocked = true;
                    self.oam_write_blocked = true;

                    // Transition to Mode 3 Rendering (Tick 168)
                    self.cgb_palettes_blocked = true;
                    self.enter_mode3_from_oam_scan(ints);
                } else {
                    self.phase = PpuPhase::OamScan(OamScanStage::Running { tick: tick + 1 });
                }
            }
        }
    }

    /// Scan OAM entry at a specific index.
    fn scan_oam_entry_at(&mut self, index: u8) {
        self.oam_scan_index = index;
        self.scan_oam_entry();
    }

    /// Enter Mode 3 (Drawing) after OAM scan completes.
    fn enter_mode3_from_oam_scan(&mut self, _ints: &mut Interrupts) {
        self.phase = PpuPhase::Drawing;

        // Memory blocking already set during Transition1/2

        // Initialize drawing state
        self.fetcher_state = FetcherState::GetTileT1;
        self.fetcher_step = 0;
        self.window_tile_x = 0;
        self.position_in_line = -16 - (self.scx & 7) as i16;
        self.lcd_x = 0;
        self.bg_fifo.clear();
        self.oam_fifo.clear();
        // Push 8 "junk" pixels to prime the FIFO (will be discarded during scroll)
        self.bg_fifo.push_bg_row(0, 0, 0, false, false);
        self.sprite_fetcher_state = SpriteFetcherState::Idle;

        // Reset per-line flags
        self.line_has_fractional_scrolling = false;
        self.window_is_being_fetched = false;
        self.window_activation_delay = 0;
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
        let height: i16 = if self.lcdc & LCDC_OBJL_B != 0 { 16 } else { 8 };

        // Check if sprite is on this scanline
        // Sprite Y is offset by 16, so visible range is Y-16 to Y-16+height-1
        // Cast to i16 to handle partial top visibility (when y < 16)
        let sprite_y = i16::from(y) - 16;
        let ly = i16::from(self.ly);

        if sprite_y <= ly && sprite_y + height > ly {
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
    fn tick_drawing(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode) {
        // Check for window activation
        self.check_window_trigger(cgb_mode);

        // Handle window activation delay (1 cycle stall after window triggers)
        if self.window_activation_delay > 0 {
            self.window_activation_delay -= 1;
            return;
        }

        // Check for sprites at current position.
        // x_for_object_match: position_in_line + 8, clamped to 0 if overflow.
        let match_x = {
            let raw = self.position_in_line.wrapping_add(8) as u8;
            // If raw > 240 (i.e., position_in_line was < -8), use 0
            if raw > 240 { 0 } else { raw }
        };

        // Handle sprite fetch state machine
        if self.sprite_fetcher_state != SpriteFetcherState::Idle {
            self.tick_sprite_fetcher(cgb_mode);
            return;
        }

        // Check if we should start fetching sprites.
        // Multiple sprites at the same X are fetched consecutively (matching SameBoy behavior).
        let sprites_enabled = self.lcdc & LCDC_OBJ_B != 0 || matches!(cgb_mode, CgbMode::Cgb);
        if sprites_enabled
            && let Some(sprite) = self.sprite_buffer.peek()
            && sprite.x == match_x
        {
            let sprite = self.sprite_buffer.pop().unwrap();
            self.start_sprite_fetch(sprite, cgb_mode);
            // Continue with sprite fetcher on next tick
            self.tick_sprite_fetcher(cgb_mode);
            return;
        }

        // Render pixel if possible THEN advance fetcher state machine.
        // Try to output a pixel first (output_pixel handles empty FIFO checks internally).
        // Run every 2 ticks (1 T-cycle).
        if !self.dots_in_line.is_multiple_of(2) {
            self.output_pixel(cgb_mode);
        }

        // Advance fetcher every tick (it handles its own 2-tick wait states now)
        self.advance_fetcher(cgb_mode, false);

        // HBlank interrupt fires 6 dots (12 ticks) before Mode 3 ends on hardware.
        if self.position_in_line >= 154 && self.mode_for_interrupt != Some(Mode::HBlank) {
            self.set_mode_stat(Mode::HBlank);
            self.mode_for_interrupt = Some(Mode::HBlank);
            self.update_stat(ints);
        }

        // Check if line rendering is complete
        if self.position_in_line >= 160 {
            // End of Mode 3 handling.
            // Reset window_y at line 143 (last visible line).
            if self.current_line == 143 {
                self.window_y = 0xFF; // -1 in unsigned, will wrap to 0 on first increment
            }

            // WX=166 DMG special case - triggers window for next line.
            let is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);
            if !is_cgb && self.wy_triggered && self.is_window_enabled() && self.wx == 166 {
                self.wx_triggered = true;
                self.window_tile_x = 1;
                self.window_y = self.window_y.wrapping_add(1);
            } else {
                self.wx_triggered = false;
            }

            // Transition to HBlank.
            self.phase = PpuPhase::HBlank(HBlankStage::StatUpdate { remaining: 2 });

            // Update STAT bits immediately (SameBoy accurate).
            self.set_mode_stat(Mode::HBlank);
            self.mode_for_interrupt = Some(Mode::HBlank);
            self.update_stat(ints);
        }
    }

    /// Start fetching a sprite.
    fn start_sprite_fetch(&mut self, sprite: sprite::SpriteEntry, cgb_mode: CgbMode) {
        // Calculate tile address
        let height_16 = self.lcdc & LCDC_OBJL_B != 0;
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
    /// Sprite fetch sequence:
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
                // State 27: Wait loop.
                // Condition: while (fetcher_state < 5 || fifo_size == 0)
                // fetcher_state >= 5 means GetDataHighT2, PushT1, or PushT2.
                let fetcher_aligned = matches!(
                    self.fetcher_state,
                    FetcherState::GetDataHighT2 | FetcherState::Push
                );
                let fifo_not_empty = self.bg_fifo.size() > 0;

                if fetcher_aligned && fifo_not_empty {
                    // Exit wait loop. The advance_fetcher call below is SameBoy's "free advance"
                    // (no sleep cost in state 27 exit). State 41 is exactly 1 cycle = 2 T-ticks.
                    // Since the current WaitForBgFetcher tick itself runs advance_fetcher (counting
                    // as the first T-tick of State41), we set step=1 so State41Advance only needs
                    // 1 more tick to complete (matching SameBoy's 12 T-tick total sprite fetch).
                    self.sprite_fetcher_state = SpriteFetcherState::State41Advance;
                    self.sprite_fetcher_step = 1;
                } else {
                    // Matches State 27 (Loop body advance).
                    // Stay in WaitForBgFetcher.
                }

                self.advance_fetcher(cgb_mode, true);
            }

            SpriteFetcherState::State41Advance => {
                // State 41: 1 cycle (2 ticks), then do "free" advance and transition.
                self.advance_fetcher(cgb_mode, true);
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 4 {
                    self.sprite_fetcher_state = SpriteFetcherState::GetTileAndFlags;
                    self.sprite_fetcher_step = 0;
                }
            }

            SpriteFetcherState::GetTileAndFlags => {
                // State 20
                if self.sprite_fetcher_step < 4 {
                    self.advance_fetcher(cgb_mode, true);
                }
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 6 {
                    self.sprite_fetcher_state = SpriteFetcherState::GetDataLow;
                    self.sprite_fetcher_step = 0;
                }
            }

            SpriteFetcherState::GetDataLow => {
                // State 39
                if self.sprite_fetcher_step == 0 {
                    self.sprite_tile_data_low = self
                        .vram
                        .vram_at_bank(self.sprite_tile_address, self.sprite_vram_bank);
                }
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 6 {
                    self.sprite_fetcher_state = SpriteFetcherState::GetDataHighAndPush;
                    self.sprite_fetcher_step = 0;
                }
            }

            SpriteFetcherState::GetDataHighAndPush => {
                // State 40
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 4 {
                    self.sprite_tile_data_high = self
                        .vram
                        .vram_at_bank(self.sprite_tile_address + 1, self.sprite_vram_bank);

                    // Overlay sprite onto OAM FIFO
                    self.oam_fifo.overlay_sprite_row(
                        self.sprite_tile_data_low,
                        self.sprite_tile_data_high,
                        self.sprite_palette,
                        self.sprite_bg_priority,
                        self.sprite_priority,
                        self.sprite_x_flip,
                    );

                    // Done fetching this sprite; transition back to Idle.
                    self.sprite_fetcher_state = SpriteFetcherState::Idle;
                }
            }
        }
    }

    /// Check if window should be activated at the current position.
    fn check_window_trigger(&mut self, cgb_mode: CgbMode) {
        // Window already triggered for this scanline
        if self.wx_triggered {
            return;
        }

        // Check if window is enabled
        let window_enabled = self.lcdc & LCDC_WIN_B != 0;

        if !window_enabled {
            return;
        }

        // Check WY condition (window Y trigger) - must have been triggered on or before current line
        if !self.wy_triggered {
            return;
        }

        let is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);
        let pos = self.position_in_line;

        // WX=0 has special handling.
        if self.wx == 0 {
            let should_activate = if pos == -7 || pos == -16 && (self.scx & 7) != 0 {
                true
            } else {
                (-15..=-8).contains(&pos)
            };

            if should_activate {
                self.activate_window(is_cgb);
            }
        }
        // WX < 166 (or 167 on CGB) - normal window trigger.
        else if self.wx < 166 + u8::from(is_cgb) {
            // Window activates when position_in_line + 7 == WX.
            if pos + 7 == i16::from(self.wx) {
                self.activate_window(is_cgb);
            }
        }
        // WX=166 on DMG - special case, increment window_y but don't fully trigger.
        else if !is_cgb && self.wx == 166 && pos + 7 == i16::from(self.wx) {
            // Just increment window line counter without full window activation.
            self.window_y = self.window_y.wrapping_add(1);
        } else {
            // WX > 166 (or 167 on CGB) - window never activates.
        }
    }

    /// Activate window rendering.
    fn activate_window(&mut self, is_cgb: bool) {
        self.window_y = self.window_y.wrapping_add(1);
        self.window_tile_x = 0;

        // Clear BG FIFO and restart fetcher for window
        self.bg_fifo.clear();

        // Only WX=0 with (SCX & 7) != 0 on DMG adds 1 T-cycle (2 ticks) delay.
        // All other cases have no delay.
        if self.wx == 0 && (self.scx & 7) != 0 && !is_cgb {
            self.window_activation_delay = 2;
        } else {
            self.window_activation_delay = 0;
        }

        self.wx_triggered = true;
        self.fetcher_state = FetcherState::GetTileT1;
        self.window_is_being_fetched = true;
    }

    /// Advance the background/window fetcher state machine.
    /// Uses T1/T2 states:
    /// - T1: Calculate addresses/setup (2 ticks)
    /// - T2: Perform VRAM read (2 ticks)
    fn advance_fetcher(&mut self, cgb_mode: CgbMode, during_sprite_fetch: bool) {
        // Implement 8MHz timing: each state takes 2 ticks (1 T-cycle)
        // Wait 1 tick before doing work and transitioning
        if self.fetcher_step == 0 {
            self.fetcher_step = 1;
            return;
        }
        self.fetcher_step = 0;

        match self.fetcher_state {
            FetcherState::GetTileT1 => {
                // T1: Calculate tile map address.
                // Clear wx_triggered if window is disabled.
                if !self.is_window_enabled() {
                    self.wx_triggered = false;
                }

                let tile_map = if self.wx_triggered {
                    self.win_tile_map_addr()
                } else {
                    self.bg_tile_map_addr()
                };

                let y = if self.wx_triggered {
                    self.window_y
                } else {
                    self.ly.wrapping_add(self.scy)
                };

                let x = if self.wx_triggered {
                    self.window_tile_x
                } else {
                    // BG X calculation
                    let scx = self.scx;
                    let offset = u8::from(
                        matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat) && !during_sprite_fetch,
                    );
                    let pos = self
                        .position_in_line
                        .wrapping_add(8)
                        .wrapping_sub(i16::from(offset));
                    let scx_adj = scx.wrapping_add(pos as u8);
                    scx_adj / 8
                } & 0x1F;

                // Cache address for T2
                self.fetcher_tile_index_addr = tile_map + u16::from(y / 8) * 32 + u16::from(x);
                self.fetcher_state = FetcherState::GetTileT2;
            }
            FetcherState::GetTileT2 => {
                // T2: Read tile index and attributes from VRAM
                self.current_tile = self.vram.vram_at_bank(self.fetcher_tile_index_addr, 0);
                self.current_tile_attrs = match cgb_mode {
                    CgbMode::Cgb | CgbMode::Compat => {
                        self.vram.vram_at_bank(self.fetcher_tile_index_addr, 1)
                    }
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
                // T1: Recalculate tile data address for high byte.
                // Re-read LCDC here to handle mid-fetch changes.
                self.fetcher_tile_data_addr = self.calculate_tile_data_addr() + 1;
                self.fetcher_state = FetcherState::GetDataHighT2;
            }
            FetcherState::GetDataHighT2 => {
                // T2: Read high byte from VRAM.
                self.current_tile_data[1] = self.read_tile_byte(self.fetcher_tile_data_addr);

                // Increment window_tile_x AFTER reading high byte.
                if self.wx_triggered {
                    self.window_tile_x = self.window_tile_x.wrapping_add(1) & 0x1F;
                }

                // Fallthrough to PUSH logic immediately.
                // If FIFO is empty, push and go to GetTileT1 (0 cycles for push).
                if self.bg_fifo.is_empty() {
                    self.push_to_fifo();
                    self.fetcher_state = FetcherState::GetTileT1;
                } else {
                    // FIFO not empty, wait in Push state
                    self.fetcher_state = FetcherState::Push;
                }
            }
            FetcherState::Push => {
                // Wait for FIFO to be empty
                if self.bg_fifo.is_empty() {
                    self.push_to_fifo();
                    self.fetcher_state = FetcherState::GetTileT1;
                }
                // Else stay in Push state
            }
        }
    }

    /// Helper to push current tile data to BG FIFO.
    fn push_to_fifo(&mut self) {
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
        // Note: window_tile_x increment moved to GetDataHighT2.
    }

    /// Calculate tile data address for current tile.
    fn calculate_tile_data_addr(&self) -> u16 {
        let y = if self.wx_triggered {
            self.window_y
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
        let base = if self.lcdc & LCDC_BG_SIGNED == 0 {
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
        if self.vram_ppu_blocked || self.ext_hdma_active {
            return 0xFF;
        }
        let bank = (self.current_tile_attrs >> 3) & 1;
        self.vram.vram_at_bank(addr, bank)
    }

    /// Output a pixel to the LCD buffer.
    /// Implements `render_pixel_if_possible` logic.
    fn output_pixel(&mut self, cgb_mode: CgbMode) {
        // FIFO empty check FIRST, before anything else.
        if self.bg_fifo.is_empty() {
            return;
        }

        // SCX alignment jump logic
        if self.position_in_line >= -16 && self.position_in_line < -8 {
            if (self.position_in_line as u8 & 7) == (self.scx & 7) {
                self.position_in_line = -8;
            } else if self.window_is_being_fetched
                && (self.position_in_line as u8 & 7) == 6
                && (self.scx & 7) == 7
            {
                self.position_in_line = -8;
            }
        }

        // Pop from BG FIFO.
        let bg_pixel = self.bg_fifo.pop().unwrap();
        let sprite_pixel = self.oam_fifo.pop();

        self.window_is_being_fetched = false;

        // Drop pixels for scrolling or priming.
        // Screen starts at position_in_line = 0.
        if self.position_in_line < 0 {
            self.position_in_line += 1;
            return;
        }

        // Drop pixels if we've reached the end of the visible line.
        if self.lcd_x >= PX_WIDTH {
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

        let idx = u32::from(self.ly) * u32::from(PX_WIDTH) + u32::from(self.lcd_x);
        // Safety check: only write to visible area
        if self.ly < PX_HEIGHT {
            self.rgb_buf.set_px(idx, rgb);
        }

        self.lcd_x += 1;
        self.position_in_line += 1;
    }

    /// Mix background and sprite pixels according to priority rules.
    fn mix_pixels(
        &self,
        bg_pixel: fifo::FifoPixel,
        sprite_pixel: Option<fifo::FifoPixel>,
        cgb_mode: CgbMode,
    ) -> (u8, u8, bool) {
        // Check if BG is enabled (LCDC bit 0)
        // On DMG: When disabled, BG renders as color 0 (white)
        // On CGB: This bit acts as master priority, not enable/disable
        let bg_enabled = self.lcdc & LCDC_BG_B != 0 || matches!(cgb_mode, CgbMode::Cgb);
        let effective_bg_color = if bg_enabled { bg_pixel.color } else { 0 };

        // Check if sprites are enabled (LCDC bit 1)
        // On DMG: Sprites are disabled if bit is 0
        // On CGB: Sprites are ALWAYS processed regardless of this bit (checked at pop time)
        let sprites_enabled = self.lcdc & LCDC_OBJ_B != 0;

        let Some(sprite) = sprite_pixel else {
            return (effective_bg_color, bg_pixel.palette, false);
        };

        // If sprites are disabled, use background
        // On CGB, this check still applies - sprites won't render if OBJ bit is 0
        if !sprites_enabled {
            return (effective_bg_color, bg_pixel.palette, false);
        }

        // Transparent sprite pixel - use background
        if sprite.color == 0 {
            return (effective_bg_color, bg_pixel.palette, false);
        }

        // Check sprite priority
        let sprite_behind_bg = sprite.bg_priority;
        let bg_priority = bg_pixel.bg_priority;
        let bg_opaque = effective_bg_color != 0;

        // Determine if sprite should be hidden behind BG
        let hide_sprite = match cgb_mode {
            CgbMode::Dmg => {
                // DMG: BG/OBJ priority from sprite attribute
                sprite_behind_bg && bg_opaque
            }
            CgbMode::Compat => {
                // Compat: Respect BG priority bit (CGB hardware behavior)
                (sprite_behind_bg || bg_priority) && bg_opaque
            }
            CgbMode::Cgb => {
                // CGB: LCDC bit 0 acts as master priority
                if self.lcdc & LCDC_BG_B == 0 {
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
            (sprite.color, sprite.palette, true)
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
                self.ocp.rgb(palette, shade, self.color_correction_mode)
            }
            CgbMode::Cgb => self.ocp.rgb(palette, color, self.color_correction_mode),
        }
    }

    /// Get background tile map address.
    #[inline]
    const fn bg_tile_map_addr(&self) -> u16 {
        if self.lcdc & LCDC_BG_AREA != 0 {
            0x9C00
        } else {
            0x9800
        }
    }

    /// Get window tile map address.
    #[inline]
    const fn win_tile_map_addr(&self) -> u16 {
        if self.lcdc & LCDC_WIN_AREA != 0 {
            0x9C00
        } else {
            0x9800
        }
    }

    /// Check if window is enabled in LCDC.
    #[inline]
    const fn is_window_enabled(&self) -> bool {
        self.lcdc & LCDC_WIN_B != 0
    }

    /// Monochrome palette lookup.
    #[must_use]
    const fn mono_rgb(shade: u8) -> (u8, u8, u8) {
        color_palette::GRAYSCALE_PALETTE[shade as usize]
    }

    /// Tick during Mode 0 (HBlank).
    /// Timing:
    /// - State 22 (2 ticks): STAT = Mode 0, memory unblocked, fire STAT interrupt
    /// - State 33 (4 ticks): CGB palettes blocked (non-double-speed only)
    /// - State 36 (4 ticks): CGB palettes unblocked
    /// - State 11 (variable): Wait until line ends (912 ticks total)
    /// - State 31 (4 ticks): Set mode_for_interrupt = 2 for next line
    fn tick_hblank(&mut self, ints: &mut Interrupts, double_speed: bool) {
        // Line length is 456 T-cycles = 912 ticks
        const LINE_LENGTH_TICKS: u16 = 912;
        // PreEnd starts 4 ticks before line end
        const PRE_END_START: u16 = LINE_LENGTH_TICKS - 8;

        let PpuPhase::HBlank(stage) = self.phase else {
            return;
        };

        match stage {
            HBlankStage::StatUpdate { remaining } => {
                // State 22: STAT = Mode 0, memory unblocked.
                if remaining == 2 {
                    self.set_mode_stat(Mode::HBlank);
                    self.mode_for_interrupt = Some(Mode::HBlank);
                    self.update_stat(ints);

                    self.oam_read_blocked = false;
                    self.vram_read_blocked = false;
                    self.oam_write_blocked = false;
                    self.vram_write_blocked = false;
                }

                if remaining <= 1 {
                    self.phase = PpuPhase::HBlank(HBlankStage::PalettesBlock { remaining: 4 });
                } else {
                    self.phase = PpuPhase::HBlank(HBlankStage::StatUpdate {
                        remaining: remaining - 1,
                    });
                }
            }

            HBlankStage::PalettesBlock { remaining } => {
                // State 33: CGB palettes blocked (non-double-speed only).
                if remaining == 4 && !double_speed {
                    self.cgb_palettes_blocked = true;
                }

                if remaining <= 1 {
                    self.phase = PpuPhase::HBlank(HBlankStage::PalettesUnblock { remaining: 4 });
                } else {
                    self.phase = PpuPhase::HBlank(HBlankStage::PalettesBlock {
                        remaining: remaining - 1,
                    });
                }
            }

            HBlankStage::PalettesUnblock { remaining } => {
                // State 36: CGB palettes unblocked.
                if remaining == 4 {
                    self.cgb_palettes_blocked = false;
                    // TODO: HDMA trigger check
                }

                if remaining <= 1 {
                    // Transition to Remainder
                    self.phase = PpuPhase::HBlank(HBlankStage::Remainder);
                } else {
                    self.phase = PpuPhase::HBlank(HBlankStage::PalettesUnblock {
                        remaining: remaining - 1,
                    });
                }
            }

            HBlankStage::Remainder => {
                // State 11: Wait for line to near-complete.
                // We wait until 4 ticks before line end, then transition to PreEnd.
                //
                // The LCD-on startup line (line 0) is 16 half-clocks (8 T-cycles) shorter than a
                // normal line (SameBoy display.c: cycles_for_line += 8).  When first_line_short is
                // set we trigger PreEnd 16 ticks early so line 0 ends at tick 896 instead of 912.
                let threshold = if self.first_line_short {
                    PRE_END_START - 16
                } else {
                    PRE_END_START
                };
                if self.dots_in_line >= threshold {
                    self.first_line_short = false; // consumed — clear for subsequent lines
                    self.phase = PpuPhase::HBlank(HBlankStage::PreEnd { remaining: 8 });
                }
                // Otherwise stay in Remainder
            }

            HBlankStage::PreEnd { remaining } => {
                // State 31: Pre-end, set mode_for_interrupt = 2 for next line.
                // Use current_line, not ly.
                if remaining == 4 && self.current_line != 143 {
                    // Prepare Mode 2 interrupt for next line (LineStart)
                    self.mode_for_interrupt = Some(Mode::OamScan);
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    // Line complete - transition to next line
                    self.dots_in_line = 0;
                    self.current_line = self.current_line.wrapping_add(1);

                    if self.current_line == self.wy {
                        self.wy_triggered = true;
                    }

                    // Check if we should enter VBlank (after line 143)
                    if self.current_line >= 144 {
                        self.enter_mode(Mode::VBlank, ints);
                    } else {
                        // Next visible line
                        self.oam_scan_index = 0;
                        self.sprite_buffer.clear();

                        self.oam_read_blocked = false;
                        self.oam_write_blocked = false;
                        self.vram_read_blocked = false;
                        self.vram_write_blocked = false;
                        self.cgb_palettes_blocked = false;
                        // Note: wx_triggered is reset at end of Mode 3, not here
                        // This allows WX=166 DMG quirk to persist across lines

                        self.enter_mode(Mode::OamScan, ints);
                    }
                } else {
                    self.phase = PpuPhase::HBlank(HBlankStage::PreEnd {
                        remaining: remaining - 1,
                    });
                }
            }
        }
    }

    /// Tick during Mode 1 (VBlank).
    fn tick_vblank(&mut self, ints: &mut Interrupts) {
        let PpuPhase::VBlank(mut stage) = self.phase else {
            return;
        };

        match stage {
            VBlankStage::LycReset { ref mut remaining } => {
                // State 26: 2 cycles (4 ticks).
                if *remaining == 4 {
                    // Start of VBlank line logic.
                    // ly_for_comparison = -1 (0xFFFF).
                    self.ly_for_comparison = 0xFFFF;
                    self.stat &= !STAT_LYC_B; // Clear coincidence flag when comparison disabled
                    self.update_stat(ints);
                }

                if *remaining <= 1 {
                    self.phase = PpuPhase::VBlank(VBlankStage::LyUpdate { remaining: 4 });
                    return;
                }
                *remaining -= 1;
            }

            VBlankStage::LyUpdate { ref mut remaining } => {
                // State 12: 2 cycles (4 ticks)
                if *remaining == 4 {
                    self.ly = self.current_line;

                    // OAM interrupt quirk: at line 144, if Mode 2 OAM STAT interrupt is
                    // enabled and the STAT interrupt line was previously low, fire an
                    // additional STAT interrupt. (SameBoy display.c ~line 2160)
                    if self.current_line == PX_HEIGHT
                        && !self.stat_interrupt_line
                        && (self.stat & STAT_IF_OAM_B != 0)
                    {
                        ints.request_lcd();
                    }
                }

                if *remaining <= 1 {
                    self.phase = PpuPhase::VBlank(VBlankStage::LycUpdate { remaining: 2 });
                    return;
                }
                *remaining -= 1;
            }

            VBlankStage::LycUpdate { ref mut remaining } => {
                // State 24: 1 cycle (2 ticks).
                if *remaining <= 1 {
                    self.ly_for_comparison = u16::from(self.ly);
                    self.update_stat(ints);

                    // Step 10: VBlank Entry Logic (Line 144 only).
                    if self.current_line == PX_HEIGHT {
                        // Enter Mode 1 (VBlank) officially.
                        self.set_mode_stat(Mode::VBlank);
                        ints.request_vblank();
                        self.wy_triggered = false; // Reset WY trigger.

                        self.mode_for_interrupt = Some(Mode::VBlank);
                        self.update_stat(ints);
                    }

                    // Total used: 4+4+2 = 10 ticks.
                    // Remainder = 912 - 10 = 902.
                    self.phase = PpuPhase::VBlank(VBlankStage::Remainder { remaining: 902 });
                    return;
                }
                *remaining -= 1;
            }

            VBlankStage::Remainder { ref mut remaining } => {
                if *remaining <= 1 {
                    // End of VBlank line
                    self.dots_in_line = 0;
                    self.current_line = self.current_line.wrapping_add(1);

                    if self.current_line == 153 {
                        // Transition to Line 153 (Special)
                        self.phase = PpuPhase::Line153(Line153Stage::default());
                    } else if self.current_line > 153 {
                        // Should be handled by Line153 logic, but fallback if we stay in VBlank?
                        // If we are in VBlank and current_line > 153, we wrap to 0.
                        self.finish_frame_and_start_new(ints);
                    } else {
                        // Next VBlank line (145-152)
                        self.phase = PpuPhase::VBlank(VBlankStage::default());
                    }
                    return;
                }
                *remaining -= 1;
            }
        }

        self.phase = PpuPhase::VBlank(stage);
    }

    /// Complete the current frame and start a new one.
    fn finish_frame_and_start_new(&mut self, ints: &mut Interrupts) {
        self.current_line = 0;
        if self.wy == 0 {
            self.wy_triggered = true;
        }
        self.ly_for_comparison = 0xFFFF;
        self.stat &= !STAT_LYC_B; // Clear coincidence flag when comparison disabled
        self.dots_in_line = 0;

        // Present frame
        if self.frame_skip_state == FrameSkipState::LcdTurnedOn {
            self.frame_skip_state = FrameSkipState::FirstFrameRendered;
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
        // window_y starts at -1 (0xFF), incremented to 0 when window first activates.
        // The reset at line 143 handles this for subsequent frames.
        self.window_y = 0xFF;
        self.wx_triggered = false;
        self.enter_mode(Mode::OamScan, ints);
    }

    pub const fn write_bgp(&mut self, val: u8) {
        self.bgp = val;
    }

    pub fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts) {
        // turn off
        if val & LCDC_ON_B == 0 && self.lcdc & LCDC_ON_B != 0 {
            self.ly = 0;
            self.ly_for_comparison = 0;
            self.current_line = 0;
            let mode = Mode::HBlank;
            self.set_mode_stat(mode);
            self.rgba_buf_present.clear();

            // Reset startup state
            self.phase = PpuPhase::LcdOff;
            self.frame_skip_state = FrameSkipState::Normal;
            self.dots_in_line = 0;

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
                (self.stat & STAT_LYC_B != 0) && (self.stat & STAT_IF_LYC_B != 0);
        }

        // turn on
        if val & LCDC_ON_B != 0 && self.lcdc & LCDC_ON_B == 0 {
            self.ly = 0;
            self.ly_for_comparison = 0;
            self.current_line = 0;
            let mode = Mode::HBlank;
            self.set_mode_stat(mode);
            self.mode_for_interrupt = None;
            // Comparison clock restarts - update coincidence and check for interrupt
            self.update_stat(ints);

            // Start LCD startup state machine with Phase 1 (152 ticks)
            // Total: 152 + 4 + 4 + 6 = 166 ticks
            self.phase = PpuPhase::Line0Startup(Line0Stage::InitialMode0 { remaining: 152 });
            self.oam_read_blocked = false;
            self.oam_write_blocked = false;
            self.vram_read_blocked = false;
            self.vram_write_blocked = false;
            self.cgb_palettes_blocked = false;

            // First line after LCD on has no OAM scan
            self.sprite_buffer.clear();

            // Mark as first frame for special timing
            self.frame_skip_state = FrameSkipState::LcdTurnedOn;
            self.dots_in_line = 0;
            if self.wy == 0 {
                self.wy_triggered = true;
            }
        }

        self.lcdc = val;
    }

    pub fn write_lyc(&mut self, val: u8, ints: &mut Interrupts) {
        self.lyc = val;
        // LYC change may affect coincidence - update STAT line if LCD is on.
        // A CPU write to LYC always recalculates the LYC=LY flag against the
        // current LY register, even during the window where ly_for_comparison
        // is 0xFFFF (which normally suppresses the internal comparison).
        if self.lcdc & LCDC_ON_B != 0 {
            self.stat &= !STAT_LYC_B;
            if u16::from(val) == u16::from(self.ly) {
                self.stat |= STAT_LYC_B;
            }
            self.update_stat(ints);
        }
    }

    pub fn enter_stop_mode(&mut self) {
        self.is_frozen = true;
    }

    #[must_use]
    pub const fn is_frozen(&self) -> bool {
        self.is_frozen
    }

    pub fn leave_stop_mode(&mut self) {
        self.is_frozen = false;
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
        let ly_equals_lyc = self.stat & STAT_LYC_B;
        let mode: u8 = self.mode() as u8;

        self.stat = val;
        self.stat &= !(STAT_LYC_B | STAT_MODE_B);
        self.stat |= ly_equals_lyc | mode;

        // STAT write may change which interrupts are enabled - update line if LCD is on
        if self.lcdc & LCDC_ON_B != 0 {
            self.update_stat(ints);
        }
    }

    pub const fn write_wx(&mut self, val: u8) {
        self.wx = val;
    }

    pub fn write_wy(&mut self, val: u8) {
        self.wy = val;
        if self.current_line < 144 && self.current_line == val {
            self.wy_triggered = true;
        }
    }
}

#[cfg(test)]
mod tests;
