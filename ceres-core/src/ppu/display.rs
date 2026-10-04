//! Per-dot display engine, structured after SameBoy's `GB_display_run`.
//!
//! SameBoy models the PPU as a coroutine that sleeps for a fixed number of
//! dots between observable events. This module keeps the same shape: `state`
//! is the id of the sleep the engine is currently in (the same numbers as
//! SameBoy's `display_state`), `wait` is the number of dots left in it, and
//! the match in [`Ppu::run_display`] holds the code that follows each sleep.
//! Keeping the ids and durations identical makes it possible to compare
//! traces line by line against SameBoy.

use crate::{CgbMode, Model, interrupts::Interrupts};

use super::{Ppu, STAT_IF_HBLANK_B, STAT_IF_LYC_B, STAT_IF_OAM_B, STAT_IF_VBLANK_B, STAT_LYC_B};

pub(super) const MODE2_LENGTH: i32 = 80;
pub(super) const LINE_LENGTH: i32 = 456;
pub(super) const LINES: u8 = 144;

/// Fetcher states (SameBoy's `fetcher_step_t`).
const F_GET_TILE_T1: u8 = 0;
const F_GET_TILE_T2: u8 = 1;
const F_DATA_LOW_T1: u8 = 2;
const F_DATA_LOW_T2: u8 = 3;
const F_DATA_HIGH_T1: u8 = 4;
const F_DATA_HIGH_T2: u8 = 5;
const F_PUSH: u8 = 6;

#[derive(Clone, Copy, Default)]
struct Item {
    pixel: u8,
    palette: u8,
    priority: u8,
    bg_priority: bool,
}

/// An 8-entry ring, like SameBoy's `GB_fifo_t`.
#[derive(Clone, Copy, Default)]
struct Fifo {
    items: [Item; 8],
    read_end: u8,
    size: u8,
}

impl Fifo {
    const fn clear(&mut self) {
        self.read_end = 0;
        self.size = 0;
    }

    fn pop(&mut self) -> Item {
        let item = self.items[usize::from(self.read_end)];
        self.read_end = (self.read_end + 1) & 7;
        self.size -= 1;
        item
    }

    fn push_bg_row(&mut self, mut lower: u8, mut upper: u8, palette: u8, bg_priority: bool, flip_x: bool) {
        self.size = 8;
        for i in 0..8 {
            let pixel = if flip_x {
                let p = (lower & 1) | ((upper & 1) << 1);
                lower >>= 1;
                upper >>= 1;
                p
            } else {
                let p = (lower >> 7) | ((upper >> 7) << 1);
                lower <<= 1;
                upper <<= 1;
                p
            };
            self.items[i] = Item { pixel, palette, priority: 0, bg_priority };
        }
    }

    fn overlay_object_row(
        &mut self,
        mut lower: u8,
        mut upper: u8,
        palette: u8,
        bg_priority: bool,
        priority: u8,
        flip_x: bool,
    ) {
        while self.size < 8 {
            self.items[usize::from((self.read_end + self.size) & 7)] = Item::default();
            self.size += 1;
        }
        let flip_xor: u8 = if flip_x { 0 } else { 7 };
        for i in (0..8u8).rev() {
            let pixel = (lower >> 7) | ((upper >> 7) << 1);
            let target = &mut self.items[usize::from((self.read_end + (i ^ flip_xor)) & 7)];
            if pixel != 0 && (target.pixel == 0 || target.priority > priority) {
                *target = Item { pixel, palette, priority, bg_priority };
            }
            lower <<= 1;
            upper <<= 1;
        }
    }
}

/// One pixel as it reaches the LCD, before palette lookup.
#[derive(Clone, Copy)]
pub(super) struct PixelOut {
    pub lx: u8,
    /// Background colour id (already 0 when the background is disabled).
    pub bg_pixel: u8,
    pub bg_palette: u8,
    /// Object pixel, if one is drawn over the background.
    pub obj: Option<(u8, u8)>,
}

#[derive(Clone)]
pub(super) struct Display {
    /// SameBoy's `display_state`: id of the current sleep (0 = not started).
    pub state: u8,
    /// Dots left before the code following the current sleep runs.
    wait: i32,

    pub current_line: u8,
    /// `cycles_for_line`: SameBoy's line-length accounting.
    cfl: i32,
    /// `position_in_line`: wraps like SameBoy's `uint8_t` (240..=255 is -16..=-1).
    pub position_in_line: u8,
    lcd_x: u8,
    line_has_fractional_scrolling: bool,

    n_visible_objs: usize,
    orig_n_visible_objs: usize,
    visible_objs: [u8; 10],
    objects_x: [u8; 10],
    objects_y: [u8; 10],
    oam_search_index: u8,
    mode2_y_bus: u8,
    mode2_x_bus: u8,
    object_flags: u8,
    object_low_line_address: u16,
    object_tile_data: [u8; 2],
    pub during_object_fetch: bool,
    object_fetch_aborted: bool,

    bg_fifo: Fifo,
    oam_fifo: Fifo,
    insert_bg_pixel: bool,

    fetcher_state: u8,
    current_tile: u8,
    current_tile_attributes: u8,
    current_tile_data: [u8; 2],
    last_tile_index_address: u16,
    last_tile_data_address: u16,
    last_tileset: bool,
    fetcher_y: u8,
    data_for_sel_glitch: u8,
    pub tile_sel_glitch: bool,

    window_y: u8,
    window_tile_x: u8,
    wx_triggered: bool,
    wy_triggered: bool,
    window_is_being_fetched: bool,
    wy_check_scheduled: bool,
    wy_check_countdown: i32,
    wy_just_checked: bool,
    cgb_wx_glitch: bool,
    disable_window_pixel_insertion_glitch: bool,
    wx_166_interrupt_glitch: bool,
    pub wx_just_changed: bool,

    /// `ly_for_comparison`; `-1` is SameBoy's `(uint16_t)-1`.
    ly_for_comparison: i32,
    mode_for_interrupt: i8,
    stat_interrupt_line: bool,
    lyc_interrupt_line: bool,
    delayed_glitch_hblank_interrupt: bool,
    /// Dots since the last `cycles_for_line` reset (diagnostics / injection).
    pub line_clock: i32,

