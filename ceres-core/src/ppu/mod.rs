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
#[derive(Clone, Copy, Debug, Default)]
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
    /// SameBoy uses this to fire Mode 2 interrupt slightly before STAT shows Mode 2.
    /// Value of -1 (represented as None) means no mode-based interrupt should fire.
    mode_for_interrupt: Option<Mode>,
    vram: Vram,
    wy_triggered: bool,
    wx: u8,
    wy: u8,

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
        // SameBoy: if (ly_for_comparison != -1 || (model <= CGB_C && !double_speed))
        // Note: We don't track model exactly like SameBoy, assuming CGB behavior
        // If ly_for_comparison is valid (not 0xFFFF), we compare.
        // If it is 0xFFFF:
        // - On DMG/CGB Double Speed: we do nothing (preserve flag)
        // - On CGB Single Speed: we clear flag?
        // Let's assume standard behavior for now: if 0xFFFF, don't update.
        // But checking SameBoy: "if (ly_fc != -1 || ...)" -> if -1, we might still enter if CGB Single Speed.
        // Inside: "if (ly_fc == lyc) ... else { if (ly_fc != -1) lyc_int=false; STAT &= ~4; }"
        // So if ly_fc == -1 and CGB Single Speed: we go to else, skip lyc_int=false, but CLEAR STAT bit.
        // So on CGB Single Speed, -1 clears the bit.
        // On others, -1 preserves it.
        // Let's implement the CGB Single Speed behavior (clear bit) if 0xFFFF, but check mode?
        // Ppu doesn't know about CGB double speed here, passed in tick but not stored?
        // Wait, update_stat doesn't take cgb_mode/double_speed.
        // We should probably just compare if valid for now.
        if self.ly_for_comparison != 0xFFFF {
            self.stat &= !STAT_LYC_B;
            if self.ly_for_comparison == u16::from(self.lyc) {
                self.stat |= STAT_LYC_B;
            }
        }

        // Compute new STAT interrupt line state from all enabled sources
        let mut new_line = false;

        // LY=LYC coincidence interrupt
        if (self.stat & STAT_IF_LYC_B != 0) && (self.stat & STAT_LYC_B != 0) {
            new_line = true;
        }

        // Mode-based interrupts use mode_for_interrupt (which can differ from STAT bits)
        // SameBoy: mode_for_interrupt can be set to 2 at the end of HBlank, before STAT changes
        // If mode_for_interrupt is None (SameBoy's -1), no mode interrupt is generated.
        if let Some(interrupt_mode) = self.mode_for_interrupt {
            match interrupt_mode {
                Mode::HBlank if self.stat & STAT_IF_HBLANK_B != 0 => new_line = true,
                Mode::VBlank if self.stat & STAT_IF_VBLANK_B != 0 => new_line = true,
                Mode::OamScan if self.stat & STAT_IF_OAM_B != 0 => new_line = true,
                _ => {}
            }
        }

        self.stat_interrupt_line = new_line;

        // Only fire interrupt on rising edge (low -> high transition)
        if new_line && !previous_line {
            ints.request_lcd();
        }
    }

    fn enter_mode(&mut self, mode: Mode, ints: &mut Interrupts) {
        match mode {
            Mode::OamScan => self.phase = PpuPhase::OamScan(OamScanStage::default()),
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
        // Compute mode from dots_in_line for accurate timing
        // This matches Age's approach of calculating mode on read
        let computed_mode = self.compute_stat_mode();
        (self.stat & !STAT_MODE_B) | computed_mode | 0x80
    }

    /// Compute STAT mode based on current timing state.
    /// This provides more accurate mode reporting than the maintained state
    /// for the first line after LCD enable (which has special timing).
    const fn compute_stat_mode(&self) -> u8 {
        // LCD off: mode 0
        if self.lcdc & LCDC_ON_B == 0 {
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

    pub fn run(&mut self, dots: i32, ints: &mut Interrupts, cgb_mode: CgbMode, double_speed: bool) {
        for _ in 0..dots {
            self.tick(ints, cgb_mode, double_speed);
        }
    }

    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.color_correction_mode = mode;
    }

    const fn set_mode_stat(&mut self, mode: Mode) {
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
        if self.lcdc & LCDC_ON_B == 0 {
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
                // State 17
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
    /// SameBoy timing for first line after LCD on (8MHz ticks):
    /// - InitialMode0 (152 ticks): Mode 0 in STAT, all unblocked
    /// - OamWriteBlock (4 ticks): OAM write blocked
    /// - StatMode3 (4 ticks): STAT = Mode 3, OAM fully blocked, VRAM blocked (DMG), CGB palettes blocked
    /// - PalettesBlock (6 ticks): VRAM fully blocked, enter Mode 3 rendering
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

        // Initialize drawing state
        // SameBoy: cycles_for_line is augmented by 8 extra cycles for first line (16 ticks).
        // Startup duration 166 + 16 = 182.
        self.dots_in_line = 182;
        self.fetcher_state = FetcherState::GetTileT1;
        self.fetcher_step = 0;
        self.window_tile_x = 0;
        // SameBoy: position_in_line starts at -16
        self.position_in_line = -16;
        self.lcd_x = 0;
        self.bg_fifo.clear();
        self.oam_fifo.clear();
        // Push 8 "junk" pixels to prime the FIFO (will be discarded during scroll)
        self.bg_fifo.push_bg_row(0, 0, 0, false, false);
        self.wx_triggered = false;
        // SameBoy: window_y starts at -1 (0xFF), incremented when window activates
        self.window_y = 0xFF;
        // Reset per-line flags
        self.line_has_fractional_scrolling = false;
        self.window_is_being_fetched = false;
        self.window_activation_delay = 0;
        // Note: No OAM scan happened, so sprite_buffer stays empty for first line after LCD on

        // Clear sprite buffer and visible object count
        self.sprite_buffer.clear();

        self.update_stat(ints);
    }

    /// Tick during Mode 2 (OAM Scan) using hierarchical state machine.
    /// SameBoy timing (all in ticks = 8MHz half-cycles):
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
            OamScanStage::Entry { remaining } => {
                // State 35: OAM write blocked on CGB (non-double-speed)
                // SameBoy line 1767: gb->oam_write_blocked = GB_is_cgb(gb) && !gb->cgb_double_speed;
                if remaining == 4 {
                    self.oam_write_blocked = is_cgb && !double_speed;
                }
                // SameBoy line 1771: After 2 cycles (4 ticks), upgrade to full CGB block
                if remaining == 2 && is_cgb {
                    self.oam_write_blocked = true;
                }

                if remaining <= 1 {
                    self.phase = PpuPhase::OamScan(OamScanStage::LyUpdate { remaining: 2 });
                } else {
                    self.phase = PpuPhase::OamScan(OamScanStage::Entry {
                        remaining: remaining - 1,
                    });
                }
            }

            OamScanStage::LyUpdate { remaining } => {
                // State 6: LY visible in register, OAM read blocked
                // SameBoy lines 1773-1787
                // Logic executes at START of state (remaining == 2)
                //
                // Note: LY was already incremented by HBlank transition (or is 0 for first frame)
                // Here we just make it visible and set up ly_for_comparison
                if remaining == 2 {
                    self.ly = self.current_line;
                    // SameBoy line 1776: ly_for_comparison = current_line ? -1 : 0
                    // This creates a window where LY=LYC comparison is undefined
                    self.ly_for_comparison = if self.current_line != 0 { 0xFFFF } else { 0 };
                    // SameBoy line 1775: oam_read_blocked depends on model
                    self.oam_read_blocked = !double_speed;

                    // SameBoy lines 1778-1787: Mode 2 interrupt fires 1 T-cycle early (except line 0)
                    // The OAM STAT interrupt occurs before STAT mode bits change to 2
                    if self.current_line != 0 {
                        self.mode_for_interrupt = Some(Mode::OamScan);
                        // SameBoy line 1782: Clear STAT mode bits to 0 (keeping them as HBlank)
                        self.set_mode_stat(Mode::HBlank);
                    } else if !is_cgb {
                        // DMG line 0: clear mode bits but don't set mode_for_interrupt
                        self.set_mode_stat(Mode::HBlank);
                    }
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    self.phase = PpuPhase::OamScan(OamScanStage::StatUpdate { remaining: 2 });
                } else {
                    self.phase = PpuPhase::OamScan(OamScanStage::LyUpdate {
                        remaining: remaining - 1,
                    });
                }
            }

            OamScanStage::StatUpdate { remaining } => {
                // State 7: STAT = Mode 2, OAM fully blocked
                // SameBoy lines 1789-1800
                // Logic executes at START of state (remaining == 2)
                if remaining == 2 {
                    self.oam_read_blocked = true;
                    self.set_mode_stat(Mode::OamScan);
                    self.oam_write_blocked = true;
                    self.ly_for_comparison = u16::from(self.ly);

                    // SameBoy: mode_for_interrupt = 2 for STAT update, then -1
                    self.mode_for_interrupt = Some(Mode::OamScan);
                    self.update_stat(ints);
                    self.mode_for_interrupt = None;
                    self.update_stat(ints);

                    // Initialize OAM scan
                    self.sprite_buffer.clear();
                    self.oam_scan_index = 0;
                }

                if remaining <= 1 {
                    // Transition to Scan state
                    self.phase = PpuPhase::OamScan(OamScanStage::Scan {
                        entry: 0,
                        sub_tick: 0,
                    });
                } else {
                    self.phase = PpuPhase::OamScan(OamScanStage::StatUpdate {
                        remaining: remaining - 1,
                    });
                }
            }

            OamScanStage::Scan { entry, sub_tick } => {
                // State 8: OAM scan loop
                // SameBoy lines 1807-1823
                // Each entry takes 4 ticks (2 8MHz cycles)
                // On CGB: scan happens at start of entry (sub_tick 0)
                // On DMG: scan happens after 2 ticks (sub_tick 2)

                if sub_tick == 0 && is_cgb {
                    // CGB scans at start
                    self.scan_oam_entry_at(entry);
                }

                if sub_tick == 2 && !is_cgb {
                    // DMG scans after 2 ticks
                    self.scan_oam_entry_at(entry);
                }

                // SameBoy line 1817-1822: At entry 37 + 2 ticks, update memory blocking
                if entry == 37 && sub_tick == 2 {
                    self.vram_read_blocked = !is_cgb;
                    self.vram_write_blocked = false;
                    self.cgb_palettes_blocked = false;
                    self.oam_write_blocked = is_cgb;
                }

                let next_sub_tick = sub_tick + 1;
                if next_sub_tick >= 4 {
                    // Entry complete
                    let next_entry = entry + 1;
                    if next_entry >= 40 {
                        // All entries scanned, transition to Mode 3 setup
                        self.phase = PpuPhase::OamScan(OamScanStage::Transition1 { remaining: 6 });
                    } else {
                        self.phase = PpuPhase::OamScan(OamScanStage::Scan {
                            entry: next_entry,
                            sub_tick: 0,
                        });
                    }
                } else {
                    self.phase = PpuPhase::OamScan(OamScanStage::Scan {
                        entry,
                        sub_tick: next_sub_tick,
                    });
                }
            }

            OamScanStage::Transition1 { remaining } => {
                // State 10: Mode 3 transition
                // SameBoy lines 1824-1840
                if remaining == 6 {
                    // Set up Mode 3
                    self.set_mode_stat(Mode::Drawing);
                    self.mode_for_interrupt = Some(Mode::Drawing);
                    self.vram_read_blocked = true;
                    self.vram_write_blocked = true;
                    self.oam_read_blocked = true;
                    self.oam_write_blocked = true;
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    self.phase = PpuPhase::OamScan(OamScanStage::Transition2 { remaining: 4 });
                } else {
                    self.phase = PpuPhase::OamScan(OamScanStage::Transition1 {
                        remaining: remaining - 1,
                    });
                }
            }

            OamScanStage::Transition2 { remaining } => {
                // State 32: CGB palettes blocked
                // SameBoy lines 1842-1844
                if remaining == 4 {
                    self.cgb_palettes_blocked = true;
                }

                if remaining <= 1 {
                    // Transition to Mode 3 (Drawing)
                    self.enter_mode3_from_oam_scan(ints);
                } else {
                    self.phase = PpuPhase::OamScan(OamScanStage::Transition2 {
                        remaining: remaining - 1,
                    });
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
        self.position_in_line = -16;
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
        let height = if self.lcdc & LCDC_OBJL_B != 0 { 16 } else { 8 };

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
    fn tick_drawing(&mut self, _ints: &mut Interrupts, cgb_mode: CgbMode) {
        // Check for window activation
        self.check_window_trigger(cgb_mode);

        // Handle window activation delay (1 cycle stall after window triggers)
        if self.window_activation_delay > 0 {
            self.window_activation_delay -= 1;
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
            return;
        }

        // Check if we should start fetching sprites
        let sprites_enabled = self.lcdc & LCDC_OBJ_B != 0 || matches!(cgb_mode, CgbMode::Cgb);
        if sprites_enabled {
            if let Some(sprite) = self.sprite_buffer.peek() {
                if sprite.x == match_x {
                    let sprite = self.sprite_buffer.pop().unwrap();
                    self.start_sprite_fetch(sprite, cgb_mode);
                    // Continue with sprite fetcher on next tick
                    self.tick_sprite_fetcher(cgb_mode);
                    return;
                }
            }
        }

        // SameBoy order: render_pixel_if_possible THEN advance_fetcher_state_machine
        // Try to output a pixel first (output_pixel handles empty FIFO checks internally)
        // Run every 2 ticks (1 T-cycle)
        // Start on even ticks relative to global counter, or local?
        // dots_in_line runs continuously.
        if self.dots_in_line % 2 == 0 {
            self.output_pixel(cgb_mode);
        }

        // Advance fetcher every tick (it handles its own 2-tick wait states now)
        self.advance_fetcher(cgb_mode, false);

        // Check if line rendering is complete
        if self.position_in_line >= 160 {
            // SameBoy lines 2077-2087: End of Mode 3 handling
            // Reset window_y at line 143 (last visible line)
            if self.current_line == 143 {
                self.window_y = 0xFF; // -1 in unsigned, will wrap to 0 on first increment
            }

            // SameBoy: WX=166 DMG special case - triggers window for next line
            let is_cgb = matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat);
            if !is_cgb && self.wy_triggered && self.is_window_enabled() && self.wx == 166 {
                self.wx_triggered = true;
                self.window_tile_x = 1;
                self.window_y = self.window_y.wrapping_add(1);
            } else {
                self.wx_triggered = false;
            }

            // Transition to HBlank
            // Memory unblocking and STAT update happens in tick_hblank StatUpdate state
            self.phase = PpuPhase::HBlank(HBlankStage::StatUpdate { remaining: 2 });
            // Note: STAT mode bits are NOT updated here - that happens in tick_hblank
            // This matches SameBoy where Mode 0 is set in State 22
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
                    FetcherState::GetDataHighT2 | FetcherState::Push => true,
                    _ => false,
                };
                let fifo_not_empty = self.bg_fifo.size() > 0;

                if fetcher_aligned && fifo_not_empty {
                    // Exit wait loop, enter State 41 (1 cycle for advance)
                    // But we advance BG fetcher THIS tick as well?
                    // SameBoy loop body executes advance_fetcher_state_machine().
                    // When breaking, it does NOT execute advance?
                    // "if (...) goto state_41;"
                    self.sprite_fetcher_state = SpriteFetcherState::State41Advance;
                    self.sprite_fetcher_step = 0;
                    // Note: We don't call advance_fetcher here because we'll be in State 41
                    // which advances it?
                    // Actually, if we break, we stop iterating State 27 loop.
                    // State 41 will run next.
                    // BUT SameBoy runs multiple states in one loop if cycles allow.
                    // We run one state per tick.
                    // So we transition to State 41, which will run NEXT tick.
                    // We should probably run advance_fetcher() for THIS tick if we were waiting?
                    // If we were waiting, we advanced.
                    // If we break immediately, did we wait?
                    // Let's assume consistent with previous logic:
                    // If we match the condition, we transition.
                    // If not, we advance and stay.
                    self.advance_fetcher(cgb_mode, true);
                } else {
                    // Matches SameBoy State 27 (Loop body advance)
                    self.advance_fetcher(cgb_mode, true);
                    // Stay in WaitForBgFetcher
                }
            }

            SpriteFetcherState::State41Advance => {
                // SameBoy State 41: 1 cycle (2 ticks), then do "free" advance and transition
                self.advance_fetcher(cgb_mode, true);
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 2 {
                    self.sprite_fetcher_state = SpriteFetcherState::GetTileAndFlags;
                    self.sprite_fetcher_step = 0;
                }
            }

            SpriteFetcherState::GetTileAndFlags => {
                // SameBoy State 20: OAM read (2 cycles = 4 ticks)
                // Advance BG fetcher 1 step (2 ticks) then wait
                if self.sprite_fetcher_step < 2 {
                    self.advance_fetcher(cgb_mode, true);
                }
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 4 {
                    self.sprite_fetcher_state = SpriteFetcherState::GetDataLow;
                    self.sprite_fetcher_step = 0;
                }
            }

            SpriteFetcherState::GetDataLow => {
                // SameBoy State 39: VRAM low read (2 cycles = 4 ticks)
                if self.sprite_fetcher_step == 0 {
                    self.sprite_tile_data_low = self
                        .vram
                        .vram_at_bank(self.sprite_tile_address, self.sprite_vram_bank);
                }
                self.sprite_fetcher_step += 1;
                if self.sprite_fetcher_step >= 4 {
                    self.sprite_fetcher_state = SpriteFetcherState::GetDataHighAndPush;
                    self.sprite_fetcher_step = 0;
                }
            }

            SpriteFetcherState::GetDataHighAndPush => {
                // SameBoy State 40: VRAM high read (1 cycle = 2 ticks), then overlay
                self.sprite_fetcher_step += 1;

                if self.sprite_fetcher_step >= 2 {
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

                    // Done fetching this sprite
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
            self.window_y = self.window_y.wrapping_add(1);
        }
    }

    /// Activate window rendering.
    fn activate_window(&mut self, is_cgb: bool) {
        self.window_y = self.window_y.wrapping_add(1);
        self.window_tile_x = 0;

        // Clear BG FIFO and restart fetcher for window
        self.bg_fifo.clear();

        // SameBoy lines 1917-1919: Only WX=0 with (SCX & 7) != 0 on DMG adds 1 T-cycle (2 ticks) delay
        // All other cases have no delay
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
    /// Uses T1/T2 states matching SameBoy:
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
                // T1: Calculate tile map address
                // SameBoy line 923-924: Clear wx_triggered if window is disabled
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
                    // SameBoy BG X calculation
                    let scx = self.scx;
                    // (pos + 16 < 8) is equivalent to (pos < -8)
                    if self.position_in_line < -8 {
                        scx / 8
                    } else {
                        let offset = u8::from(
                            matches!(cgb_mode, CgbMode::Cgb | CgbMode::Compat)
                                && !during_sprite_fetch,
                        );
                        let pos = self
                            .position_in_line
                            .wrapping_add(8)
                            .wrapping_sub(i16::from(offset));
                        let scx_adj = scx.wrapping_add(pos as u8);
                        scx_adj / 8
                    }
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
                // T1: Recalculate tile data address for high byte
                // SameBoy re-reads LCDC here to handle mid-fetch changes
                self.fetcher_tile_data_addr = self.calculate_tile_data_addr() + 1;
                self.fetcher_state = FetcherState::GetDataHighT2;
            }
            FetcherState::GetDataHighT2 => {
                // T2: Read high byte from VRAM
                self.current_tile_data[1] = self.read_tile_byte(self.fetcher_tile_data_addr);

                // SameBoy lines 1075-1078: Increment window_tile_x AFTER reading high byte
                if self.wx_triggered {
                    self.window_tile_x = self.window_tile_x.wrapping_add(1) & 0x1F;
                }

                // SameBoy: Fallthrough to PUSH logic immediately
                // If FIFO is empty, push and go to GetTileT1 (0 cycles for push)
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
        // Note: window_tile_x increment moved to GetDataHighT2 to match SameBoy
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
        if self.vram_ppu_blocked {
            return 0xFF;
        }
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
        let bg_pixel = self.bg_fifo.pop().unwrap();
        let sprite_pixel = self.oam_fifo.pop();

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

        // SameBoy lines 709-711: Drop pixels for scrolling (position >= 160 in uint8 terms)
        // In signed terms: position < 0 (discard phase) OR position >= 160 (line complete)
        if self.position_in_line < 0 {
            // Discard phase - just increment position, pixel already popped
            self.position_in_line += 1;
            return;
        }

        // SameBoy: Drop pixels if we've reached the end of the visible line
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
    /// SameBoy timing:
    /// - State 22 (2 ticks): STAT = Mode 0, memory unblocked, fire STAT interrupt
    /// - State 33 (4 ticks): CGB palettes blocked (non-double-speed only)
    /// - State 36 (4 ticks): CGB palettes unblocked
    /// - State 11 (variable): Wait until line ends (912 ticks total)
    /// - State 31 (4 ticks): Set mode_for_interrupt = 2 for next line
    fn tick_hblank(&mut self, ints: &mut Interrupts, double_speed: bool) {
        let PpuPhase::HBlank(stage) = self.phase else {
            return;
        };

        // Line length is 456 T-cycles = 912 ticks
        const LINE_LENGTH_TICKS: u16 = 912;
        // PreEnd starts 4 ticks before line end
        const PRE_END_START: u16 = LINE_LENGTH_TICKS - 4;

        match stage {
            HBlankStage::StatUpdate { remaining } => {
                // State 22: STAT = Mode 0, memory unblocked
                // SameBoy lines 2090-2108: Two-phase mode change

                // Phase 1 (remaining == 2): Non-double-speed early unblock
                // SameBoy lines 2090-2097
                if remaining == 2 && !double_speed {
                    self.set_mode_stat(Mode::HBlank);
                    self.mode_for_interrupt = Some(Mode::HBlank);
                    // Note: oam_read_blocked stays true on CGB-D+, but we don't track model
                    self.oam_read_blocked = false;
                    self.vram_read_blocked = false;
                    self.oam_write_blocked = false;
                    self.vram_write_blocked = false;
                    // No STAT update yet!
                }

                // Phase 2 (remaining == 1, right before transition): Full unblock + STAT update
                // SameBoy lines 2102-2108
                if remaining == 1 {
                    self.set_mode_stat(Mode::HBlank);
                    self.mode_for_interrupt = Some(Mode::HBlank);
                    self.oam_read_blocked = false;
                    self.vram_read_blocked = false;
                    self.oam_write_blocked = false;
                    self.vram_write_blocked = false;
                    self.update_stat(ints);
                }

                if remaining <= 1 {
                    // Transition to PalettesBlock
                    self.phase = PpuPhase::HBlank(HBlankStage::PalettesBlock { remaining: 4 });
                } else {
                    self.phase = PpuPhase::HBlank(HBlankStage::StatUpdate {
                        remaining: remaining - 1,
                    });
                }
            }

            HBlankStage::PalettesBlock { remaining } => {
                // State 33: CGB palettes blocked (non-double-speed only)
                // SameBoy lines 2111-2113
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
                // State 36: CGB palettes unblocked
                // SameBoy lines 2119-2121
                if remaining == 4 {
                    self.cgb_palettes_blocked = false;
                    // TODO: HDMA trigger check (line 2115-2117)
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
                // State 11: Wait for line to near-complete
                // SameBoy lines 2129-2133
                // We wait until 4 ticks before line end, then transition to PreEnd
                if self.dots_in_line >= PRE_END_START {
                    self.phase = PpuPhase::HBlank(HBlankStage::PreEnd { remaining: 4 });
                }
                // Otherwise stay in Remainder
            }

            HBlankStage::PreEnd { remaining } => {
                // State 31: Pre-end, set mode_for_interrupt = 2 for next line
                // SameBoy lines 2135-2139
                // Use current_line, not ly, to match SameBoy line 2137
                if remaining == 4 && self.current_line != 143 {
                    // Prepare Mode 2 interrupt for next line (LineStart)
                    self.mode_for_interrupt = Some(Mode::OamScan);
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

                        self.oam_read_blocked = true;
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
                // State 26: 2 cycles (4 ticks)
                if *remaining == 4 {
                    // Start of VBlank line logic
                    // SameBoy: ly_for_comparison = -1;
                    self.ly_for_comparison = 0xFFFF;
                    self.update_stat(ints);
                }

                if *remaining <= 1 {
                    self.phase = PpuPhase::VBlank(VBlankStage::LyUpdate { remaining: 4 });
                    return;
                } else {
                    *remaining -= 1;
                }
            }

            VBlankStage::LyUpdate { ref mut remaining } => {
                // State 12: 2 cycles (4 ticks)
                if *remaining == 4 {
                    self.ly = self.current_line;
                }

                if *remaining <= 1 {
                    self.phase = PpuPhase::VBlank(VBlankStage::LycUpdate { remaining: 2 });
                    return;
                } else {
                    *remaining -= 1;
                }
            }

            VBlankStage::LycUpdate { ref mut remaining } => {
                // State 24: 1 cycle (2 ticks)
                if *remaining <= 1 {
                    self.ly_for_comparison = u16::from(self.ly);
                    self.update_stat(ints);

                    // SameBoy Step 10: VBlank Entry Logic (Line 144 only)
                    if self.current_line == PX_HEIGHT {
                        // Enter Mode 1 (VBlank) officially
                        self.set_mode_stat(Mode::VBlank);
                        ints.request_vblank();
                        self.wy_triggered = false; // Reset WY trigger (usually done in VBlank INT in SameBoy?)

                        // Quirk #2: Check for OAM interrupt again?
                        // SameBoy line 2197: if (!gb->stat_interrupt_line && (gb->io_registers[GB_IO_STAT] & 0x20))
                        if !self.stat_interrupt_line && (self.stat & STAT_IF_OAM_B != 0) {
                            ints.request_lcd();
                        }

                        self.mode_for_interrupt = Some(Mode::VBlank);
                        self.update_stat(ints);
                    }

                    // Total used: 4+4+2 = 10 ticks.
                    // Remainder = 912 - 10 = 902.
                    self.phase = PpuPhase::VBlank(VBlankStage::Remainder { remaining: 902 });
                    return;
                } else {
                    *remaining -= 1;
                }
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
                } else {
                    *remaining -= 1;
                }
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
        // SameBoy: window_y starts at -1 (0xFF), incremented to 0 when window first activates
        // The reset at line 143 handles this for subsequent frames
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
        // LYC change may affect coincidence - update STAT line if LCD is on
        if self.lcdc & LCDC_ON_B != 0 {
            self.update_stat(ints);
        }
    }

    pub fn enter_stop_mode(&mut self) {
        self.vram_ppu_blocked = !self.vram_read_blocked;
    }

    pub fn leave_stop_mode(&mut self) {
        self.vram_ppu_blocked = false;
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
