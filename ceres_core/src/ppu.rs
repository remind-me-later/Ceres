use crate::{CompatMode, Gb, IF_LCD_B, IF_VBLANK_B};

/// `GameBoy` screen width in pixels.
pub const PX_WIDTH: u8 = 160;

/// `GameBoy` screen height in pixels.
pub const PX_HEIGHT: u8 = 144;

const PX_TOTAL: u16 = PX_WIDTH as u16 * PX_HEIGHT as u16;

// Mode timings
const OAM_SCAN_CYCLES: i32 = 80; // Constant
const DRAWING_CYCLES: i32 = 172; // Variable, minimum ammount
const HBLANK_CYCLES: i32 = 204; // Variable, maximum ammount
const VBLANK_CYCLES: i32 = 456; // Constant

// LCDC bits
const LCDC_BG_B: u8 = 1;
const LCDC_OBJ_B: u8 = 1 << 1;
const LCDC_OBJL_B: u8 = 1 << 2;
const LCDC_BG_AREA: u8 = 1 << 3;
const LCDC_BG_SIGNED: u8 = 1 << 4;
const LCDC_WIN_B: u8 = 1 << 5;
const LCDC_WIN_AREA: u8 = 1 << 6;
const LCDC_ON_B: u8 = 1 << 7;

// STAT bits
const STAT_MODE_B: u8 = 3;
const STAT_LYC_B: u8 = 4;
const STAT_IF_HBLANK_B: u8 = 8;
const STAT_IF_VBLANK_B: u8 = 0x10;
const STAT_IF_OAM_B: u8 = 0x20;
const STAT_IF_LYC_B: u8 = 0x40;

// BG attributes bits
const BG_PAL_B: u8 = 0x7;
const BG_VBK_B: u8 = 0x8;
const BG_X_FLIP_B: u8 = 0x20;
const BG_Y_FLIP_B: u8 = 0x40;
const BG_PR_B: u8 = 0x80;

pub const OAM_SIZE: usize = 0x100;

const VRAM_SIZE: u16 = 0x2000;
pub const VRAM_SIZE_CGB: usize = VRAM_SIZE as usize * 2;

// Sprite attributes bites
const SPR_CGB_PAL: u8 = 0x7;
const SPR_TILE_BANK: u8 = 0x8;
const SPR_PAL: u8 = 0x10;
const SPR_FLIP_X: u8 = 0x20;
const SPR_FLIP_Y: u8 = 0x40;
const SPR_BG_FIRST: u8 = 0x80;

// CGB palette RAM
const PAL_RAM_SIZE: usize = 0x20;
const PAL_RAM_SIZE_COLORS: usize = PAL_RAM_SIZE * 3;

// DMG palette colors RGB
const GRAYSCALE_PALETTE: [(u8, u8, u8); 4] = [
    (0xFF, 0xFF, 0xFF),
    (0xCC, 0xCC, 0xCC),
    (0x77, 0x77, 0x77),
    (0x00, 0x00, 0x00),
];

const RGBA_BUF_SIZE: usize = PX_TOTAL as usize * 4;

pub struct RgbaBuf {
    data: [u8; RGBA_BUF_SIZE],
}

impl Default for RgbaBuf {
    fn default() -> Self {
        Self {
            data: [0xFF; RGBA_BUF_SIZE],
        }
    }
}

impl RgbaBuf {
    #[inline]
    fn set_px(&mut self, i: usize, rgb: (u8, u8, u8)) {
        let base = i * 4;
        self.data[base] = rgb.0;
        self.data[base + 1] = rgb.1;
        self.data[base + 2] = rgb.2;
    }

    fn clear(&mut self) {
        self.data = [0xFF; RGBA_BUF_SIZE];
    }

    pub fn pixel_data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    HBlank  = 0,
    VBlank  = 1,
    OamScan = 2,
    Drawing = 3,
}