    // CPU-visible access blocking, set at the same points as SameBoy.
    pub oam_read_blocked: bool,
    pub oam_write_blocked: bool,
    pub vram_read_blocked: bool,
    pub vram_write_blocked: bool,
    pub cgb_palettes_blocked: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            state: 0,
            wait: 0,
            current_line: 0,
            cfl: 0,
            position_in_line: 240,
            lcd_x: 0,
            line_has_fractional_scrolling: false,
            n_visible_objs: 0,
            orig_n_visible_objs: 0,
            visible_objs: [0; 10],
            objects_x: [0; 10],
            objects_y: [0; 10],
            oam_search_index: 0,
            mode2_y_bus: 0,
            mode2_x_bus: 0,
            object_flags: 0,
            object_low_line_address: 0,
            object_tile_data: [0; 2],
            during_object_fetch: false,
            object_fetch_aborted: false,
            bg_fifo: Fifo::default(),
            oam_fifo: Fifo::default(),
            insert_bg_pixel: false,
            fetcher_state: 0,
            current_tile: 0,
            current_tile_attributes: 0,
            current_tile_data: [0; 2],
            last_tile_index_address: 0,
            last_tile_data_address: 0,
            last_tileset: false,
            fetcher_y: 0,
            data_for_sel_glitch: 0,
            tile_sel_glitch: false,
            window_y: 0xFF,
            window_tile_x: 0,
            wx_triggered: false,
            wy_triggered: false,
            window_is_being_fetched: false,
            wy_check_scheduled: false,
            wy_check_countdown: 0,
            wy_just_checked: false,
            cgb_wx_glitch: false,
            disable_window_pixel_insertion_glitch: false,
            wx_166_interrupt_glitch: false,
            wx_just_changed: false,
            ly_for_comparison: 0,
            mode_for_interrupt: -1,
            stat_interrupt_line: false,
            lyc_interrupt_line: false,
            delayed_glitch_hblank_interrupt: false,
            line_clock: 0,
            oam_read_blocked: false,
            oam_write_blocked: false,
            vram_read_blocked: false,
            vram_write_blocked: false,
            cgb_palettes_blocked: false,
        }
    }
}

impl Display {
    pub(super) const fn restart(&mut self) {
        self.state = 0;
        self.wait = 0;
        self.cfl = 0;
    }

    /// Schedule the WY comparison that follows an LCDC/WY write.
    pub(super) fn schedule_wy_check(&mut self, cgb: bool, double_speed: bool) {
        if !self.wy_check_scheduled {
            self.wy_check_scheduled = true;
            // SameBoy aligns the check to the 8 MHz grid; in dots that is
            // 4 (CGB), 3 (DMG) or 1 (CGB double speed) dots after the write.
            self.wy_check_countdown = if double_speed {
                1
            } else if cgb {
                4
            } else {
                3
            };
        }
    }
}

/// Model helpers matching SameBoy's `gb->model` comparisons.
const fn model_ge_cgb_d(m: Model) -> bool {
    matches!(m, Model::CgbD | Model::CgbE | Model::Agb)
}

impl Ppu {
    #[inline]
    const fn hw_cgb(&self) -> bool {
        self.model.is_cgb_hardware()
    }

    #[inline]
    const fn cgb_mode_on(&self) -> bool {
        matches!(self.cgb_mode, CgbMode::Cgb)
    }

    #[inline]
    const fn double_speed(&self) -> bool {
        self.double_speed
    }

    /// True when a DMG-family model (not SGB) is running.
    #[inline]
    const fn is_dmg_family(&self) -> bool {
        matches!(self.model, Model::Dmg0 | Model::DmgB | Model::Mgb)
    }

    // ---------------------------------------------------------------------
    // STAT / LYC
    // ---------------------------------------------------------------------

    pub(super) fn stat_update(&mut self, ints: &mut Interrupts) {
        if self.lcdc & 0x80 == 0 {
            return;
        }

        let previous = self.d.stat_interrupt_line;

        // LY=LYC flag
        // `model <= CGB_C` in SameBoy's ordering: everything before CGB-D.
        let le_c = !model_ge_cgb_d(self.model) && !self.double_speed();
        if self.d.ly_for_comparison != -1 || le_c {
            if self.d.ly_for_comparison == i32::from(self.lyc) {
                self.d.lyc_interrupt_line = true;
                self.stat |= STAT_LYC_B;
            } else {
                if self.d.ly_for_comparison != -1 {
                    self.d.lyc_interrupt_line = false;
                }
                self.stat &= !STAT_LYC_B;
            }
        }

        self.d.stat_interrupt_line = match self.d.mode_for_interrupt {
            0 => self.stat & STAT_IF_HBLANK_B != 0,
            1 => self.stat & STAT_IF_VBLANK_B != 0,
            2 => self.stat & STAT_IF_OAM_B != 0,
            _ => false,
        };

        if self.stat & STAT_IF_LYC_B != 0 && self.d.lyc_interrupt_line {
            self.d.stat_interrupt_line = true;
        }

        if self.d.stat_interrupt_line && !previous {
            ints.request_lcd();
        }
    }

    fn wy_check(&mut self) {
        if self.lcdc & 0x80 == 0 {
            return;
        }
        let mut comparison = i32::from(self.d.current_line);
        if (!self.hw_cgb() || self.double_speed()) && self.d.ly_for_comparison != -1 {
            comparison = i32::from(self.d.ly_for_comparison as u8);
        }
        if self.lcdc & 0x20 != 0 && i32::from(self.wy) == comparison {
            self.d.wy_triggered = true;
        }
    }

    pub(super) fn lcd_off(&mut self) {
        self.d.cfl = 0;
        self.d.state = 0;
        self.d.wait = 0;
        self.ly = 0;
        self.stat &= !super::STAT_MODE_B;
        self.d.current_line = 0;
        self.d.ly_for_comparison = 0;
        self.d.wy_triggered = false;
        self.d.oam_read_blocked = false;
        self.d.vram_read_blocked = false;
        self.d.oam_write_blocked = false;
        self.d.vram_write_blocked = false;
        self.d.cgb_palettes_blocked = false;
    }

    // ---------------------------------------------------------------------
    // Mode 2 object search
    // ---------------------------------------------------------------------

    fn add_object_from_index(&mut self, index: u8) {
        let base = u16::from(index) * 4;
        self.d.mode2_y_bus = self.oam.read(base);
        self.d.mode2_x_bus = self.oam.read(base + 1);

        if self.d.n_visible_objs == 10 {
            return;
        }

        let height_16 = self.lcdc & 0x04 != 0;
        let y = i32::from(self.d.mode2_y_bus) - 16;
        let line = i32::from(self.d.current_line);
        if y <= line && y + if height_16 { 16 } else { 8 } > line {
            // Reverse-sorted insertion by X (stable for equal X).
            let n = self.d.n_visible_objs;
            let mut j = 0;
            while j < n {
                if self.d.objects_x[j] <= self.d.mode2_x_bus {
                    break;
                }
                j += 1;
            }
            let mut k = n;
            while k > j {
                self.d.visible_objs[k] = self.d.visible_objs[k - 1];
                self.d.objects_x[k] = self.d.objects_x[k - 1];
                self.d.objects_y[k] = self.d.objects_y[k - 1];
                k -= 1;
            }
            self.d.visible_objs[j] = index;
            self.d.objects_x[j] = self.d.mode2_x_bus;
            self.d.objects_y[j] = self.d.mode2_y_bus;
            self.d.n_visible_objs += 1;
        }
    }

