//! The background and window tile fetcher.

use {
    super::{fifo::Item, model_ge_cgb_d},
    crate::{
        Model,
        ppu::{LCDC_BG_MAP_B, LCDC_TILE_SEL_B, LCDC_WIN_EN_B, LCDC_WIN_MAP_B, Ppu},
    },
};

/// The steps of the fetcher, two dots each but the last (SameBoy's
/// `fetcher_step_t`).
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum FetcherStep {
    #[default]
    GetTileT1,
    GetTileT2,
    DataLowT1,
    DataLowT2,
    DataHighT1,
    DataHighT2,
    Push,
}

impl FetcherStep {
    pub const fn next(self) -> Self {
        match self {
            Self::GetTileT1 => Self::GetTileT2,
            Self::GetTileT2 => Self::DataLowT1,
            Self::DataLowT1 => Self::DataLowT2,
            Self::DataLowT2 => Self::DataHighT1,
            Self::DataHighT1 => Self::DataHighT2,
            Self::DataHighT2 | Self::Push => Self::Push,
        }
    }
}

#[derive(Clone, Default)]
pub struct Fetcher {
    pub step: FetcherStep,
    pub tile: u8,
    pub attributes: u8,
    pub data: [u8; 2],
    pub tile_index_address: u16,
    pub data_address: u16,
    pub last_tileset: bool,
    /// The tile row, latched on the CGB-D and newer.
    pub y: u8,
    /// What the tile-select glitch reads instead of the tile data.
    pub sel_glitch_data: u8,
    /// A write to the LCDC tile-select bit is landing.
    pub tile_sel_glitch: bool,
}

impl Ppu {
    pub(super) const fn fetcher_y_value(&self) -> u8 {
        if self.d.window.wx_triggered {
            self.d.window.line
        } else {
            self.d.current_line.wrapping_add(self.scy)
        }
    }

    /// `data_for_tile_sel_glitch`: returns (data, use_glitched, cgb_d_glitch).
    pub(super) fn data_for_tile_sel_glitch(&mut self) -> (u8, bool, bool) {
        if self.d.fetcher.last_tileset {
            if self.model != Model::CgbD {
                return (self.d.fetcher.tile, self.d.fetcher.tile & 0x80 == 0, false);
            }
            self.d.fetcher.data_address &= !0x1000;
            return (0, false, true);
        }
        (self.d.fetcher.sel_glitch_data, true, false)
    }

    pub(super) fn tile_address(&self) -> u16 {
        let mut address = if self.d.fetcher.last_tileset {
            u16::from(self.d.fetcher.tile) * 0x10
        } else {
            0x1000_u16.wrapping_add_signed(i16::from(self.d.fetcher.tile.cast_signed()) * 0x10)
        };
        if self.d.fetcher.attributes & 8 != 0 {
            address += 0x2000;
        }
        address
    }

