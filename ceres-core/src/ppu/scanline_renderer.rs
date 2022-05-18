use {
    super::{
        Obj, BG_PAL, BG_TO_OAM_PR, BG_X_FLIP, BG_Y_FLIP, LCDC_OBJL_B, LCDC_OBJ_B, OAM_SIZE,
        SPR_BG_FIRST, SPR_CGB_PAL, SPR_FLIP_X, SPR_FLIP_Y, SPR_PAL,
    },
    crate::{FunctionMode, Gb, PX_WIDTH},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Priority {
    Sprites,
    Bg,
    Normal,
}

fn shade_index(reg: u8, color: u8) -> u8 {
    (reg >> (color * 2)) & 0x3
}

impl Gb {
    pub(crate) fn draw_scanline(&mut self) {
        let mut bg_priority = [Priority::Normal; PX_WIDTH as usize];
        let base_idx = PX_WIDTH as usize * self.ly as usize;

        self.draw_bg(&mut bg_priority, base_idx);
        self.draw_win(&mut bg_priority, base_idx);
        self.draw_obj(&mut bg_priority, base_idx);
    }

    fn draw_bg(&mut self, bg_priority: &mut [Priority; PX_WIDTH as usize], base_idx: usize) {
        if self.bg_enabled() {
            let y = self.ly.wrapping_add(self.scy);
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in 0..PX_WIDTH {
                let x = i.wrapping_add(self.scx);
                let col = (x / 8) as u16;

                let tile_map = self.bg_tile_map() + row + col;

                let attr = match self.function_mode {
                    FunctionMode::Dmg | FunctionMode::Compat => 0,
                    FunctionMode::Cgb => self.bg_attr(tile_map),
                };

                let color = {
                    let tile_number = self.tile_number(tile_map);

                    let tile_addr = self.tile_addr(tile_number)
                        + if attr & BG_Y_FLIP == 0 {
                            line
                        } else {
                            14 - line
                        };

                    let (lo, hi) = self.bg_tile(tile_addr, attr);

                    let color_bit = 1
                        << if attr & BG_X_FLIP == 0 {
                            7 - (x & 7)
                        } else {
                            x & 7
                        };

                    ((hi & color_bit != 0) as u8) << 1 | (lo & color_bit != 0) as u8
                };

                let rgb = match self.function_mode {
                    FunctionMode::Dmg => Self::get_mono_color(shade_index(self.bgp, color)),
                    FunctionMode::Compat => self
                        .bcp
                        .get_color(attr & BG_PAL, shade_index(self.bgp, color)),
                    FunctionMode::Cgb => self.bcp.get_color(attr & BG_PAL, color),
                };

                self.rgba_buf.set_px(base_idx + i as usize, rgb);

                bg_priority[i as usize] = if color == 0 {
                    Priority::Sprites
                } else if attr & BG_TO_OAM_PR != 0 {
                    Priority::Bg
                } else {
                    Priority::Normal
                };
            }
        }
    }

    fn draw_win(&mut self, bg_priority: &mut [Priority; PX_WIDTH as usize], base_idx: usize) {
        if self.win_enabled() && self.wy <= self.ly {
            let wx = self.wx.saturating_sub(7);
            let y = ((self.ly - self.wy) as u16).wrapping_sub(self.ppu_win_skipped) as u8;
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in wx..PX_WIDTH {
                self.ppu_win_in_frame = true;
                self.ppu_win_in_ly = true;

                let x = i.wrapping_sub(wx);
                let col = (x / 8) as u16;

                let tile_map = self.win_tile_map() + row + col;

                let attr = match self.function_mode {
                    FunctionMode::Dmg | FunctionMode::Compat => 0,
                    FunctionMode::Cgb => self.bg_attr(tile_map),
                };

                let color = {
                    let tile_number = self.tile_number(tile_map);

                    let tile_addr = self.tile_addr(tile_number)
                        + if attr & BG_Y_FLIP == 0 {
                            line
                        } else {
                            14 - line
                        };

                    let (lo, hi) = self.bg_tile(tile_addr, attr);
                    let color_bit = 1
                        << if attr & BG_X_FLIP == 0 {
                            7 - (x % 8)
                        } else {
                            x % 8
                        };

                    ((hi & color_bit != 0) as u8) << 1 | (lo & color_bit != 0) as u8
                };

                let rgb = match self.function_mode {
                    FunctionMode::Dmg => Self::get_mono_color(shade_index(self.bgp, color)),
                    FunctionMode::Compat => self
                        .bcp
                        .get_color(attr & BG_PAL, shade_index(self.bgp, color)),
                    FunctionMode::Cgb => self.bcp.get_color(attr & BG_PAL, color),
                };

                bg_priority[i as usize] = if color == 0 {
                    Priority::Sprites
                } else if attr & BG_TO_OAM_PR != 0 {
                    Priority::Bg
                } else {
                    Priority::Normal
                };

                self.rgba_buf.set_px(base_idx + i as usize, rgb);
            }
        }

        if self.ppu_win_in_frame && !self.ppu_win_in_ly {
            self.ppu_win_skipped += 1;
        }
    }

    fn objs_in_ly(&mut self, height: u8, function_mode: FunctionMode) -> ([Obj; 10], usize) {
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

        match function_mode {
            FunctionMode::Cgb => {
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

    fn draw_obj(&mut self, bg_priority: &mut [Priority; PX_WIDTH as usize], base_idx: usize) {
        if self.lcdc & LCDC_OBJ_B != 0 {
            let large = self.lcdc & LCDC_OBJL_B != 0;
            let height = 8 * (large as u8 + 1);

            let (objs, len) = self.objs_in_ly(height, self.function_mode);

            for obj in objs.iter().take(len) {
                let tile_addr = {
                    let tile_number = if large {
                        obj.tile_index & !1
                    } else {
                        obj.tile_index
                    };

                    let offset = if obj.attr & SPR_FLIP_Y == 0 {
                        self.ly.wrapping_sub(obj.y) as u16 * 2
                    } else {
                        (height as u16 - 1).wrapping_sub((self.ly.wrapping_sub(obj.y)) as u16) * 2
                    };

                    (tile_number as u16 * 16).wrapping_add(offset)
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

                    let color = {
                        let color_bit = 1
                            << if obj.attr & SPR_FLIP_X == 0 {
                                xi
                            } else {
                                7 - xi
                            };

                        (((hi & color_bit != 0) as u8) << 1) | (lo & color_bit != 0) as u8
                    };

                    // transparent
                    if color == 0 {
                        continue;
                    }

                    let rgb = match self.function_mode {
                        FunctionMode::Dmg => {
                            let palette = if obj.attr & SPR_PAL == 0 {
                                self.obp0
                            } else {
                                self.obp1
                            };

                            Self::get_mono_color(shade_index(palette, color))
                        }
                        FunctionMode::Compat => {
                            let palette = if obj.attr & SPR_PAL == 0 {
                                self.obp0
                            } else {
                                self.obp1
                            };

                            self.ocp.get_color(0, shade_index(palette, color))
                        }
                        FunctionMode::Cgb => {
                            let cgb_palette = obj.attr & SPR_CGB_PAL;
                            self.ocp.get_color(cgb_palette, color)
                        }
                    };

                    self.rgba_buf.set_px(base_idx + x as usize, rgb);
                }
            }
        }
    }
}