    fn x_for_object_match(&self) -> u8 {
        let ret = self.d.position_in_line.wrapping_add(8);
        if ret > 240 { 0 } else { ret }
    }

    fn object_line_address(&self, y: u8, tile: u8, flags: u8) -> u16 {
        let height_16 = self.lcdc & 0x04 != 0;
        let mut tile_y = self.d.current_line.wrapping_sub(y) & if height_16 { 0xF } else { 7 };
        if flags & 0x40 != 0 {
            tile_y ^= if height_16 { 0xF } else { 7 };
        }
        let mut address =
            u16::from(if height_16 { tile & 0xFE } else { tile }) * 0x10 + u16::from(tile_y) * 2;
        if self.cgb_mode_on() && flags & 0x8 != 0 {
            address += 0x2000;
        }
        address
    }

    fn vram_read(&self, address: u16) -> u8 {
        if address >= 0x2000 {
            self.vram.vram_at_bank(address - 0x2000, 1)
        } else {
            self.vram.vram_at_bank(address, 0)
        }
    }

    // ---------------------------------------------------------------------
    // Fetcher
    // ---------------------------------------------------------------------

    fn fetcher_y_value(&self) -> u8 {
        if self.d.wx_triggered {
            self.d.window_y
        } else {
            self.d.current_line.wrapping_add(self.scy)
        }
    }

    fn update_wx_glitch(&mut self) {
        if !self.hw_cgb() {
            return;
        }
        if self.lcdc & 0x20 == 0 || !self.d.wy_triggered {
            self.d.cgb_wx_glitch = false;
            return;
        }
        let position = self.d.position_in_line;
        if self.wx == 0 {
            // (position + 16 <= 8) in u8 arithmetic
            self.d.cgb_wx_glitch = position.wrapping_add(16) <= 8
                || (position == 249 && self.d.line_has_fractional_scrolling);
            return;
        }
        self.d.cgb_wx_glitch = position
            .wrapping_add(7)
            .wrapping_add(u8::from(self.d.window_is_being_fetched))
            == self.wx;
    }

    /// `data_for_tile_sel_glitch`: returns (data, use_glitched, cgb_d_glitch).
    fn data_for_tile_sel_glitch(&mut self) -> (u8, bool, bool) {
        if self.d.last_tileset {
            if self.model != Model::CgbD {
                return (self.d.current_tile, self.d.current_tile & 0x80 == 0, false);
            }
            self.d.last_tile_data_address &= !0x1000;
            return (0, false, true);
        }
        (self.d.data_for_sel_glitch, true, false)
    }

    fn tile_address(&self) -> u16 {
        let mut address = if self.d.last_tileset {
            u16::from(self.d.current_tile) * 0x10
        } else {
            (i32::from(self.d.current_tile as i8) * 0x10 + 0x1000) as u16
        };
        if self.d.current_tile_attributes & 8 != 0 {
            address += 0x2000;
        }
        address
    }

    fn advance_fetcher(&mut self) {
        match self.d.fetcher_state {
            F_GET_TILE_T1 => {
                self.update_wx_glitch();
                let mut map: u16 = 0x1800;
                if self.lcdc & 0x20 == 0 {
                    self.d.wx_triggered = false;
                }
                if self.lcdc & 0x08 != 0 && !self.d.wx_triggered {
                    map = 0x1C00;
                } else if self.lcdc & 0x40 != 0 && self.d.wx_triggered {
                    map = 0x1C00;
                }

                let y = self.fetcher_y_value();
                let position = self.d.position_in_line;
                let x: u16 = if self.d.wx_triggered {
                    u16::from(self.d.window_tile_x)
                } else if position.wrapping_add(16) < 8 {
                    u16::from(self.scx >> 3)
                } else {
                    let sub = i32::from(self.hw_cgb() && !self.d.during_object_fetch);
                    ((((i32::from(self.scx) + i32::from(position) + 8 - sub) / 8) & 0x1F) as u16)
                        & 0x1F
                };
                if model_ge_cgb_d(self.model) {
                    // Cached on CGB-D and newer, so it cannot mix tiles.
                    self.d.fetcher_y = y;
                }
                self.d.last_tile_index_address = map + x + u16::from(y / 8) * 32;
                self.d.fetcher_state += 1;
            }
            F_GET_TILE_T2 => {
                if self.d.cgb_wx_glitch {
                    self.d.fetcher_state += 1;
                    return;
                }
                let address = self.d.last_tile_index_address;
                self.d.current_tile = self.vram.vram_at_bank(address, 0);
                if self.hw_cgb() {
                    self.d.current_tile_attributes = self.vram.vram_at_bank(address, 1);
                }
                self.d.fetcher_state += 1;
            }
            F_DATA_LOW_T1 | F_DATA_HIGH_T1 => {
                self.update_wx_glitch();
                let y = if model_ge_cgb_d(self.model) {
                    self.d.fetcher_y
                } else {
                    self.fetcher_y_value()
                };
                self.d.last_tileset = self.lcdc & 0x10 != 0;
                let tile_address = self.tile_address();
                let y_flip = if self.d.current_tile_attributes & 0x40 != 0 { 7 } else { 0 };
                let low = self.d.fetcher_state == F_DATA_LOW_T1;
                self.d.last_tile_data_address =
                    tile_address + u16::from((y & 7) ^ y_flip) * 2 + u16::from(!low);
                self.d.fetcher_state += 1;
            }
            F_DATA_LOW_T2 => {
                if self.d.cgb_wx_glitch {
                    self.d.current_tile_data[0] = self.d.current_tile_data[1];
                    self.d.fetcher_state += 1;
                    return;
                }
                let mut use_glitched = false;
                let mut cgb_d_glitch = false;
                if self.d.tile_sel_glitch {
                    let (data, used, d) = self.data_for_tile_sel_glitch();
                    self.d.current_tile_data[0] = data;
                    use_glitched = used;
                    cgb_d_glitch = d;
                }
                if !use_glitched {
                    self.d.current_tile_data[0] = self.vram_read(self.d.last_tile_data_address);
                }
                if self.d.last_tileset && self.d.tile_sel_glitch {
                    self.d.data_for_sel_glitch = self.vram_read(self.d.last_tile_data_address);
                } else if cgb_d_glitch {
                    self.d.data_for_sel_glitch =
                        self.vram_read(self.d.last_tile_data_address & !0x1000);
                }
                self.d.fetcher_state += 1;
            }
            F_DATA_HIGH_T2 => {
                if self.d.cgb_wx_glitch {
                    self.d.current_tile_data[1] = self.d.current_tile_data[0];
                    self.d.fetcher_state += 1;
                    if self.d.wx_triggered {
                        self.d.window_tile_x = (self.d.window_tile_x + 1) & 0x1F;
                    }
                    return;
                }
                let mut use_glitched = false;
                let mut cgb_d_glitch = false;
                if self.d.tile_sel_glitch {
                    let (data, used, d) = self.data_for_tile_sel_glitch();
                    self.d.current_tile_data[1] = data;
                    use_glitched = used;
                    cgb_d_glitch = d;
                    if cgb_d_glitch {
                        self.d.last_tile_data_address -= 1;
                    }
                }
                if !use_glitched {
                    let value = self.vram_read(self.d.last_tile_data_address);
                    self.d.current_tile_data[1] = value;
                    self.d.data_for_sel_glitch = value;
                }
                if self.d.last_tileset && self.d.tile_sel_glitch {
                    self.d.data_for_sel_glitch = self.vram_read(self.d.last_tile_data_address);
                } else if cgb_d_glitch {
                    self.d.data_for_sel_glitch =
                        self.vram_read((self.d.last_tile_data_address & !0x1000) + 1);
                }
                if self.d.wx_triggered {
                    self.d.window_tile_x = (self.d.window_tile_x + 1) & 0x1F;
                }
                self.fetcher_push();
            }
            _ => self.fetcher_push(),
        }
    }