impl Mode {
    pub(crate) fn cycles(self, scroll_x: u8) -> i32 {
        let scroll_adjust = i32::from(scroll_x & 7) * 4;
        match self {
            Self::OamScan => OAM_SCAN_CYCLES,
            Self::Drawing => DRAWING_CYCLES + scroll_adjust,
            Self::HBlank => HBLANK_CYCLES - scroll_adjust,
            Self::VBlank => VBLANK_CYCLES,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Priority {
    Sprites,
    Bg,
    Normal,
}

fn shade_index(palette: u8, color: u8) -> u8 {
    (palette >> (color * 2)) & 0x3
}

pub struct ColorPalette {
    // Rgb color ram
    col: [u8; PAL_RAM_SIZE_COLORS],
    idx: u8,
    inc: bool, // increment after write
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            col: [0; PAL_RAM_SIZE_COLORS],
            idx: 0,
            inc: false,
        }
    }
}

impl ColorPalette {
    #[inline]
    pub(crate) fn set_spec(&mut self, val: u8) {
        self.idx = val & 0x3F;
        self.inc = val & 0x80 != 0;
    }

    #[inline]
    pub(crate) fn spec(&self) -> u8 {
        self.idx | 0x40 | (u8::from(self.inc) << 7)
    }

    pub(crate) fn data(&self) -> u8 {
        let i = (self.idx as usize / 2) * 3;

        if self.idx & 1 == 0 {
            // red and green
            let r = self.col[i];
            let g = self.col[i + 1] << 5;
            r | g
        } else {
            // green and blue
            let g = self.col[i + 1] >> 3;
            let b = self.col[i + 2] << 2;
            g | b
        }
    }

    pub(crate) fn set_data(&mut self, val: u8) {
        let i = (self.idx as usize / 2) * 3;

        if self.idx & 1 == 0 {
            // red
            self.col[i] = val & 0x1F;
            // green
            let tmp = (self.col[i + 1] & 3) << 3;
            self.col[i + 1] = tmp | (val & 0xE0) >> 5;
        } else {
            // green
            let tmp = self.col[i + 1] & 7;
            self.col[i + 1] = tmp | (val & 3) << 3;
            // blue
            self.col[i + 2] = (val & 0x7C) >> 2;
        }

        // if auto-increment is enabled increment index with
        // some branchless trickery, reference code:
        // if self.inc {
        //     self.idx = (self.idx + 1) & 0x3F;
        // }
        let mask = u8::from(self.inc).wrapping_sub(1);
        self.idx = ((self.idx + 1) & 0x3F) & !mask | self.idx & mask;
    }

    fn rgb(&self, palette: u8, color: u8) -> (u8, u8, u8) {
        fn scale_channel(c: u8) -> u8 {
            (c << 3) | (c >> 2)
        }

        let i = (palette as usize * 4 + color as usize) * 3;
        let r = self.col[i];
        let g = self.col[i + 1];
        let b = self.col[i + 2];

        (scale_channel(r), scale_channel(g), scale_channel(b))
    }
}

#[derive(Default)]
struct Obj {
    pub x: u8,
    pub y: u8,
    pub tile_index: u8,
    pub attr: u8,
}

impl Gb {
    pub(crate) fn run_ppu(&mut self, cycles: i32) {
        fn check_lyc(gb: &mut Gb) {
            gb.stat &= !STAT_LYC_B;

            if gb.ly == gb.lyc {
                gb.stat |= STAT_LYC_B;
                if gb.stat & STAT_IF_LYC_B != 0 {
                    gb.ifr |= IF_LCD_B;
                }
            }
        }

        if self.lcdc & LCDC_ON_B != 0 && !self.lcdc_delay {
            // advance in 0x40 t-cycle chunks to avoid skipping a state
            // machine transition
            // TODO: think of something more elegant
            let chunks = (cycles >> 6) + 1;

            for i in 0..chunks {
                // TODO: WTF?????
                let new_cycles = if i == chunks - 1 {
                    // last iteration
                    self.ppu_cycles - (cycles & 0x3F)
                } else {
                    self.ppu_cycles - 0x40
                };
                self.ppu_cycles = new_cycles;

                if new_cycles >= 0 {
                    continue;
                }

                match self.ppu_mode() {
                    Mode::OamScan => self.switch_mode(Mode::Drawing),
                    Mode::Drawing => {
                        self.draw_scanline();
                        self.switch_mode(Mode::HBlank);
                    }
                    Mode::HBlank => {
                        self.ly += 1;
                        if self.ly < 144 {
                            self.switch_mode(Mode::OamScan);
                        } else {
                            self.switch_mode(Mode::VBlank);
                        }
                        check_lyc(self);
                    }
                    Mode::VBlank => {
                        self.ly += 1;
                        if self.ly > 153 {
                            self.ly = 0;
                            self.switch_mode(Mode::OamScan);
                        } else {
                            let scx = self.scx;
                            self.ppu_cycles =
                                self.ppu_cycles.wrapping_add(self.ppu_mode().cycles(scx));
                        }
                        check_lyc(self);
                    }
                }
            }
        }

        self.frame_dots += cycles;

        if self.frame_dots >= 70224 {
            if self.lcdc_delay {
                self.lcdc_delay = false;
            }

            self.running_frame = false;
            self.frame_dots -= 70224;
        }
    }

