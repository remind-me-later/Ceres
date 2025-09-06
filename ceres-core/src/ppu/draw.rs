use {
    super::{
        LCDC_BG_AREA, LCDC_BG_B, LCDC_BG_SIGNED, LCDC_OBJ_B, LCDC_OBJL_B, LCDC_WIN_AREA,
        LCDC_WIN_B, Ppu,
    },
    crate::{CgbMode, PX_WIDTH},
};

const BG_PAL_B: u8 = 0x7; // BG attribute bits
const BG_VBK_B: u8 = 0x8;
const BG_X_FLIP_B: u8 = 0x20;
const BG_Y_FLIP_B: u8 = 0x40;
const BG_PR_B: u8 = 0x80;

const SPR_CGB_PAL: u8 = 0x7; // Sprite attribute bits
const SPR_TILE_BANK: u8 = 0x8;
const SPR_PAL: u8 = 0x10;
const SPR_FLIP_X: u8 = 0x20;
const SPR_FLIP_Y: u8 = 0x40;
const SPR_BG_FIRST: u8 = 0x80;

#[derive(Clone, Copy)]
enum PxPrio {
    Bg,
    Normal,
    Sprites,
}

impl Ppu {
    #[must_use]
    fn bg_tile(&self, tile_addr: u16, attr: u8) -> (u8, u8) {
        let bank = u8::from(attr & BG_VBK_B != 0);
        let lo = self.vram.vram_at_bank(tile_addr, bank);
        let hi = self.vram.vram_at_bank(tile_addr + 1, bank);
        (lo, hi)
    }

    #[must_use]
    fn bg_tile_map(&self) -> u16 {
        0x9800 | (u16::from(self.lcdc & LCDC_BG_AREA != 0) << 10)
    }

    fn draw_bg(
        &mut self,
        bg_priority: &mut [PxPrio; PX_WIDTH as usize],
        base_idx: u32,
        cgb_mode: CgbMode,
    ) {
        if !self.is_bg_enabled(cgb_mode) {
            return;
        }

        let y = self.ly.wrapping_add(self.scy);
        let row = u16::from(y / 8) * 32;
        let line = u16::from((y & 7) * 2);

        for i in 0..PX_WIDTH {
            let x = i.wrapping_add(self.scx);
            let col = u16::from(x / 8);

            let tile_map = self.bg_tile_map() + row + col;

            let attr = match cgb_mode {
                CgbMode::Dmg | CgbMode::Compat => 0,
                CgbMode::Cgb => self.vram.vram_at_bank(tile_map, 1),
            };

            let color = {
                let tile_num = self.vram.vram_at_bank(tile_map, 0);

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

                (u8::from(hi & bit != 0) << 1) | u8::from(lo & bit != 0)
            };

            let rgb = match cgb_mode {
                CgbMode::Dmg => Self::mono_rgb(shade_index(self.bgp, color)),
                CgbMode::Compat => self.bcp.rgb(
                    attr & BG_PAL_B,
                    shade_index(self.bgp, color),
                    self.color_correction_mode,
                ),
                CgbMode::Cgb => self
                    .bcp
                    .rgb(attr & BG_PAL_B, color, self.color_correction_mode),
            };

            self.rgb_buf.set_px(base_idx + u32::from(i), rgb);

            bg_priority[i as usize] = if color == 0 {
                PxPrio::Sprites
            } else if attr & BG_PR_B != 0 {
                PxPrio::Bg
            } else {
                PxPrio::Normal
            };
        }
    }