    fn fetcher_push(&mut self) {
        self.d.fetcher_state = F_PUSH;
        if self.d.bg_fifo.size > 0 {
            return;
        }

        if self.d.wy_triggered
            && self.lcdc & 0x20 == 0
            && !self.hw_cgb()
            && !self.d.disable_window_pixel_insertion_glitch
        {
            // See https://github.com/LIJI32/SameBoy/issues/278
            let mut logical_position = self.d.position_in_line.wrapping_add(7);
            if logical_position > 167 {
                logical_position = 0;
            }
            if self.wx == logical_position {
                let fifo = &mut self.d.bg_fifo;
                fifo.read_end = fifo.read_end.wrapping_sub(1) & 7;
                fifo.items[usize::from(fifo.read_end)] = Item::default();
                fifo.size = 1;
                return;
            }
        }

        let attr = self.d.current_tile_attributes;
        let (low, high) = (self.d.current_tile_data[0], self.d.current_tile_data[1]);
        self.d.bg_fifo.push_bg_row(low, high, attr & 7, attr & 0x80 != 0, attr & 0x20 != 0);
        self.d.fetcher_state = F_GET_TILE_T1;
    }

    // ---------------------------------------------------------------------
    // Pixel output
    // ---------------------------------------------------------------------

    fn render_pixel_if_possible(&mut self) -> Option<PixelOut> {
        let obj_en = self.lcdc & 0x02 != 0 || self.hw_cgb();
        if self.d.n_visible_objs != 0
            && obj_en
            && self.d.objects_x[self.d.n_visible_objs - 1] == 0
        {
            return None;
        }
        if self.d.bg_fifo.size == 0 {
            return None;
        }

        let fifo_item = if self.d.insert_bg_pixel {
            self.d.insert_bg_pixel = false;
            Item::default()
        } else {
            self.d.bg_fifo.pop()
        };
        let mut bg_priority = fifo_item.bg_priority;
        let mut oam_item = Item::default();
        let mut draw_oam = false;

        if self.d.oam_fifo.size != 0 {
            oam_item = self.d.oam_fifo.pop();
            if oam_item.pixel != 0 && self.lcdc & 0x02 != 0 {
                draw_oam = true;
                bg_priority |= oam_item.bg_priority;
            }
        }

        // (position + 16 < 8) in u8 arithmetic: the lead-in range.
        let position = self.d.position_in_line;
        if position.wrapping_add(16) < 8 {
            if position == 239 {
                self.d.position_in_line = 240;
            } else if position & 7 == self.scx & 7 {
                self.d.position_in_line = 248;
            } else if self.d.window_is_being_fetched && position & 7 == 6 && self.scx & 7 == 7 {
                self.d.position_in_line = 248;
            } else if position == 247 {
                self.d.position_in_line = 240;
                return None;
            } else {
                self.d.line_has_fractional_scrolling = true;
            }
        }

        self.d.window_is_being_fetched = false;

        // Drop pixels for scrolling (negative positions compare >= 160 in u8).
        if self.d.position_in_line >= 160 {
            self.d.position_in_line = self.d.position_in_line.wrapping_add(1);
            return None;
        }

        // Mixing
        let mut bg_enabled = true;
        if self.lcdc & 0x01 == 0 {
            if self.cgb_mode_on() {
                bg_priority = false;
            } else {
                bg_enabled = false;
            }
        }
        let bg_pixel = if bg_enabled { fifo_item.pixel } else { 0 };
        if bg_pixel != 0 && bg_priority {
            draw_oam = false;
        }

        let out = PixelOut {
            lx: self.d.lcd_x,
            bg_pixel,
            bg_palette: fifo_item.palette,
            obj: draw_oam.then_some((oam_item.pixel, oam_item.palette)),
        };

        self.d.position_in_line = self.d.position_in_line.wrapping_add(1);
        self.d.lcd_x = self.d.lcd_x.wrapping_add(1);
        Some(out)
    }

    fn output_pixel(&mut self, out: PixelOut) {
        if out.lx >= 160 || self.d.current_line >= LINES {
            return;
        }
        let rgb = self.pixel_rgb(out);
        let idx = u32::from(self.d.current_line) * u32::from(super::PX_WIDTH) + u32::from(out.lx);
        self.rgb_buf.set_px(idx, rgb);
    }

    // ---------------------------------------------------------------------
    // Engine
    // ---------------------------------------------------------------------

    fn mode3_start(&mut self) {
        self.d.disable_window_pixel_insertion_glitch = false;
        self.d.bg_fifo.clear();
        self.d.oam_fifo.clear();
        // Fill the FIFO with 8 pixels of "junk", it's going to be dropped anyway.
        self.d.bg_fifo.push_bg_row(0, 0, 0, false, false);
        self.d.lcd_x = 0;
        self.d.fetcher_state = F_GET_TILE_T1;
    }

    /// Sleep for `n` dots; the code of state `id` runs afterwards.
    #[inline]
    const fn sleep(&mut self, id: u8, n: i32) {
        self.d.state = id;
        self.d.wait = n;
    }

