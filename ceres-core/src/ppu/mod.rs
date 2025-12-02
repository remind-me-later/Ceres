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
    self::color_palette::ColorPalette, crate::CgbMode, fetcher::FetcherState, fifo::PixelFifo,
    rgba_buf::RgbaBuf, sprite::SpriteBuffer,
};

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;

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

const DOTS_UNTIL_ENABLED: i32 = 80;

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
    pub fn dots(self, _scroll_x: u8) -> i32 {
        const OAM_SCAN_DOTS: i32 = 84;
        // Mode 3 base is 172 minimum, but with proper fetcher/FIFO timing
        // it's closer to 167 + (SCX & 7) for no sprites/window
        const DRAWING_DOTS: i32 = 172;
        const HBLANK_DOTS: i32 = 200;
        const VBLANK_DOTS: i32 = 456;
        // SCX handling is done via pixel discarding, not directly in Mode::dots duration.
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
    enable_timer: i32,
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
    /// OAM is blocked (during Mode 2 and Mode 3).
    oam_blocked: bool,
    /// VRAM is blocked (during Mode 3).
    vram_blocked: bool,
    /// Fetcher sub-cycle counter (0-1, each fetcher step takes 2 T-cycles).
    fetcher_step: u8,
    /// OAM scan index (0-39, which OAM entry is being checked).
    oam_scan_index: u8,
    /// Mode 3 delay (pipeline startup and sprite fetch penalties).
    mode3_delay: u8,
    /// Last X coordinate where sprites were fetched (to prevent re-triggering).
    last_fetched_x: i16,
    /// Pending sprite fetch X coordinate (waiting for fetcher alignment).
    pending_sprite_fetch_x: Option<u8>,
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
        self.stat &= !STAT_LYC_B;
        if self.ly_for_comparison == self.lyc {
            self.stat |= STAT_LYC_B;
        }

        // Compute new STAT interrupt line state from all enabled sources
        let mut new_line = false;

        // LY=LYC coincidence interrupt
        if (self.stat & STAT_IF_LYC_B != 0) && (self.stat & STAT_LYC_B != 0) {
            new_line = true;
        }

        // Mode-based interrupts use mode_for_interrupt (which can differ from STAT bits)
        // SameBoy: mode_for_interrupt can be set to 2 at the end of HBlank, before STAT changes
        // If mode_for_interrupt is None, fall back to the actual STAT mode bits
        let interrupt_mode = self.mode_for_interrupt.unwrap_or_else(|| self.mode());
        match interrupt_mode {
            Mode::HBlank if self.stat & STAT_IF_HBLANK_B != 0 => new_line = true,
            Mode::VBlank if self.stat & STAT_IF_VBLANK_B != 0 => new_line = true,
            Mode::OamScan if self.stat & STAT_IF_OAM_B != 0 => new_line = true,
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
            // Mode 3 startup delay (approx 6-8 T-cycles)
            // Required for accurate timing in Mooneye intr_2_mode0_timing
            self.mode3_delay = 0;
            self.last_fetched_x = -1;
        }

        // VBlank always fires its own interrupt (separate from STAT)
        if matches!(mode, Mode::VBlank) {
            ints.request_vblank();
            self.win_skipped = 0;
            self.win_in_frame = false;

            // DMG/MGB/SGB quirk: When entering VBlank at line 144, the OAM interrupt
            // (STAT bit 5) also triggers a STAT interrupt if enabled.
            // This is because the Mode 2 condition is briefly asserted at VBlank entry.
            // See: vblank_stat_intr-GS test
            if self.ly == PX_HEIGHT && !self.stat_interrupt_line && (self.stat & STAT_IF_OAM_B != 0)
            {
                ints.request_lcd();
            }
        }

        if matches!(mode, Mode::OamScan) {
            self.win_in_ly = false;
        }

        // Update STAT interrupt line (fires on rising edge)
        self.update_stat(ints);
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

    pub fn run(&mut self, dots: i32, ints: &mut Interrupts, cgb_mode: CgbMode) {
        for _ in 0..dots {
            self.tick(ints, cgb_mode);
        }
    }

    pub const fn set_color_correction_mode(&mut self, mode: ColorCorrectionMode) {
        self.color_correction_mode = mode;
    }

    const fn set_mode_stat(&mut self, mode: Mode) {
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
    }

    /// Returns true if OAM is accessible by the CPU.
    ///
    /// OAM is blocked during Mode 2 (OAM scan) and Mode 3 (drawing).
    #[inline]
    #[must_use]
    pub const fn is_oam_accessible(&self) -> bool {
        // OAM is accessible when LCD is off or during Mode 0/1
        self.lcdc & LCDC_ON_B == 0 || !self.oam_blocked
    }

    /// Returns true if VRAM is accessible by the CPU.
    ///
    /// VRAM is blocked during Mode 3 (drawing).
    #[inline]
    #[must_use]
    pub const fn is_vram_accessible(&self) -> bool {
        // VRAM is accessible when LCD is off or not in Mode 3
        self.lcdc & LCDC_ON_B == 0 || !self.vram_blocked
    }

    /// Advance PPU by one T-cycle (dot).
    #[inline]
    pub fn tick(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode) {
        if self.lcdc & LCDC_ON_B == 0 {
            return;
        }

        // Handle LCD enable delay
        if self.enable_timer > 0 {
            self.enable_timer -= 1;
            if self.enable_timer == 0 {
                self.start_mode3(ints);
            }
            return;
        }

        // Advance the current mode
        self.dots_in_line += 1;
        self.remaining_dots_in_mode -= 1;

        match self.mode() {
            Mode::OamScan => self.tick_oam_scan(ints),
            Mode::Drawing => self.tick_drawing(ints, cgb_mode),
            Mode::HBlank => self.tick_hblank(ints),
            Mode::VBlank => self.tick_vblank(ints),
        }
    }

    /// Start Mode 3 (drawing) - called after LCD enable delay.
    fn start_mode3(&mut self, ints: &mut Interrupts) {
        self.mode = Mode::Drawing;
        self.set_mode_stat(Mode::Drawing);
        self.remaining_dots_in_mode = Mode::Drawing.dots(self.scx);
        self.oam_blocked = true;
        self.vram_blocked = true;
        // Mode 3 startup delay
        self.mode3_delay = 0;
        self.last_fetched_x = -1;
        self.pending_sprite_fetch_x = None;
        // Initialize drawing state
        // SameBoy quirk: First line after LCD on behaves as if Mode 2 (80 dots) + 8 dots have passed.
        // Total 88 dots.
        self.dots_in_line = 88;
        self.fetcher_state = FetcherState::GetTile;
        self.fetcher_step = 0;
        self.fetcher_tile_x = 0;
        // SameBoy: position_in_line starts at -16. The SCX alignment algorithm
        // in output_pixel will handle jumping to -8 when (position_in_line & 7) == (SCX & 7).
        self.position_in_line = -16;
        self.lcd_x = 0;
        self.bg_fifo.clear();
        self.oam_fifo.clear();
        // Push 8 "junk" pixels to prime the FIFO (will be discarded during scroll)
        self.bg_fifo.push_bg_row(0, 0, 0, false, false);
        self.window_triggered = false;
        self.window_line = 0;
        // Note: No OAM scan happened, so sprite_buffer stays empty for first line after LCD on

        self.update_stat(ints);
    }

    /// Tick during Mode 2 (OAM Scan).
    fn tick_oam_scan(&mut self, ints: &mut Interrupts) {
        // SameBoy startup delay logic
        // dots_in_line:
        // 1, 2: Sleep (Mode 0)
        // 3: Sleep (Mode 0), LY update at end
        // 4: Sleep (Mode 0), STAT update at end
        // 5+: OAM Scan
        
        if self.dots_in_line == 3 {
             self.ly = self.ly.wrapping_add(1);
             self.ly_for_comparison = self.ly;
             self.update_stat(ints);
        }
        
        if self.dots_in_line == 4 {
            self.set_mode_stat(Mode::OamScan);
            self.update_stat(ints);
        }
        
        // Only scan if we are past the startup phase
        if self.dots_in_line > 4 {
            let effective_dots = self.dots_in_line - 4;
            
            // OAM scan takes exactly 80 dots
            // Every 2 dots, check one OAM entry (40 entries total)
            if effective_dots.is_multiple_of(2) && self.oam_scan_index < 40 {
                self.scan_oam_entry();
                self.oam_scan_index += 1;
            }
        }

        if self.remaining_dots_in_mode <= 0 {
            self.enter_mode(Mode::Drawing, ints);
            self.oam_blocked = true;
            self.vram_blocked = true;
            self.fetcher_state = FetcherState::GetTile;
            self.fetcher_step = 0;
            self.fetcher_tile_x = 0;
            // SameBoy: position_in_line starts at -16. The SCX alignment algorithm
            // in output_pixel will handle jumping to -8 when (position_in_line & 7) == (SCX & 7).
            self.position_in_line = -16;
            self.lcd_x = 0;
            self.bg_fifo.clear();
            self.oam_fifo.clear();
            // Push 8 "junk" pixels to prime the FIFO (will be discarded during scroll)
            self.bg_fifo.push_bg_row(0, 0, 0, false, false);
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

    /// Fetch sprites at the current position and overlay onto OAM FIFO.
    fn fetch_sprites_at_position(&mut self, cgb_mode: CgbMode, match_x: u8) {
        if self.lcdc & LCDC_OBJ_B == 0 && !matches!(cgb_mode, CgbMode::Cgb) {
            return;
        }

        if i16::from(match_x) == self.last_fetched_x {
            return;
        }
        self.last_fetched_x = i16::from(match_x);

        let mut sprite_count = 0;

        for sprite in self.sprite_buffer.sprites_at_x(match_x) {
            sprite_count += 1;

            // Calculate tile address using get_object_line_address logic
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

            // Determine VRAM bank (CGB only)
            let vram_bank = if matches!(cgb_mode, CgbMode::Cgb) {
                sprite.cgb_vram_bank()
            } else {
                0
            };

            // Read tile data from VRAM
            let data_low = self.vram.vram_at_bank(0x8000 + line_address, vram_bank);
            let data_high = self.vram.vram_at_bank(0x8000 + line_address + 1, vram_bank);

            // Determine palette
            let palette = match cgb_mode {
                CgbMode::Cgb => sprite.cgb_palette(),
                _ => sprite.dmg_palette(),
            };

            // Overlay onto OAM FIFO
            // Priority for FIFO overlay:
            // - DMG: Lower X = higher priority, so use X coordinate (higher X = higher priority number = lower priority)
            // - CGB: Lower OAM index = higher priority
            let priority = match cgb_mode {
                CgbMode::Cgb => sprite.oam_index,
                _ => sprite.x, // For DMG, X coordinate determines priority
            };
            self.oam_fifo.overlay_sprite_row(
                data_low,
                data_high,
                palette,
                sprite.bg_priority(),
                priority,
                sprite.x_flip(),
            );
        }

        if sprite_count > 0 {
            // Calculate sprite fetch penalty (6 cycles per sprite + fetcher alignment)
            // Fetcher runs in 8-cycle steps (4 states * 2 cycles)
            // Target alignment: Cycle 7
            self.mode3_delay += 6 * sprite_count as u8;
        }
    }

    /// Tick during Mode 3 (Drawing).
    fn tick_drawing(&mut self, ints: &mut Interrupts, cgb_mode: CgbMode) {
        // SameBoy: Exit check happens BEFORE the cycle's work, not after.
        // This means the cycle where position_in_line reaches 160 exits immediately.
        if self.position_in_line >= 160 {
            // Increment window line counter if window was active this scanline
            if self.window_triggered {
                self.window_line = self.window_line.wrapping_add(1);
            }
            // Transition to HBlank
            // Calculate actual HBlank duration: 456 total - actual dots used
            let hblank_dots = 456 - i32::from(self.dots_in_line);
            self.oam_blocked = false;
            self.vram_blocked = false;
            self.mode = Mode::HBlank;
            self.set_mode_stat(Mode::HBlank);
            self.remaining_dots_in_mode = hblank_dots;
            self.update_stat(ints);
            return;
        }

        if self.mode3_delay > 0 {
            self.mode3_delay -= 1;
            return;
        }

        // Check for window activation
        self.check_window_trigger(cgb_mode);

        // Check for sprites at current position
        // SameBoy's x_for_object_match: position_in_line + 8, clamped to 0 if overflow
        let match_x = {
            let raw = self.position_in_line.wrapping_add(8) as u8;
            // If raw > 240 (i.e., position_in_line was < -8), use 0
            if raw > 240 { 0 } else { raw }
        };

        // Handle sprite fetch pipeline
        if self.pending_sprite_fetch_x.is_none() {
            // Check if we should start a fetch
            // Only fetch if enabled and not already fetched
            // Note: fetch_sprites_at_position checks enable bits, but we need to check here to avoid setting pending
            let sprites_enabled = self.lcdc & LCDC_OBJ_B != 0 || matches!(cgb_mode, CgbMode::Cgb);

            if sprites_enabled && i16::from(match_x) != self.last_fetched_x {
                if self.sprite_buffer.sprites_at_x(match_x).count() > 0 {
                    self.pending_sprite_fetch_x = Some(match_x);
                } else {
                    self.last_fetched_x = i16::from(match_x);
                }
            }
        }

        if let Some(x) = self.pending_sprite_fetch_x {
            // Alignment check: Wait for fetcher to finish reading (Wait if state < DataHighT2 OR FIFO empty)
            // ceres-core fetcher states: GetTile -> GetDataLow -> GetDataHigh -> Push
            // GetDataHigh + step 1 corresponds to End of DataHigh (SameBoy T2), ready to Push.

            let aligned = match self.fetcher_state {
                FetcherState::Push => true,
                FetcherState::GetDataHigh if self.fetcher_step == 1 => true,
                _ => false,
            };

            // SameBoy also checks FIFO size > 0 (unless empty, then wait?)
            // "while (state < ... || fifo_size == 0)" -> Wait if empty.
            let fifo_not_empty = self.bg_fifo.size() > 0;

            if aligned && fifo_not_empty {
                self.fetch_sprites_at_position(cgb_mode, x);
                self.pending_sprite_fetch_x = None;
                return;
            }

            // Not aligned, advance fetcher and return (don't output pixel)
            if self.fetcher_step == 0 {
                self.advance_fetcher(cgb_mode);
            }
            self.fetcher_step = (self.fetcher_step + 1) % 2;
            return;
        }

        // SameBoy order: render_pixel_if_possible THEN advance_fetcher_state_machine
        // Try to output a pixel first (output_pixel handles empty FIFO checks internally)
        self.output_pixel(cgb_mode);

        // Advance fetcher every 2 T-cycles (each fetcher state takes 2 T-cycles)
        if self.fetcher_step == 0 {
            self.advance_fetcher(cgb_mode);
        }
        self.fetcher_step = (self.fetcher_step + 1) % 2;
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
                self.lcdc & (LCDC_BG_B | LCDC_WIN_B) == (LCDC_BG_B | LCDC_WIN_B)
            }
            CgbMode::Cgb => self.lcdc & LCDC_WIN_B != 0,
        };

        if !window_enabled {
            return;
        }

        // Check WY condition (window Y trigger)
        if self.ly < self.wy {
            return;
        }

        // Check WX condition (window X trigger)
        // Window activates when position_in_line == WX - 7
        let wx_trigger = i16::from(self.wx) - 7;
        if self.position_in_line == wx_trigger {
            self.activate_window();
        }
    }

    /// Activate window rendering.
    fn activate_window(&mut self) {
        self.window_triggered = true;
        self.win_in_frame = true;

        // Clear BG FIFO and restart fetcher for window
        self.bg_fifo.clear();
        self.fetcher_state = FetcherState::GetTile;
        self.fetcher_tile_x = 0;

        // Window activation incurs a 6-dot penalty (handled by fetcher restart)
    }

    /// Advance the background/window fetcher state machine.
    fn advance_fetcher(&mut self, cgb_mode: CgbMode) {
        match self.fetcher_state {
            FetcherState::GetTile => {
                // Calculate tile map address
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

                let addr = tile_map + u16::from(y / 8) * 32 + u16::from(x);

                self.current_tile = self.vram.vram_at_bank(addr, 0);
                self.current_tile_attrs = match cgb_mode {
                    CgbMode::Cgb => self.vram.vram_at_bank(addr, 1),
                    _ => 0,
                };

                self.fetcher_state = FetcherState::GetDataLow;
            }
            FetcherState::GetDataLow => {
                let tile_addr = self.calculate_tile_data_addr();
                self.current_tile_data[0] = self.read_tile_byte(tile_addr);
                self.fetcher_state = FetcherState::GetDataHigh;
            }
            FetcherState::GetDataHigh => {
                let tile_addr = self.calculate_tile_data_addr();
                self.current_tile_data[1] = self.read_tile_byte(tile_addr + 1);
                self.fetcher_state = FetcherState::Push;
            }
            FetcherState::Push => {
                // Push if FIFO has space (capacity 16, push 8, so need <= 8)
                if self.bg_fifo.size() <= 8 {
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

                    self.fetcher_tile_x = self.fetcher_tile_x.wrapping_add(1) & 0x1F;
                    self.fetcher_state = FetcherState::GetTile;
                }
                // If FIFO full (size > 8), stay in Push state
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
        let bank = (self.current_tile_attrs >> 3) & 1;
        self.vram.vram_at_bank(addr, bank)
    }

    /// Output a pixel to the LCD buffer.
    /// Implements SameBoy's render_pixel_if_possible logic.
    fn output_pixel(&mut self, cgb_mode: CgbMode) {
        // SameBoy: Handle position_in_line alignment for SCX.
        // (position_in_line + 16 < 8) is equivalent to (position_in_line < -8) in unsigned logic.
        // When position_in_line is in the range [-16, -9], we're in the "fractional scrolling" phase.
        #[expect(clippy::cast_sign_loss)]
        let unsigned_pos_plus_16 = (self.position_in_line + 16) as u8;
        if unsigned_pos_plus_16 < 8 {
            // SameBoy: Check if we should jump to -8 based on SCX alignment.
            // When (position_in_line & 7) == (SCX & 7), jump to -8.
            #[expect(clippy::cast_sign_loss)]
            let pos_mod_8 = (self.position_in_line & 7) as u8;
            let scx_mod_8 = self.scx & 7;

            if pos_mod_8 == scx_mod_8 {
                self.position_in_line = -8;
            }
            // SameBoy: If alignment doesn't match, the function continues
            // and will pop a pixel from FIFO + increment position_in_line below.
        }

        // SameBoy: Discard phase - position_in_line < 0 means we're discarding junk pixels
        if self.position_in_line < 0 {
            // SameBoy: if (fifo_size(&gb->bg_fifo) == 0) return;
            if self.bg_fifo.is_empty() {
                return;
            }
            let _ = self.bg_fifo.pop();
            let _ = self.oam_fifo.pop();
            self.position_in_line += 1;
            return;
        }

        // SameBoy: Drop pixels if we've reached the end of the visible line
        if self.lcd_x >= PX_WIDTH {
            self.position_in_line += 1;
            return;
        }

        let Some(bg_pixel) = self.bg_fifo.pop() else {
            return;
        };

        let sprite_pixel = self.oam_fifo.pop();

        let (color, palette, is_sprite) = self.mix_pixels(bg_pixel, sprite_pixel, cgb_mode);

        let rgb = if is_sprite {
            self.sprite_color_to_rgb(color, palette, cgb_mode)
        } else {
            self.bg_color_to_rgb(color, palette, cgb_mode)
        };

        let idx = u32::from(self.ly) * u32::from(PX_WIDTH) + u32::from(self.lcd_x);
        self.rgb_buf.set_px(idx, rgb);

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
            CgbMode::Dmg | CgbMode::Compat => {
                // DMG: BG/OBJ priority from sprite attribute
                sprite_behind_bg && bg_opaque
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
                self.ocp.rgb(0, shade, self.color_correction_mode)
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

    /// Monochrome palette lookup.
    #[must_use]
    const fn mono_rgb(shade: u8) -> (u8, u8, u8) {
        color_palette::GRAYSCALE_PALETTE[shade as usize]
    }

    /// Tick during Mode 0 (HBlank).
    fn tick_hblank(&mut self, ints: &mut Interrupts) {
        // SameBoy quirk: Mode 2 interrupt fires near the end of HBlank,
        // about 1 cycle before the line actually changes.
        // Fire the interrupt early while STAT still shows Mode 0.
        if self.remaining_dots_in_mode == 1 && self.ly < 143 {
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
        
                    if self.ly + 1 > 143 {
                        self.ly += 1;
                        self.ly_for_comparison = self.ly;
                        self.enter_mode(Mode::VBlank, ints);
                    } else {
                        // Reset for next scanline
                        self.oam_scan_index = 0;
                        self.sprite_buffer.clear();
                        self.oam_blocked = true;
                        self.window_triggered = false;
                        
                        self.enter_mode(Mode::OamScan, ints);
                    }
                }    }

    /// Tick during Mode 1 (VBlank).
    fn tick_vblank(&mut self, ints: &mut Interrupts) {
        if self.remaining_dots_in_mode <= 0 {
            self.ly += 1;
            self.ly_for_comparison = self.ly;

            if self.ly > 153 {
                self.ly = 0xFF;
                self.ly_for_comparison = 0xFF;
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
                self.oam_blocked = true;
                self.window_line = 0;
                self.window_triggered = false;
                self.enter_mode(Mode::OamScan, ints);
            } else {
                self.remaining_dots_in_mode += Mode::VBlank.dots(self.scx);
                self.update_stat(ints);
            }
        }
    }

    pub const fn write_bgp(&mut self, val: u8) {
        self.bgp = val;
    }

    pub fn write_lcdc(&mut self, val: u8, ints: &mut Interrupts) {
        // turn off
        if val & LCDC_ON_B == 0 && self.lcdc & LCDC_ON_B != 0 {
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
            let mode = Mode::HBlank;
            self.mode = mode;
            self.set_mode_stat(mode);
            self.remaining_dots_in_mode = mode.dots(self.scx);
            self.rgba_buf_present.clear();

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
            let mode = Mode::HBlank;
            self.mode = mode;
            self.set_mode_stat(mode);
            self.remaining_dots_in_mode = mode.dots(self.scx);
            // Comparison clock restarts - update coincidence and check for interrupt
            self.update_stat(ints);
            self.enable_timer = DOTS_UNTIL_ENABLED;
            self.delay_one_frame = true;
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

    pub const fn write_wy(&mut self, val: u8) {
        self.wy = val;
    }
}
