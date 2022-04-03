use crate::{
    memory::FunctionMode,
    video::ppu::{BgAttributes, Ppu},
};

use super::{BgFifo, pixel::Pixel};

pub enum FetcherStep {
    GetTile1,
    GetTile2 {
        tile_map_address: u16,
    },
    GetTileDataLow1 {
        tile_number: u8,
        background_attributes: BgAttributes,
    },
    GetTileDataLow2 {
        tile_number: u8,
        tile_data_address: u16,
        background_attributes: BgAttributes,
    },
    GetTileDataHigh1 {
        tile_number: u8,
        background_attributes: BgAttributes,
        data_low: u8,
    },
    GetTileDataHigh2 {
        tile_data_address: u16,
        background_attributes: BgAttributes,
        data_low: u8,
    },
    Push {
        background_attributes: BgAttributes,
        data_low: u8,
        data_high: u8,
    },
    Sleep1 {
        background_attributes: BgAttributes,
        data_low: u8,
        data_high: u8,
    },
    Sleep2 {
        background_attributes: BgAttributes,
        data_low: u8,
        data_high: u8,
    },
}

impl Default for FetcherStep {
    fn default() -> Self {
        FetcherStep::GetTile1
    }
}

pub struct Fetcher {
    step: FetcherStep,
    scanline_x: u8,
    x: u8,
    y: u8,
    rendering_window: bool,
}

impl Fetcher {
    pub fn new_line(&mut self) {
        self.rendering_window = false;
    }

    pub fn tick(
        &mut self,
        ppu: &mut Ppu,
        function_mode: FunctionMode,
        bg_fifo: &mut BgFifo,
        mode_3_len: &mut u16,
    ) {
        let lcdc = ppu.registers.lcdc();
        let ly = ppu.registers.ly();
        let wy = ppu.registers.wy();
        let wx = ppu.registers.wx().saturating_sub(7);

        match self.step {
            FetcherStep::GetTile1 => {
                let tile_map_address;

                if lcdc.window_enabled(function_mode) && wy <= ly && self.scanline_x >= wx {
                    // pixel inside window
                    if !self.rendering_window {
                        let scx = ppu.registers.scx();

                        if scx & 7 > 0 && wx == 0 {
                            *mode_3_len -= 1;
                        }

                        self.rendering_window = true;
                    }
                    tile_map_address = lcdc.window_tile_map_address();
                    self.x = self.scanline_x - wx;
                    self.y = ly - wy;
                } else {
                    let scx = ppu.registers.scx();
                    let scy = ppu.registers.scy();

                    tile_map_address = lcdc.bg_tile_map_address();
                    self.x = ((scx / 8) + self.x) & 0x1f;
                    self.y = ly.wrapping_add(scy);
                }

                self.step = FetcherStep::GetTile2 { tile_map_address }
            }
            FetcherStep::GetTile2 { tile_map_address } => {
                let tile_num_address = tile_map_address + self.x as u16 + self.y as u16;
                let tile_number = ppu.vram.tile_number(tile_num_address);
                let background_attributes = match function_mode {
                    FunctionMode::Monochrome => BgAttributes::empty(),
                    FunctionMode::Color | FunctionMode::Compatibility => {
                        ppu.vram.background_attributes(tile_num_address)
                    }
                };

                self.step = FetcherStep::GetTileDataLow1 {
                    tile_number,
                    background_attributes,
                };
            }

            FetcherStep::GetTileDataLow1 {
                tile_number,
                background_attributes,
            } => {
                let line = ((self.y % 8) * 2) as u16;

                let tile_data_address = if background_attributes.contains(BgAttributes::Y_FLIP) {
                    lcdc.tile_data_address(tile_number) + 14 - line
                } else {
                    lcdc.tile_data_address(tile_number) + line
                };

                self.step = FetcherStep::GetTileDataLow2 {
                    tile_number,
                    tile_data_address,
                    background_attributes,
                };
            }
            FetcherStep::GetTileDataLow2 {
                tile_number,
                tile_data_address,
                background_attributes,
            } => {
                let data_low = ppu.vram.get_bank(
                    tile_data_address - 0x8000,
                    background_attributes
                        .contains(BgAttributes::VRAM_BANK_NUMBER)
                        .into(),
                );
                self.step = FetcherStep::GetTileDataHigh1 {
                    tile_number,

                    background_attributes,
                    data_low,
                };
            }
            FetcherStep::GetTileDataHigh1 {
                tile_number,
                background_attributes,
                data_low,
            } => {
                let line = ((self.y % 8) * 2) as u16;

                let tile_data_address = if background_attributes.contains(BgAttributes::Y_FLIP) {
                    lcdc.tile_data_address(tile_number) + 14 - line
                } else {
                    lcdc.tile_data_address(tile_number) + line
                };

                self.step = FetcherStep::GetTileDataHigh2 {
                    tile_data_address,
                    background_attributes,
                    data_low,
                };
            }
            FetcherStep::GetTileDataHigh2 {
                tile_data_address,
                background_attributes,
                data_low,
            } => {
                let data_high = ppu.vram.get_bank(
                    tile_data_address - 0x8000 + 1,
                    background_attributes
                        .contains(BgAttributes::VRAM_BANK_NUMBER)
                        .into(),
                );
                self.step = FetcherStep::Sleep1 {
                    background_attributes,
                    data_low,
                    data_high,
                };
            }
            FetcherStep::Sleep1 {
                background_attributes,
                data_low,
                data_high,
            } => {
                self.step = FetcherStep::Sleep2 {
                    background_attributes,
                    data_low,
                    data_high,
                }
            }
            FetcherStep::Sleep2 {
                background_attributes,
                data_low,
                data_high,
            } => {
                self.step = FetcherStep::Push {
                    background_attributes,
                    data_low,
                    data_high,
                }
            }
            FetcherStep::Push {
                background_attributes,
                data_low,
                data_high,
            } => {
                if !bg_fifo.is_empty() {
                    return;
                }

                for i in 0..8 {
                    let color_bit = 1
                        << if background_attributes.contains(BgAttributes::X_FLIP) {
                            (self.x + i) % 8
                        } else {
                            7 - ((self.x + i) % 8)
                        };
                    let color = (((data_high & color_bit != 0) as u8) << 1)
                        | (data_low & color_bit != 0) as u8;
                    let palette = background_attributes.bits() & 0x7;
                    let obj_to_bg_priority =
                        background_attributes.contains(BgAttributes::BG_TO_OAM_PR) as u8;

                    let pixel = Pixel {
                        color,
                        palette,
                        obj_to_bg_priority,
                    };

                    bg_fifo.push_pixel(pixel);
                }

                self.step = FetcherStep::GetTile1;
            }
        }
    }
}