    #[must_use]
    pub(crate) fn ppu_mode(&self) -> Mode {
        match self.stat & 3 {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamScan,
            3 => Mode::Drawing,
            _ => unreachable!(),
        }
    }

    pub(crate) fn read_vram(&mut self, addr: u16) -> u8 {
        if self.ppu_mode() == Mode::Drawing {
            0xFF
        } else {
            let bank = u16::from(self.vbk) * VRAM_SIZE;
            let i = (addr & 0x1FFF) + bank;
            self.vram[i as usize]
        }
    }

    pub(crate) fn read_oam(&mut self, addr: u16) -> u8 {
        match self.ppu_mode() {
            Mode::HBlank | Mode::VBlank if !self.dma_on => self.oam[(addr & 0xFF) as usize],
            _ => 0xFF,
        }
    }

    pub(crate) fn write_lcdc(&mut self, val: u8) {
        // turn off
        if val & LCDC_ON_B == 0 && self.lcdc & LCDC_ON_B != 0 {
            debug_assert!(self.ppu_mode() == Mode::VBlank);
            self.ly = 0;
            self.rgba_buf.clear();
            self.frame_dots = 0;
        }

        // turn on
        if val & LCDC_ON_B != 0 && self.lcdc & LCDC_ON_B == 0 {
            self.set_mode(Mode::HBlank);
            self.stat &= !STAT_LYC_B;
            self.stat |= STAT_LYC_B;
            self.ppu_cycles = Mode::OamScan.cycles(self.scx);
            self.lcdc_delay = true;
            self.frame_dots = 0;
        }

        self.lcdc = val;
    }

    pub(crate) fn write_stat(&mut self, val: u8) {
        let ly_equals_lyc = self.stat & STAT_LYC_B;
        let mode: u8 = self.ppu_mode() as u8;

        self.stat = val;
        self.stat &= !(STAT_LYC_B | STAT_MODE_B);
        self.stat |= ly_equals_lyc | mode;
    }

    pub(crate) fn write_vram(&mut self, addr: u16, val: u8) {
        if self.ppu_mode() != Mode::Drawing {
            let bank = u16::from(self.vbk) * VRAM_SIZE as u16;
            let i = (addr & 0x1FFF) + bank;
            self.vram[i as usize] = val;
        }
    }

    pub(crate) fn write_oam(&mut self, addr: u16, val: u8, dma_active: bool) {
        match self.ppu_mode() {
            Mode::HBlank | Mode::VBlank if !dma_active => {
                self.oam[(addr & 0xFF) as usize] = val;
            }
            _ => (),
        };
    }

    #[inline]
    fn set_mode(&mut self, mode: Mode) {
        self.stat = (self.stat & !STAT_MODE_B) | mode as u8;
    }

    #[inline]
    fn mono_rgb(index: u8) -> (u8, u8, u8) {
        GRAYSCALE_PALETTE[index as usize]
    }