    /// Window activation check at the top of a mode-3 iteration. Returns
    /// `true` if the engine went to sleep (state 42).
    fn mode3_window(&mut self) -> bool {
        self.d.wx_166_interrupt_glitch = false;
        if self.d.wy_just_checked {
            self.d.wy_just_checked = false;
        } else if !self.d.wx_triggered && self.d.wy_triggered && self.lcdc & 0x20 != 0 {
            let position = self.d.position_in_line;
            let hw = self.hw_cgb();
            let mut should_activate = false;
            if self.wx == 0 {
                if position == 249 {
                    should_activate = true;
                } else if position == 240 && self.scx & 7 != 0 {
                    should_activate = true;
                } else if (241..=248).contains(&position) {
                    should_activate = true;
                }
            } else if u16::from(self.wx) < 166 + u16::from(hw) {
                if self.wx == position.wrapping_add(7) {
                    should_activate = true;
                } else if !hw
                    && self.wx == position.wrapping_add(6)
                    && !self.d.wx_just_changed
                {
                    should_activate = true;
                    // LCD-PPU horizontal desync on DMG units.
                    if self.is_dmg_family() && self.d.lcd_x > 0 {
                        self.d.lcd_x -= 1;
                    }
                }
            }

            if should_activate {
                self.d.window_y = self.d.window_y.wrapping_add(1);
                self.d.window_tile_x = 0;
                self.d.bg_fifo.clear();
                if self.wx == 0 && self.scx & 7 != 0 && !hw {
                    self.d.cfl += 1;
                    self.sleep(42, 1);
                    return true;
                } else if self.wx == 166 {
                    self.d.wx_166_interrupt_glitch = true;
                }
                self.mode3_window_activated();
            } else if !hw && self.wx == 166 && self.wx == position.wrapping_add(7) {
                self.d.window_y = self.d.window_y.wrapping_add(1);
            }
        }
        false
    }

    const fn mode3_window_activated(&mut self) {
        self.d.wx_triggered = true;
        self.d.fetcher_state = F_GET_TILE_T1;
        self.d.window_is_being_fetched = true;
    }

    // ---------------------------------------------------------------------
    // Mode 3 loop (SameBoy's `while (true)` inside `mode_3_start`)
    // ---------------------------------------------------------------------

    /// Runs the mode-3 loop from `entry` until it sleeps or finishes.
    /// Entries: 0 = top of an iteration, 42/27/41/20/39/40/21 = resume
    /// points after the corresponding SameBoy sleep.
    fn mode3(&mut self, ints: &mut Interrupts, mut entry: u8) -> Mode3Flow {
        loop {
            match entry {
                0 => {
                    if self.mode3_window() {
                        return Mode3Flow::Slept;
                    }
                    entry = 100;
                }
                42 => {
                    self.mode3_window_activated();
                    entry = 100;
                }
                100 => {
                    // Insert a pixel right at the FIFO's end.
                    let hw = self.hw_cgb();
                    if self.wx == self.d.position_in_line.wrapping_add(7)
                        && (!hw || self.wx == 0)
                        && self.d.wx_triggered
                        && !self.d.window_is_being_fetched
                        && self.d.fetcher_state == F_GET_TILE_T1
                        && self.d.bg_fifo.size == 8
                    {
                        self.d.insert_bg_pixel = true;
                    }

                    // Handle objects.
                    while self.d.n_visible_objs != 0
                        && self.d.objects_x[self.d.n_visible_objs - 1] < self.x_for_object_match()
                    {
                        self.d.n_visible_objs -= 1;
                    }
                    self.d.during_object_fetch = true;
                    entry = 101;
                }
                101 => {
                    let n = self.d.n_visible_objs;
                    entry = if n != 0
                        && (self.lcdc & 0x02 != 0 || self.hw_cgb())
                        && self.d.objects_x[n - 1] == self.x_for_object_match()
                    {
                        102
                    } else {
                        130
                    };
                }
                102 => {
                    if self.d.fetcher_state < F_DATA_HIGH_T2 || self.d.bg_fifo.size == 0 {
                        self.advance_fetcher();
                        self.d.cfl += 1;
                        self.sleep(27, 1);
                        return Mode3Flow::Slept;
                    }
                    entry = 103;
                }
                27 => entry = if self.d.object_fetch_aborted { 130 } else { 102 },
                103 => {
                    self.advance_fetcher();
                    self.d.cfl += 1;
                    self.sleep(41, 1);
                    return Mode3Flow::Slept;
                }
                41 => entry = if self.d.object_fetch_aborted { 130 } else { 104 },
                104 => {
                    self.advance_fetcher();
                    let base = u16::from(self.d.visible_objs[self.d.n_visible_objs - 1]) * 4;
                    self.d.mode2_y_bus = self.oam.read(base + 2);
                    self.d.object_flags = self.oam.read(base + 3);
                    self.d.cfl += 2;
                    self.sleep(20, 2);
                    return Mode3Flow::Slept;
                }
                20 => entry = if self.d.object_fetch_aborted { 130 } else { 105 },
                105 => {
                    let n = self.d.n_visible_objs;
                    self.d.object_low_line_address = self.object_line_address(
                        self.d.objects_y[n - 1],
                        self.d.mode2_y_bus,
                        self.d.object_flags,
                    );
                    self.d.object_tile_data[0] = self.vram_read(self.d.object_low_line_address);
                    self.d.cfl += 2;
                    self.sleep(39, 2);
                    return Mode3Flow::Slept;
                }
                39 => entry = if self.d.object_fetch_aborted { 130 } else { 106 },
                106 => {
                    self.d.during_object_fetch = false;
                    self.d.cfl += 1;
                    let n = self.d.n_visible_objs;
                    self.d.object_low_line_address = self.object_line_address(
                        self.d.objects_y[n - 1],
                        self.d.mode2_y_bus,
                        self.d.object_flags,
                    );
                    self.d.object_tile_data[1] =
                        self.vram_read(self.d.object_low_line_address + 1);
                    self.sleep(40, 1);
                    return Mode3Flow::Slept;
                }
                40 => {
                    let n = self.d.n_visible_objs;
                    let flags = self.d.object_flags;
                    let mut palette = u8::from(flags & 0x10 != 0);
                    if self.cgb_mode_on() {
                        palette = flags & 0x7;
                    }
                    let priority = if self.opri_index_priority() {
                        self.d.visible_objs[n - 1]
                    } else {
                        0
                    };
                    let (low, high) = (self.d.object_tile_data[0], self.d.object_tile_data[1]);
                    self.d.oam_fifo.overlay_object_row(
                        low,
                        high,
                        palette,
                        flags & 0x80 != 0,
                        priority,
                        flags & 0x20 != 0,
                    );
                    self.d.data_for_sel_glitch =
                        self.vram_read(self.d.object_low_line_address + 1);
                    if std::env::var("CERES_OBJ").is_ok() && (68..72).contains(&self.d.current_line) {
                        eprintln!("OBJ ly={} pos={} idx={} x={} flags={:02x} data={:02x},{:02x} prio={} fifo_size={}", self.d.current_line, self.d.position_in_line, self.d.visible_objs[n-1], self.d.objects_x[n-1], flags, low, high, priority, self.d.oam_fifo.size);
                    }
                    self.d.n_visible_objs -= 1;
                    entry = 101;
                }
                130 => {
                    // abort_fetching_object:
                    self.d.object_fetch_aborted = false;
                    self.d.during_object_fetch = false;
                    if let Some(out) = self.render_pixel_if_possible() {
                        self.output_pixel(out);
                    }
                    self.advance_fetcher();
                    if self.d.position_in_line == 160 {
                        return Mode3Flow::Done;
                    }
                    self.d.cfl += 1;
                    self.sleep(21, 1);
                    return Mode3Flow::Slept;
                }
                21 => {
                    if self.d.wx_166_interrupt_glitch {
                        self.d.mode_for_interrupt = 0;
                        self.stat_update(ints);
                    }
                    entry = 0;
                }
                _ => unreachable!(),
            }
        }
    }