    fn draw_obj(
        &mut self,
        bg_priority: &[PxPrio; PX_WIDTH as usize],
        base_idx: u32,
        cgb_mode: CgbMode,
    ) {
        if self.lcdc & LCDC_OBJ_B == 0 {
            return;
        }

        let large = self.lcdc & LCDC_OBJL_B != 0;
        let height = 8 * (u8::from(large) + 1);

        let (objs, len) = self.objs_in_ly(height, cgb_mode);

        for obj in objs.iter().take(len as usize) {
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
                    || (!self.is_cgb_master_priority(cgb_mode)
                        && (matches!(bg_priority[x as usize], PxPrio::Bg)
                            || obj.attr & SPR_BG_FIRST != 0
                                && matches!(bg_priority[x as usize], PxPrio::Normal)))
                {
                    continue;
                }

                let mut bit = xi;
                if obj.attr & SPR_FLIP_X != 0 {
                    bit = 7 - bit;
                }
                let bit = 1 << bit;

                let color = (u8::from(hi & bit != 0) << 1) | u8::from(lo & bit != 0);

                // transparent
                if color == 0 {
                    continue;
                }

                let rgb = match cgb_mode {
                    CgbMode::Dmg => {
                        let palette = if obj.attr & SPR_PAL == 0 {
                            self.obp0
                        } else {
                            self.obp1
                        };

                        Self::mono_rgb(shade_index(palette, color))
                    }
                    CgbMode::Compat => {
                        let palette = if obj.attr & SPR_PAL == 0 {
                            self.obp0
                        } else {
                            self.obp1
                        };

                        self.ocp
                            .rgb(0, shade_index(palette, color), self.color_correction_mode)
                    }
                    CgbMode::Cgb => {
                        let cgb_palette = obj.attr & SPR_CGB_PAL;
                        self.ocp.rgb(cgb_palette, color, self.color_correction_mode)
                    }
                };

                self.rgb_buf.set_px(base_idx + u32::from(x), rgb);
            }
        }
    }

    pub fn draw_scanline(&mut self, cgb_mode: CgbMode) {
        let mut bg_priority = [PxPrio::Normal; PX_WIDTH as usize];
        let base_idx = u32::from(PX_WIDTH) * u32::from(self.ly);

        self.draw_bg(&mut bg_priority, base_idx, cgb_mode);
        self.draw_win(&mut bg_priority, base_idx, cgb_mode);
        self.draw_obj(&bg_priority, base_idx, cgb_mode);
    }

    fn draw_win(
        &mut self,
        bg_priority: &mut [PxPrio; PX_WIDTH as usize],
        base_idx: u32,
        cgb_mode: CgbMode,
    ) {
        // not so sure about last condition...
        if !(self.is_window_enabled(cgb_mode) && self.wy <= self.ly && self.wx < PX_WIDTH) {
            if self.win_in_frame {
                self.win_skipped += 1;
            }
            return;
        }

        let wx = self.wx.saturating_sub(7);
        let y = (self.ly - self.wy).wrapping_sub(self.win_skipped);
        let row = u16::from(y / 8) * 32;
        let line = u16::from((y & 7) * 2);

        for i in wx..PX_WIDTH {
            self.win_in_frame = true;
            self.win_in_ly = true;

            let x = i.wrapping_sub(wx);
            let col = u16::from(x / 8);

            let tile_map = self.win_tile_map() + row + col;

            let attr = match cgb_mode {
                CgbMode::Dmg | CgbMode::Compat => 0,
                CgbMode::Cgb => self.vram.vram_at_bank(tile_map, 1),
            };

            let color = {
                let tile_num = self.vram.vram_at_bank(tile_map, 0);

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

                (u8::from(hi & bit != 0) << 1) | u8::from(lo & bit != 0)
            };

            let rgb = match cgb_mode {
                CgbMode::Dmg => Self::mono_rgb(shade_index(self.bgp, color)),
                CgbMode::Compat => self.bcp.rgb(
                    attr & BG_PAL_B,
                    shade_index(self.bgp, color),
                    self.color_correction_mode,
                ),
                CgbMode::Cgb => self
                    .bcp
                    .rgb(attr & BG_PAL_B, color, self.color_correction_mode),
            };

            bg_priority[i as usize] = if color == 0 {
                PxPrio::Sprites
            } else if attr & BG_PR_B != 0 {
                PxPrio::Bg
            } else {
                PxPrio::Normal
            };

            self.rgb_buf.set_px(base_idx + u32::from(i), rgb);
        }
    }

    #[must_use]
    const fn is_bg_enabled(&self, cgb_mode: CgbMode) -> bool {
        match cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => self.lcdc & LCDC_BG_B != 0,
            CgbMode::Cgb => true,
        }
    }

    #[must_use]
    const fn is_cgb_master_priority(&self, cgb_mode: CgbMode) -> bool {
        match cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => false,
            CgbMode::Cgb => self.lcdc & LCDC_BG_B == 0,
        }
    }

    #[must_use]
    const fn is_window_enabled(&self, cgb_mode: CgbMode) -> bool {
        match cgb_mode {
            CgbMode::Dmg | CgbMode::Compat => {
                self.lcdc & (LCDC_BG_B | LCDC_WIN_B) == (LCDC_BG_B | LCDC_WIN_B)
            }
            CgbMode::Cgb => self.lcdc & LCDC_WIN_B != 0,
        }
    }

    #[must_use]
    const fn mono_rgb(index: u8) -> (u8, u8, u8) {
        super::color_palette::GRAYSCALE_PALETTE[index as usize]
    }

    #[must_use]
    fn obj_tile(&self, tile_addr: u16, obj: &Obj) -> (u8, u8) {
        let bank = u8::from(obj.attr & SPR_TILE_BANK != 0);
        let lo = self.vram.vram_at_bank(tile_addr, bank);
        let hi = self.vram.vram_at_bank(tile_addr + 1, bank);
        (lo, hi)
    }

    #[must_use]
    fn objs_in_ly(&self, height: u8, cgb_mode: CgbMode) -> ([Obj; 10], u8) {
        let mut len: u8 = 0;
        let mut obj: [Obj; 10] = Default::default();
        let oam_bytes = self.oam.bytes();

        for chunk in oam_bytes.chunks_exact(4) {
            let y = chunk[0].wrapping_sub(16);

            if self.ly.wrapping_sub(y) < height {
                let attr = Obj {
                    y,
                    x: chunk[1].wrapping_sub(8),
                    tile_index: chunk[2],
                    attr: chunk[3],
                };

                obj[len as usize] = attr;
                len += 1;

                if len == 10 {
                    break;
                }
            }
        }

        obj[..(len as usize)].reverse();

        if matches!(cgb_mode, CgbMode::Dmg) || self.opri {
            obj[..(len as usize)].sort_by(|a, b| b.x.cmp(&a.x));
        }

        (obj, len)
    }

    #[must_use]
    fn tile_addr(&self, tile_num: u8) -> u16 {
        let signed = self.lcdc & LCDC_BG_SIGNED == 0;
        let base = 0x8000 | (u16::from(signed) << 11);

        let offset = if signed {
            #[expect(clippy::cast_possible_wrap)]
            let tile_num = tile_num as i8;
            #[expect(clippy::cast_sign_loss)]
            let tile_num = (i16::from(tile_num) + 0x80) as u16;
            tile_num
        } else {
            u16::from(tile_num)
        };

        base + offset * 16
    }

    #[must_use]
    fn win_tile_map(&self) -> u16 {
        0x9800 | (u16::from(self.lcdc & LCDC_WIN_AREA != 0) << 10)
    }
}

#[derive(Default)]
struct Obj {
    attr: u8,
    tile_index: u8,
    x: u8,
    y: u8,
}

const fn shade_index(palette: u8, color: u8) -> u8 {
    (palette >> (color * 2)) & 0x3
}