    fn switch_mode(&mut self, mode: Mode) {
        self.set_mode(mode);
        let scx = self.scx;
        self.ppu_cycles = self.ppu_cycles.wrapping_add(mode.cycles(scx));

        match mode {
            Mode::OamScan => {
                if self.stat & STAT_IF_OAM_B != 0 {
                    self.ifr |= IF_LCD_B;
                }

                self.ppu_win_in_ly = false;
            }
            Mode::VBlank => {
                self.ifr |= IF_VBLANK_B;

                if self.stat & STAT_IF_VBLANK_B != 0 {
                    self.ifr |= IF_LCD_B;
                }

                if self.stat & STAT_IF_OAM_B != 0 {
                    self.ifr |= IF_LCD_B;
                }

                self.ppu_win_skipped = 0;
                self.ppu_win_in_frame = false;
            }
            Mode::Drawing => (),
            Mode::HBlank => {
                if self.stat & STAT_IF_HBLANK_B != 0 {
                    self.ifr |= IF_LCD_B;
                }
            }
        }
    }

    #[inline]
    fn win_enabled(&self) -> bool {
        match self.compat_mode {
            CompatMode::Dmg | CompatMode::Compat => {
                self.lcdc & (LCDC_BG_B | LCDC_WIN_B) == (LCDC_BG_B | LCDC_WIN_B)
            }
            CompatMode::Cgb => self.lcdc & LCDC_WIN_B != 0,
        }
    }

    #[inline]
    fn bg_enabled(&self) -> bool {
        match self.compat_mode {
            CompatMode::Dmg | CompatMode::Compat => self.lcdc & LCDC_BG_B != 0,
            CompatMode::Cgb => true,
        }
    }

    #[inline]
    fn cgb_master_priority(&self) -> bool {
        match self.compat_mode {
            CompatMode::Dmg | CompatMode::Compat => false,
            CompatMode::Cgb => self.lcdc & LCDC_BG_B == 0,
        }
    }

    #[inline]
    fn bg_tile_map(&self) -> u16 {
        0x9800 | u16::from(self.lcdc & LCDC_BG_AREA != 0) << 10
    }

    #[inline]
    fn win_tile_map(&self) -> u16 {
        0x9800 | u16::from(self.lcdc & LCDC_WIN_AREA != 0) << 10
    }

    #[inline]
    fn tile_addr(&self, tile_num: u8) -> u16 {
        let signed = self.lcdc & LCDC_BG_SIGNED == 0;
        let base = 0x8000 | u16::from(signed) << 11;

        let offset = if signed {
            #[allow(clippy::cast_possible_wrap)]
            let tile_num = tile_num as i8;
            #[allow(clippy::cast_sign_loss)]
            let tile_num = (i16::from(tile_num) + 0x80) as u16;
            tile_num
        } else {
            u16::from(tile_num)
        };

        base + offset * 16
    }

    #[inline]
    fn vram_at_bank(&self, addr: u16, bank: u8) -> u8 {
        self.vram[((addr & 0x1FFF) + u16::from(bank) * VRAM_SIZE as u16) as usize]
    }

    #[inline]
    fn bg_tile(&self, tile_addr: u16, attr: u8) -> (u8, u8) {
        let bank = u8::from(attr & BG_VBK_B != 0);
        let lo = self.vram_at_bank(tile_addr, bank);
        let hi = self.vram_at_bank(tile_addr + 1, bank);
        (lo, hi)
    }

    #[inline]
    fn obj_tile(&self, tile_addr: u16, obj: &Obj) -> (u8, u8) {
        let bank = u8::from(obj.attr & SPR_TILE_BANK != 0);
        let lo = self.vram_at_bank(tile_addr, bank);
        let hi = self.vram_at_bank(tile_addr + 1, bank);
        (lo, hi)
    }

    #[inline]
    fn draw_scanline(&mut self) {
        let mut bg_priority = [Priority::Normal; PX_WIDTH as usize];
        let base_idx = PX_WIDTH as usize * self.ly as usize;

        self.draw_bg(&mut bg_priority, base_idx);
        self.draw_win(&mut bg_priority, base_idx);
        self.draw_obj(&mut bg_priority, base_idx);
    }

