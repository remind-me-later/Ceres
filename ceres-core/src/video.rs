mod palette;
mod pixel_data;
pub mod ppu;
mod rgb_color;
mod sprites;
mod vram;

pub use palette::MonochromePaletteColors;
pub use pixel_data::PixelData;

use rgb_color::RgbColor;

pub const SCREEN_WIDTH: u8 = 160;
pub const SCREEN_HEIGHT: u8 = 144;
pub const SCANLINES_PER_FRAME: u8 = 154;

const SCREEN_PIXELS: u16 = SCREEN_WIDTH as u16 * SCREEN_HEIGHT as u16;

const ACCESS_OAM_CYCLES: i16 = 80; // Constant
const ACCESS_VRAM_CYCLES: i16 = 172; // Variable, minimum ammount
const HBLANK_CYCLES: i16 = 204; // Variable, maximum ammount
const VBLANK_LINE_CYCLES: i16 = 456; // Constant