    /// Object priority: OAM-index priority on CGB hardware unless OPRI
    /// selects X-coordinate priority; DMG hardware always uses X.
    #[inline]
    const fn opri_index_priority(&self) -> bool {
        self.hw_cgb() && !self.opri
    }

    /// Code after the mode-3 loop breaks (`skip_slow_mode_3`).
    fn mode3_done(&mut self, ints: &mut Interrupts) {
        let _ = ints;
        self.d.position_in_line = 240;
        self.d.line_has_fractional_scrolling = false;

        if self.d.fetcher_state == F_DATA_HIGH_T1 || self.d.fetcher_state == F_DATA_HIGH_T2 {
            // Make sure current_tile_data[1] holds the last tile data byte read.
            self.d.current_tile_data[1] = self.d.current_tile_data[0];
        }

        // The PPU and LCD desynced: fill the rest of the line with the last colour.
        self.fill_desynced_line();

        if self.d.current_line == 143 {
            self.d.window_y = 0xFF;
        }
        if !self.hw_cgb()
            && self.d.wy_triggered
            && self.lcdc & 0x20 != 0
            && self.wx == 166
        {
            self.d.wx_triggered = true;
            self.d.window_tile_x = 1;
            self.d.window_y = self.d.window_y.wrapping_add(1);
        } else {
            self.d.wx_triggered = false;
        }

        if !self.double_speed() {
            self.stat &= !super::STAT_MODE_B;
            self.d.mode_for_interrupt = 0;
            self.d.oam_read_blocked = model_ge_cgb_d(self.model);
            self.d.vram_read_blocked = false;
            self.d.oam_write_blocked = false;
            self.d.vram_write_blocked = false;
        }

        self.d.cfl += 1;
        self.sleep(22, 1);
    }

    fn fill_desynced_line(&mut self) {
        while self.d.lcd_x < 160 {
            if self.d.current_line < LINES {
                let x = u32::from(self.d.lcd_x);
                let base = u32::from(self.d.current_line) * u32::from(super::PX_WIDTH);
                if x == 0 {
                    let rgb = self.desync_color();
                    self.rgb_buf.set_px(base, rgb);
                } else {
                    let rgb = self.rgb_buf.px(base + x - 1);
                    self.rgb_buf.set_px(base + x, rgb);
                }
            }
            self.d.lcd_x += 1;
        }
    }

    // ---------------------------------------------------------------------
    // Line-level state machine
    // ---------------------------------------------------------------------

    /// Start of a line for lines 0..=143 (SameBoy's `for` body head).
    fn line_start(&mut self) {
        self.wy_check();
        self.d.oam_write_blocked = self.hw_cgb() && !self.double_speed();
        self.sleep(35, 2);
    }

    fn present_frame(&mut self) {
        self.rgba_buf_present = core::mem::take(&mut self.rgb_buf);
    }

    /// Advance the display by one dot.
    pub(super) fn run_display(&mut self, ints: &mut Interrupts) {
        // Pre-run bookkeeping (top of GB_display_run).
        if self.d.wy_triggered {
            self.d.wy_check_scheduled = false;
        }

        // A line that would outgrow 456 dots is cut off (mode 3 abort).
        if i32::from(self.d.cfl) - self.d.wait + 2 > LINE_LENGTH && self.d.state != 0 {
            if self.d.state == 22 {
                self.stat &= !super::STAT_MODE_B;
                self.d.mode_for_interrupt = 0;
                self.stat_update(ints);
            }
            self.d.state = 9;
            self.d.wait = 0;
        }

        if self.d.delayed_glitch_hblank_interrupt && self.d.current_line < LINES {
            self.d.delayed_glitch_hblank_interrupt = false;
            self.d.mode_for_interrupt = 0;
            self.stat_update(ints);
            self.d.mode_for_interrupt = 3;
        }

        self.d.line_clock += 1;
        self.step_state_machine(ints);

        // The scheduled WY check lands after the dot it was due on.
        if self.d.wy_check_scheduled && !self.d.wy_triggered {
            self.d.wy_check_countdown -= 1;
            if self.d.wy_check_countdown <= 0 {
                self.d.wy_check_scheduled = false;
                self.wy_check();
                if self.d.state == 21 && self.hw_cgb() && !self.double_speed() {
                    self.d.wy_just_checked = true;
                }
            }
        }
    }