    #[inline]
    fn draw_bg(&mut self, bg_priority: &mut [Priority; PX_WIDTH as usize], base_idx: usize) {
        if !self.bg_enabled() {
            return;
        }

        let y = self.ly.wrapping_add(self.scy);
        let row = u16::from(y / 8) * 32;
        let line = u16::from((y & 7) * 2);

        for i in 0..PX_WIDTH {
            let x = i.wrapping_add(self.scx);
            let col = u16::from(x / 8);

            let tile_map = self.bg_tile_map() + row + col;

            let attr = match self.compat_mode {
                CompatMode::Dmg | CompatMode::Compat => 0,
                CompatMode::Cgb => self.vram_at_bank(tile_map, 1),
            };

            let color = {
                let tile_num = self.vram_at_bank(tile_map, 0);

                let tile_addr = self.tile_addr(tile_num)
                    + if attr & BG_Y_FLIP_B == 0 {
                        line
                    } else {
                        14 - line
                    };

                let (lo, hi) = self.bg_tile(tile_addr, attr);

                let mut bit = x & 7;
                if attr & BG_X_FLIP_B == 0 {
                    bit = 7 - bit;
                }
                let bit = 1 << bit;

                u8::from(hi & bit != 0) << 1 | u8::from(lo & bit != 0)
            };

            let rgb = match self.compat_mode {
                CompatMode::Dmg => Self::mono_rgb(shade_index(self.bgp, color)),
                CompatMode::Compat => self.bcp.rgb(attr & BG_PAL_B, shade_index(self.bgp, color)),
                CompatMode::Cgb => self.bcp.rgb(attr & BG_PAL_B, color),
            };

            self.rgba_buf.set_px(base_idx + i as usize, rgb);

            bg_priority[i as usize] = if color == 0 {
                Priority::Sprites
            } else if attr & BG_PR_B != 0 {
                Priority::Bg
            } else {
                Priority::Normal
            };
        }
    }

    #[inline]
    fn draw_win(&mut self, bg_priority: &mut [Priority; PX_WIDTH as usize], base_idx: usize) {
        // not so sure about last condition...
        if !(self.win_enabled() && self.wy <= self.ly && self.wx < PX_WIDTH) {
            if self.ppu_win_in_frame {
                self.ppu_win_skipped += 1;
            }
            return;
        }

        let wx = self.wx.saturating_sub(7);
        let y = (self.ly - self.wy).wrapping_sub(self.ppu_win_skipped) as u8;
        let row = u16::from(y / 8) * 32;
        let line = u16::from((y & 7) * 2);

        for i in wx..PX_WIDTH {
            self.ppu_win_in_frame = true;
            self.ppu_win_in_ly = true;

            let x = i.wrapping_sub(wx);
            let col = u16::from(x / 8);

            let tile_map = self.win_tile_map() + row + col;

            let attr = match self.compat_mode {
                CompatMode::Dmg | CompatMode::Compat => 0,
                CompatMode::Cgb => self.vram_at_bank(tile_map, 1),
            };

            let color = {
                let tile_num = self.vram_at_bank(tile_map, 0);

                let tile_addr = self.tile_addr(tile_num)
                    + if attr & BG_Y_FLIP_B == 0 {
                        line
                    } else {
                        14 - line
                    };

                let (lo, hi) = self.bg_tile(tile_addr, attr);

                let mut bit = x & 7;
                if attr & BG_X_FLIP_B == 0 {
                    bit = 7 - bit;
                }
                let bit = 1 << bit;

                u8::from(hi & bit != 0) << 1 | u8::from(lo & bit != 0)
            };

            let rgb = match self.compat_mode {
                CompatMode::Dmg => Self::mono_rgb(shade_index(self.bgp, color)),
                CompatMode::Compat => self.bcp.rgb(attr & BG_PAL_B, shade_index(self.bgp, color)),
                CompatMode::Cgb => self.bcp.rgb(attr & BG_PAL_B, color),
            };

            bg_priority[i as usize] = if color == 0 {
                Priority::Sprites
            } else if attr & BG_PR_B != 0 {
                Priority::Bg
            } else {
                Priority::Normal
            };

            self.rgba_buf.set_px(base_idx + i as usize, rgb);
        }
    }