    #[expect(clippy::too_many_lines, reason = "SameBoy's fetcher: one arm per step")]
    pub(super) fn advance_fetcher(&mut self) {
        match self.d.fetcher.step {
            FetcherStep::GetTileT1 => {
                self.update_wx_glitch();
                let mut map: u16 = 0x1800;
                if self.lcdc & LCDC_WIN_EN_B == 0 {
                    self.d.window.wx_triggered = false;
                }
                if self.lcdc & LCDC_BG_MAP_B != 0 && !self.d.window.wx_triggered
                    || self.lcdc & LCDC_WIN_MAP_B != 0 && self.d.window.wx_triggered
                {
                    map = 0x1C00;
                }

                let y = self.fetcher_y_value();
                let position = self.d.position_in_line;
                let x: u16 = if self.d.window.wx_triggered {
                    u16::from(self.d.window.tile_x)
                } else if position.wrapping_add(16) < 8 {
                    u16::from(self.scx >> 3)
                } else {
                    let sub = u16::from(self.hw_cgb() && !self.d.obj_fetch.active);
                    ((u16::from(self.scx) + u16::from(position) + 8 - sub) / 8) & 0x1F
                };
                if model_ge_cgb_d(self.model) {
                    // Cached on CGB-D and newer, so it cannot mix tiles.
                    self.d.fetcher.y = y;
                }
                self.d.fetcher.tile_index_address = map + x + u16::from(y / 8) * 32;
                self.d.fetcher.step = self.d.fetcher.step.next();
            }
            FetcherStep::GetTileT2 => {
                if self.d.window.cgb_wx_glitch {
                    self.d.fetcher.step = self.d.fetcher.step.next();
                    return;
                }
                let address = self.d.fetcher.tile_index_address;
                self.d.fetcher.tile = self.vram_read(address);
                if self.hw_cgb() {
                    self.d.fetcher.attributes = self.vram_read(address + 0x2000);
                }
                self.d.fetcher.step = self.d.fetcher.step.next();
            }
            FetcherStep::DataLowT1 | FetcherStep::DataHighT1 => {
                self.update_wx_glitch();
                let y = if model_ge_cgb_d(self.model) {
                    self.d.fetcher.y
                } else {
                    self.fetcher_y_value()
                };
                self.d.fetcher.last_tileset = self.lcdc & LCDC_TILE_SEL_B != 0;
                let tile_address = self.tile_address();
                let y_flip = if self.d.fetcher.attributes & 0x40 != 0 {
                    7
                } else {
                    0
                };
                let low = self.d.fetcher.step == FetcherStep::DataLowT1;
                self.d.fetcher.data_address =
                    tile_address + u16::from((y & 7) ^ y_flip) * 2 + u16::from(!low);
                self.d.fetcher.step = self.d.fetcher.step.next();
            }
            FetcherStep::DataLowT2 => {
                if self.d.window.cgb_wx_glitch {
                    self.d.fetcher.data[0] = self.d.fetcher.data[1];
                    self.d.fetcher.step = self.d.fetcher.step.next();
                    return;
                }
                let (use_glitched, cgb_d_glitch) = if self.d.fetcher.tile_sel_glitch {
                    let (data, used, d) = self.data_for_tile_sel_glitch();
                    self.d.fetcher.data[0] = data;
                    (used, d)
                } else {
                    (false, false)
                };
                if !use_glitched {
                    self.d.fetcher.data[0] = self.vram_read(self.d.fetcher.data_address);
                }
                if self.d.fetcher.last_tileset && self.d.fetcher.tile_sel_glitch {
                    self.d.fetcher.sel_glitch_data = self.vram_read(self.d.fetcher.data_address);
                } else if cgb_d_glitch {
                    self.d.fetcher.sel_glitch_data =
                        self.vram_read(self.d.fetcher.data_address & !0x1000);
                } else {
                    // No glitch to propagate.
                }
                self.d.fetcher.step = self.d.fetcher.step.next();
            }
            FetcherStep::DataHighT2 => {
                if self.d.window.cgb_wx_glitch {
                    self.d.fetcher.data[1] = self.d.fetcher.data[0];
                    self.d.fetcher.step = self.d.fetcher.step.next();
                    if self.d.window.wx_triggered {
                        self.d.window.tile_x = (self.d.window.tile_x + 1) & 0x1F;
                    }
                    return;
                }
                let (use_glitched, cgb_d_glitch) = if self.d.fetcher.tile_sel_glitch {
                    let (data, used, d) = self.data_for_tile_sel_glitch();
                    self.d.fetcher.data[1] = data;
                    if d {
                        self.d.fetcher.data_address -= 1;
                    }
                    (used, d)
                } else {
                    (false, false)
                };
                if !use_glitched {
                    let value = self.vram_read(self.d.fetcher.data_address);
                    self.d.fetcher.data[1] = value;
                    self.d.fetcher.sel_glitch_data = value;
                }
                if self.d.fetcher.last_tileset && self.d.fetcher.tile_sel_glitch {
                    self.d.fetcher.sel_glitch_data = self.vram_read(self.d.fetcher.data_address);
                } else if cgb_d_glitch {
                    self.d.fetcher.sel_glitch_data =
                        self.vram_read((self.d.fetcher.data_address & !0x1000) + 1);
                } else {
                    // No glitch to propagate.
                }
                if self.d.window.wx_triggered {
                    self.d.window.tile_x = (self.d.window.tile_x + 1) & 0x1F;
                }
                self.fetcher_push();
            }
            FetcherStep::Push => self.fetcher_push(),
        }
    }

    pub(super) fn fetcher_push(&mut self) {
        self.d.fetcher.step = FetcherStep::Push;
        if self.d.bg_fifo.size > 0 {
            return;
        }

        if self.d.window.wy_triggered
            && self.lcdc & LCDC_WIN_EN_B == 0
            && !self.hw_cgb()
            && !self.d.window.no_pixel_insertion_glitch
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

        let attr = self.d.fetcher.attributes;
        let [low, high] = self.d.fetcher.data;
        self.d
            .bg_fifo
            .push_bg_row(low, high, attr & 7, attr & 0x80 != 0, attr & 0x20 != 0);
        self.d.fetcher.step = FetcherStep::GetTileT1;
    }
}