    fn step_state_machine(&mut self, ints: &mut Interrupts) {
        // Sleeping: `wait` dots remain, resume when it reaches zero.
        if self.d.wait > 0 {
            self.d.wait -= 1;
            if self.d.wait > 0 {
                return;
            }
        }

        let mut state = self.d.state;
        loop {
            if std::env::var("CERES_TRACE").is_ok() { eprintln!("S{state} line={} cfl={} ly={} stat={:02x}", self.d.current_line, self.d.cfl, self.ly, self.stat); }
            match state {
                // ---- start of a frame after LCD on ----
                0 => {
                    if !self.hw_cgb() {
                        self.sleep(23, 1);
                        return;
                    }
                    state = 23;
                }
                23 => {
                    // Handle mode 2 on the very first line 0.
                    self.d.current_line = 0;
                    self.d.window_y = 0xFF;
                    self.d.wy_triggered = false;
                    self.d.position_in_line = 240;
                    self.d.line_has_fractional_scrolling = false;
                    self.d.ly_for_comparison = 0;
                    self.stat &= !super::STAT_MODE_B;
                    self.d.mode_for_interrupt = -1;
                    self.d.oam_read_blocked = false;
                    self.d.vram_read_blocked = false;
                    self.d.oam_write_blocked = false;
                    self.d.vram_write_blocked = false;
                    self.d.cgb_palettes_blocked = false;
                    self.d.cfl = MODE2_LENGTH - 4;
                    self.d.line_clock = 0;
                    self.stat_update(ints);
                    self.sleep(2, MODE2_LENGTH - 4);
                    return;
                }
                2 => {
                    self.d.oam_write_blocked = true;
                    self.d.cfl += 2;
                    self.stat_update(ints);
                    self.sleep(34, 2);
                    return;
                }
                34 => {
                    self.d.n_visible_objs = 0;
                    self.d.orig_n_visible_objs = 0;
                    // Mode 0 is shorter on the first line 0.
                    self.d.cfl += 8;
                    self.stat = (self.stat & !super::STAT_MODE_B) | 3;
                    self.d.mode_for_interrupt = 3;
                    self.d.oam_write_blocked = true;
                    self.d.oam_read_blocked = true;
                    self.d.vram_read_blocked = self.double_speed();
                    self.d.vram_write_blocked = self.double_speed();
                    if !self.hw_cgb() {
                        self.d.vram_read_blocked = true;
                        self.d.vram_write_blocked = true;
                    }
                    self.d.cfl += 2;
                    self.sleep(37, 2);
                    return;
                }
                37 => {
                    self.d.cgb_palettes_blocked = true;
                    self.d.cfl += 3;
                    self.sleep(38, 3);
                    return;
                }
                38 => {
                    self.d.vram_read_blocked = true;
                    self.d.vram_write_blocked = true;
                    self.d.wx_triggered = false;
                    self.mode3_start();
                    state = 200;
                }

                // ---- lines 0..=143 ----
                35 => {
                    self.d.oam_write_blocked = self.hw_cgb();
                    self.sleep(6, 1);
                    return;
                }
                6 => {
                    self.ly = self.d.current_line;
                    self.d.oam_read_blocked = !self.double_speed() || model_ge_cgb_d(self.model);
                    self.d.ly_for_comparison = if self.d.current_line != 0 { -1 } else { 0 };
                    // The OAM STAT interrupt occurs 1 T-cycle before STAT
                    // actually changes, except on line 0.
                    if self.d.current_line != 0 {
                        self.d.mode_for_interrupt = 2;
                        self.stat &= !super::STAT_MODE_B;
                    } else if !self.hw_cgb() {
                        self.stat &= !super::STAT_MODE_B;
                    }
                    self.stat_update(ints);
                    self.sleep(7, 1);
                    return;
                }
                7 => {
                    self.d.oam_read_blocked = true;
                    self.d.oam_write_blocked = true;
                    self.stat = (self.stat & !super::STAT_MODE_B) | 2;
                    self.d.mode_for_interrupt = 2;
                    self.d.ly_for_comparison = i32::from(self.d.current_line);
                    self.wy_check();
                    self.stat_update(ints);
                    self.d.mode_for_interrupt = -1;
                    self.stat_update(ints);
                    self.d.n_visible_objs = 0;
                    self.d.orig_n_visible_objs = 0;
                    self.d.oam_search_index = 0;
                    state = 201;
                }
                201 => {
                    // OAM search loop head: CGB adds the object before the sleep.
                    if self.hw_cgb() {
                        self.add_object_from_index(self.d.oam_search_index);
                    }
                    self.sleep(8, 2);
                    return;
                }
                8 => {
                    if !self.hw_cgb() {
                        self.add_object_from_index(self.d.oam_search_index);
                    }
                    if self.d.oam_search_index == 37 {
                        self.d.vram_read_blocked = !self.hw_cgb();
                        self.d.vram_write_blocked = false;
                        self.d.cgb_palettes_blocked = false;
                        self.d.oam_write_blocked = self.hw_cgb();
                    }
                    self.d.oam_search_index += 1;
                    if self.d.oam_search_index < 40 {
                        state = 201;
                    } else {
                        self.d.cfl = MODE2_LENGTH + 4;
                        self.d.orig_n_visible_objs = self.d.n_visible_objs;
                        self.stat = (self.stat & !super::STAT_MODE_B) | 3;
                        self.d.mode_for_interrupt = 3;
                        self.d.vram_read_blocked = true;
                        self.d.vram_write_blocked = true;
                        self.d.cgb_palettes_blocked = false;
                        self.d.oam_write_blocked = true;
                        self.d.oam_read_blocked = true;
                        self.stat_update(ints);
                        self.d.cfl += 3;
                        self.sleep(10, 3);
                        return;
                    }
                }
                10 => {
                    self.d.cgb_palettes_blocked = true;
                    self.d.cfl += 2;
                    self.sleep(32, 2);
                    return;
                }
                32 => {
                    self.mode3_start();
                    state = 200;
                }

                // ---- mode 3 ----
                200 | 42 | 27 | 41 | 20 | 39 | 40 | 21 => {
                    let entry = if state == 200 { 0 } else { state };
                    match self.mode3(ints, entry) {
                        Mode3Flow::Slept => return,
                        Mode3Flow::Done => {
                            self.mode3_done(ints);
                            return;
                        }
                    }
                }

                // ---- HBlank ----
                22 => {
                    self.stat &= !super::STAT_MODE_B;
                    self.d.mode_for_interrupt = 0;
                    self.d.oam_read_blocked = false;
                    self.d.vram_read_blocked = false;
                    self.d.oam_write_blocked = false;
                    self.d.vram_write_blocked = false;
                    self.stat_update(ints);
                    self.d.cfl += 2;
                    self.sleep(33, 2);
                    return;
                }
                33 => {
                    self.d.cgb_palettes_blocked = !self.double_speed();
                    self.d.cfl += 2;
                    self.sleep(36, 2);
                    return;
                }
                36 => {
                    self.d.cgb_palettes_blocked = false;
                    if self.d.cfl > LINE_LENGTH - 2 {
                        self.d.cfl = 0;
                        self.sleep(43, LINE_LENGTH);
                    } else {
                        let saved = self.d.cfl;
                        self.d.cfl = 0;
                        self.sleep(11, LINE_LENGTH - saved - 2);
                    }
                    return;
                }
                43 | 9 => {
                    // display9: mode 3 abort.
                    self.fill_desynced_line();
                    self.d.n_visible_objs = self.d.orig_n_visible_objs;
                    self.d.current_line += 1;
                    self.wy_check();
                    self.d.cfl = 0;
                    if self.d.current_line != LINES {
                        self.d.cfl = 2;
                        self.sleep(28, 2);
                        return;
                    }
                    let p = self.d.position_in_line;
                    if p >= 156 && p < 240 {
                        self.d.delayed_glitch_hblank_interrupt = true;
                    }
                    self.d.position_in_line = 240;
                    self.d.line_has_fractional_scrolling = false;
                    state = 210;
                }
                28 => {
                    self.ly = self.d.current_line;
                    let p = self.d.position_in_line;
                    if p >= 156 && p < 240 {
                        self.d.delayed_glitch_hblank_interrupt = true;
                    }
                    self.stat_update(ints);
                    self.d.position_in_line = 241;
                    self.mode3_start();
                    state = 200;
                }
                11 => {
                    self.d.cfl = 0;
                    self.d.line_clock = 0;
                    self.sleep(31, 2);
                    return;
                }
                31 => {
                    if self.d.current_line != LINES - 1 {
                        self.d.mode_for_interrupt = 2;
                    }
                    self.d.current_line += 1;
                    if self.d.current_line < LINES {
                        self.line_start();
                        return;
                    }
                    state = 210;
                }

                // ---- lines 144..=152 ----
                210 => {
                    self.d.ly_for_comparison = -1;
                    self.stat_update(ints);
                    self.sleep(26, 2);
                    return;
                }
                26 => {
                    self.ly = self.d.current_line;
                    if self.d.current_line == LINES
                        && !self.d.stat_interrupt_line
                        && self.stat & STAT_IF_OAM_B != 0
                    {
                        ints.request_lcd();
                    }
                    self.sleep(12, 2);
                    return;
                }
                12 => {
                    if self.d.delayed_glitch_hblank_interrupt {
                        self.d.delayed_glitch_hblank_interrupt = false;
                        self.d.mode_for_interrupt = 0;
                    }
                    self.d.ly_for_comparison = i32::from(self.d.current_line);
                    self.stat_update(ints);
                    self.sleep(24, 1);
                    return;
                }
                24 => {
                    if self.d.current_line == LINES {
                        // Entering VBlank also triggers the OAM interrupt.
                        self.stat &= !super::STAT_MODE_B;
                        self.stat |= 1;
                        ints.request_vblank();
                        if !self.d.stat_interrupt_line && self.stat & STAT_IF_OAM_B != 0 {
                            ints.request_lcd();
                        }
                        self.d.mode_for_interrupt = 1;
                        self.stat_update(ints);
                        self.present_frame();
                    }
                    self.sleep(13, LINE_LENGTH - 5);
                    return;
                }
                13 => {
                    self.d.current_line += 1;
                    if self.d.current_line < 153 {
                        state = 210;
                    } else {
                        state = 220;
                    }
                }

                // ---- line 153 ----
                220 => {
                    self.d.ly_for_comparison = -1;
                    self.stat_update(ints);
                    self.sleep(19, 2);
                    return;
                }
                19 => {
                    self.ly = 153;
                    self.sleep(14, if model_ge_cgb_d(self.model) { 2 } else { 4 });
                    return;
                }
                14 => {
                    if !model_ge_cgb_d(self.model) && !self.double_speed() {
                        self.ly = 0;
                    }
                    self.d.ly_for_comparison = 153;
                    self.stat_update(ints);
                    self.sleep(15, if model_ge_cgb_d(self.model) { 4 } else { 2 });
                    return;
                }
                15 => {
                    self.ly = 0;
                    self.d.ly_for_comparison =
                        if model_ge_cgb_d(self.model) || self.double_speed() { 153 } else { -1 };
                    self.stat_update(ints);
                    self.sleep(16, 4);
                    return;
                }
                16 => {
                    self.d.ly_for_comparison = 0;
                    self.stat_update(ints);
                    // Writing to LYC during this period on a CGB has side effects.
                    self.sleep(29, 12);
                    return;
                }
                29 => {
                    self.sleep(17, LINE_LENGTH - 24);
                    return;
                }
                17 => {
                    self.d.current_line = 0;
                    self.d.wy_triggered = false;
                    self.line_start();
                    return;
                }

                _ => {
                    // Unknown state: restart the machine.
                    state = 0;
                }
            }
        }
    }
}