    #[inline]
    fn objs_in_ly(&mut self, height: u8) -> ([Obj; 10], usize) {
        let mut len = 0;
        // TODO: not pretty
        let mut obj = [
            Obj::default(),
            Obj::default(),
            Obj::default(),
            Obj::default(),
            Obj::default(),
            Obj::default(),
            Obj::default(),
            Obj::default(),
            Obj::default(),
            Obj::default(),
        ];

        for i in (0..OAM_SIZE).step_by(4) {
            let y = self.oam[i].wrapping_sub(16);

            if self.ly.wrapping_sub(y) < height {
                let attr = Obj {
                    y,
                    x: self.oam[i + 1].wrapping_sub(8),
                    tile_index: self.oam[i + 2],
                    attr: self.oam[i + 3],
                };

                obj[len] = attr;
                len += 1;

                if len == 10 {
                    break;
                }
            }
        }

        match self.compat_mode {
            CompatMode::Cgb => {
                for i in 1..len {
                    let mut j = i;
                    while j > 0 {
                        obj.swap(j - 1, j);
                        j -= 1;
                    }
                }
            }
            _ => {
                for i in 1..len {
                    let mut j = i;
                    while j > 0 && obj[j - 1].x <= obj[j].x {
                        obj.swap(j - 1, j);
                        j -= 1;
                    }
                }
            }
        }

        (obj, len)
    }

    #[inline]
    fn draw_obj(&mut self, bg_priority: &mut [Priority; PX_WIDTH as usize], base_idx: usize) {
        if self.lcdc & LCDC_OBJ_B == 0 {
            return;
        }

        let large = self.lcdc & LCDC_OBJL_B != 0;
        let height = 8 * (u8::from(large) + 1);

        let (objs, len) = self.objs_in_ly(height);

        for obj in objs.iter().take(len) {
            let tile_addr = {
                let tile_number = if large {
                    obj.tile_index & !1
                } else {
                    obj.tile_index
                };

                let offset = if obj.attr & SPR_FLIP_Y == 0 {
                    u16::from(self.ly.wrapping_sub(obj.y)) * 2
                } else {
                    (u16::from(height) - 1).wrapping_sub(u16::from(self.ly.wrapping_sub(obj.y))) * 2
                };

                (u16::from(tile_number) * 16).wrapping_add(offset)
            };

            let (lo, hi) = self.obj_tile(tile_addr, obj);

            for xi in (0..8).rev() {
                let x = obj.x.wrapping_add(7 - xi);

                if x >= PX_WIDTH
                    || (!self.cgb_master_priority()
                        && (bg_priority[x as usize] == Priority::Bg
                            || obj.attr & SPR_BG_FIRST != 0
                                && bg_priority[x as usize] == Priority::Normal))
                {
                    continue;
                }

                let mut bit = xi;
                if obj.attr & SPR_FLIP_X != 0 {
                    bit = 7 - bit;
                }
                let bit = 1 << bit;

                let color = u8::from(hi & bit != 0) << 1 | u8::from(lo & bit != 0);

                // transparent
                if color == 0 {
                    continue;
                }

                let rgb = match self.compat_mode {
                    CompatMode::Dmg => {
                        let palette = if obj.attr & SPR_PAL == 0 {
                            self.obp0
                        } else {
                            self.obp1
                        };

                        Self::mono_rgb(shade_index(palette, color))
                    }
                    CompatMode::Compat => {
                        let palette = if obj.attr & SPR_PAL == 0 {
                            self.obp0
                        } else {
                            self.obp1
                        };

                        self.ocp.rgb(0, shade_index(palette, color))
                    }
                    CompatMode::Cgb => {
                        let cgb_palette = obj.attr & SPR_CGB_PAL;
                        self.ocp.rgb(cgb_palette, color)
                    }
                };

                self.rgba_buf.set_px(base_idx + x as usize, rgb);
            }
        }
    }
}
