//! The background and window tile fetcher.

use {
    super::{Pace, TILE_BYTES, VRAM_BANK1, model_ge_cgb_d},
    crate::{
        Model,
        ppu::{
            ATTR_BANK_B, ATTR_Y_FLIP_B, LCDC_BG_MAP_B, LCDC_TILE_SEL_B, LCDC_WIN_EN_B,
            LCDC_WIN_MAP_B, PpuCore,
        },
    },
};

/// The tile maps (0x9800 and 0x9C00 for the CPU), 32x32 tiles each.
const TILE_MAP_0: u16 = 0x1800;
const TILE_MAP_1: u16 = 0x1C00;
const TILE_MAP_WIDTH: u16 = 32;
/// Tile 0 of the area addressed with signed tile numbers (0x9000).
const TILE_DATA_SIGNED: u16 = 0x1000;

/// The steps of the fetcher, two dots each but the last (SameBoy's
/// `fetcher_step_t`).
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FetcherStep {
    #[default]
    GetTileT1,
    GetTileT2,
    DataLowT1,
    DataLowT2,
    DataHighT1,
    DataHighT2,
    Push,
}

/// How a tile data read goes while a write to LCDC's tile-select bit lands
/// (Matt Currie's research in the mealybug-tearoom-tests PPU documentation).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TileSelGlitch {
    /// No glitch: the fetcher reads the tile data from VRAM.
    ReadVram,
    /// The fetcher takes this byte instead of reading VRAM.
    Use(u8),
    /// The CGB-D reads VRAM, but from the unsigned tile area.
    CgbD,
}

impl FetcherStep {
    pub(super) const fn next(self) -> Self {
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

/// What the fetcher reads (its step is the display's pace,
/// `Background::step`).
#[derive(Clone, Default)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub(super) struct Fetcher {
    pub tile: u8,
    pub attributes: u8,
    pub data: [u8; 2],
    pub tile_index_address: u16,
    /// The tile's address in the background map, which the DMG reads instead
    /// if the window is turned off after the fetch began in it.
    pub bg_index_address: u16,
    /// The fetch began in the window.
    pub window_tile: bool,
    pub data_address: u16,
    pub last_tileset: bool,
    /// The tile row, latched on the CGB-D and newer.
    pub y: u8,
    /// What the tile-select glitch reads instead of the tile data.
    pub sel_glitch_data: u8,
    /// A write to the LCDC tile-select bit is landing.
    pub tile_sel_glitch: bool,
}

/// A tile's row as the fetcher pushes it onto the background FIFO.
#[derive(Clone, Copy)]
pub(super) struct Row {
    pub attributes: u8,
    pub data: [u8; 2],
}

impl Fetcher {
    pub(super) const fn row(&self) -> Row {
        Row {
            attributes: self.attributes,
            data: self.data,
        }
    }
}

impl PpuCore {
    pub(super) const fn fetcher_y_value(&self, pace: &Pace) -> u8 {
        // The DMG's "in window" falls with LCDC bit 5, at once: the data of a
        // tile that began in the window is read with the background's row
        // if the window is turned off meanwhile (the flag stays up until the
        // next tile).
        if pace.window.wx_triggered && (self.lcdc & LCDC_WIN_EN_B != 0 || self.is_cgb_hardware()) {
            pace.window.line
        } else {
            self.d.current_line.wrapping_add(self.scy)
        }
    }

    /// How this tile data read goes (SameBoy's `data_for_tile_sel_glitch`).
    /// On the CGB-D it moves `data_address` into the unsigned tile area.
    pub(super) fn tile_sel_glitch(&mut self) -> TileSelGlitch {
        if !self.d.fetcher.tile_sel_glitch {
            return TileSelGlitch::ReadVram;
        }
        if !self.d.fetcher.last_tileset {
            return TileSelGlitch::Use(self.d.fetcher.sel_glitch_data);
        }
        if self.model == Model::CgbD {
            self.d.fetcher.data_address &= !TILE_DATA_SIGNED;
            return TileSelGlitch::CgbD;
        }
        if self.d.fetcher.tile & 0x80 == 0 {
            TileSelGlitch::Use(self.d.fetcher.tile)
        } else {
            TileSelGlitch::ReadVram
        }
    }

    /// The address of the tile's number in the window's map or the
    /// background's, for the row `y` of it.
    fn tile_index_address(&self, pace: &Pace, window: bool, y: u8) -> u16 {
        let map = if self.lcdc & LCDC_BG_MAP_B != 0 && !window
            || self.lcdc & LCDC_WIN_MAP_B != 0 && window
        {
            TILE_MAP_1
        } else {
            TILE_MAP_0
        };
        let position = pace.bg.position;
        let x: u16 = if window {
            u16::from(pace.window.tile_x)
        } else if position.wrapping_add(16) < 8 {
            u16::from(self.scx >> 3)
        } else {
            let sub = u16::from(self.is_cgb_hardware() && !pace.fetching_object);
            ((u16::from(self.scx) + u16::from(position) + 8 - sub) / 8) & (TILE_MAP_WIDTH - 1)
        };
        map + x + u16::from(y / 8) * TILE_MAP_WIDTH
    }