enum Mode3Flow {
    Slept,
    Done,
}

impl Ppu {
    /// Called by the CPU's LCDC write handler: disabling the window while a
    /// window tile is being fetched suppresses the pixel-insertion glitch.
    pub fn note_window_disable(&mut self, old: u8, new: u8) {
        if old & 0x20 != 0 && new & 0x20 == 0 && self.d.window_is_being_fetched {
            self.d.disable_window_pixel_insertion_glitch = true;
        }
    }

    /// DMG: disabling objects while an object is being fetched aborts it.
    pub(super) fn abort_object_fetch_on_obj_disable(&mut self, val: u8) {
        if !self.hw_cgb()
            && self.lcdc & 0x02 != 0
            && val & 0x02 == 0
            && self.d.during_object_fetch
        {
            self.d.cfl -= self.d.wait - 1;
            self.d.wait = 1;
            self.d.object_fetch_aborted = true;
        }
    }

    pub(super) fn write_lyc_reg(&mut self, val: u8, ints: &mut Interrupts) {
        let state = self.d.state;
        let cgb = self.hw_cgb();
        // These are the states around LY changes; the display routine calls
        // `stat_update` itself so LYC writes conflict on the right dot.
        if state == 29 && cgb {
            self.d.ly_for_comparison = 153;
            self.stat_update(ints);
            self.d.ly_for_comparison = 0;
        }
        self.lyc = val;
        if !cgb || (state != 35 && state != 26 && state != 15 && state != 16) {
            if state == 14 && cgb {
                self.d.ly_for_comparison = 153;
                self.stat_update(ints);
                self.d.ly_for_comparison = -1;
            } else {
                self.stat_update(ints);
            }
        }
    }

    pub(super) fn write_stat_reg(&mut self, val: u8, ints: &mut Interrupts) {
        self.stat &= 7;
        self.stat |= val & !7;
        self.stat |= 0x80;

        // Annoying edge timing case.
        if self.double_speed()
            && self.d.state == 8
            && self.d.oam_search_index == 0
            && self.d.wait == 1
            && val & 0x20 != 0
        {
            self.d.mode_for_interrupt = 2;
            self.stat_update(ints);
            self.d.mode_for_interrupt = -1;
        } else {
            self.stat_update(ints);
        }
    }

    // CPU-visible memory access, gated by the flags above.

    #[must_use]
    pub const fn vram_read_blocked(&self) -> bool {
        self.d.vram_read_blocked
    }

    #[must_use]
    pub const fn vram_write_blocked(&self) -> bool {
        self.d.vram_write_blocked
    }

    #[must_use]
    pub const fn oam_read_blocked(&self) -> bool {
        self.d.oam_read_blocked
    }

    #[must_use]
    pub const fn oam_write_blocked(&self) -> bool {
        self.d.oam_write_blocked
    }
}