    pub(super) fn tile_address(&self) -> u16 {
        let mut address = if self.d.fetcher.last_tileset {
            u16::from(self.d.fetcher.tile) * TILE_BYTES
        } else {
            TILE_DATA_SIGNED.wrapping_add_signed(
                i16::from(self.d.fetcher.tile.cast_signed()) * TILE_BYTES.cast_signed(),
            )
        };
        if self.d.fetcher.attributes & ATTR_BANK_B != 0 {
            address += VRAM_BANK1;
        }
        address
    }

    /// The fetcher's read of this dot (mode 3's loop steps it after,
    /// `Background::advance`).
    pub(super) fn read_tile(&mut self, pace: &Pace) {
        match pace.bg.step {
            FetcherStep::GetTileT1 => {
                #[cfg(feature = "debug")]
                self.trace_tile_fetch(pace.window.wx_triggered);

                let y = self.fetcher_y_value(pace);
                if model_ge_cgb_d(self.model) {
                    // Cached on CGB-D and newer, so it cannot mix tiles.
                    self.d.fetcher.y = y;
                }
                let window = pace.window.wx_triggered;
                self.d.fetcher.window_tile = window;
                self.d.fetcher.tile_index_address = self.tile_index_address(pace, window, y);
                // Where the tile would be if the window were off by the time
                // it is read (see `GetTileT2`).
                self.d.fetcher.bg_index_address = self.tile_index_address(
                    pace,
                    false,
                    self.d.current_line.wrapping_add(self.scy),
                );
            }
            FetcherStep::GetTileT2 => {
                if !pace.window.cgb_wx_glitch {
                    // The DMG picks the map at the moment of the read, and "in
                    // window" fell with LCDC bit 5: the tile comes from the
                    // background's map.
                    let address = if self.d.fetcher.window_tile
                        && self.lcdc & LCDC_WIN_EN_B == 0
                        && !self.is_cgb_hardware()
                    {
                        self.d.fetcher.bg_index_address
                    } else {
                        self.d.fetcher.tile_index_address
                    };
                    self.d.fetcher.tile = self.vram_read(address);
                    if self.is_cgb_hardware() {
                        self.d.fetcher.attributes = self.vram_read(address + VRAM_BANK1);
                    }
                }
            }
            FetcherStep::DataLowT1 | FetcherStep::DataHighT1 => {
                let y = if model_ge_cgb_d(self.model) {
                    self.d.fetcher.y
                } else {
                    self.fetcher_y_value(pace)
                };
                self.d.fetcher.last_tileset = self.lcdc & LCDC_TILE_SEL_B != 0;
                let tile_address = self.tile_address();
                let y_flip = if self.d.fetcher.attributes & ATTR_Y_FLIP_B != 0 {
                    7
                } else {
                    0
                };
                let low = pace.bg.step == FetcherStep::DataLowT1;
                self.d.fetcher.data_address =
                    tile_address + u16::from((y & 7) ^ y_flip) * 2 + u16::from(!low);
            }
            FetcherStep::DataLowT2 => {
                if pace.window.cgb_wx_glitch {
                    self.d.fetcher.data[0] = self.d.fetcher.data[1];
                } else {
                    self.d.fetcher.data[0] = match self.tile_sel_glitch() {
                        TileSelGlitch::Use(data) => data,
                        TileSelGlitch::ReadVram | TileSelGlitch::CgbD => {
                            self.vram_read(self.d.fetcher.data_address)
                        }
                    };
                    if self.d.fetcher.last_tileset && self.d.fetcher.tile_sel_glitch {
                        self.d.fetcher.sel_glitch_data =
                            self.vram_read(self.d.fetcher.data_address);
                    }
                }
            }
            FetcherStep::DataHighT2 => {
                if pace.window.cgb_wx_glitch {
                    self.d.fetcher.data[1] = self.d.fetcher.data[0];
                } else {
                    let glitch = self.tile_sel_glitch();
                    if glitch == TileSelGlitch::CgbD {
                        self.d.fetcher.data_address -= 1;
                    }
                    if let TileSelGlitch::Use(data) = glitch {
                        self.d.fetcher.data[1] = data;
                    } else {
                        let value = self.vram_read(self.d.fetcher.data_address);
                        self.d.fetcher.data[1] = value;
                        self.d.fetcher.sel_glitch_data = value;
                    }
                    if self.d.fetcher.last_tileset && self.d.fetcher.tile_sel_glitch {
                        self.d.fetcher.sel_glitch_data =
                            self.vram_read(self.d.fetcher.data_address);
                    }
                }
            }
            FetcherStep::Push => {}
        }
    }
}
