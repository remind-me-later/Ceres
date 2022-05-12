pub use palette::MonochromePaletteColors;

pub mod ppu;

mod palette;
mod pixel_data;
mod scanline_renderer;
mod sprites;
mod vram;

pub const PX_WIDTH: u8 = 160;
pub const PX_HEIGHT: u8 = 144;
pub const SCANLINES_PER_FRAME: u8 = 154;

const SCREEN_PIXELS: u16 = PX_WIDTH as u16 * PX_HEIGHT as u16;

const ACCESS_OAM_CYCLES: i16 = 80; // Constant
const ACCESS_VRAM_CYCLES: i16 = 172; // Variable, minimum ammount
const HBLANK_CYCLES: i16 = 204; // Variable, maximum ammount
const VBLANK_LINE_CYCLES: i16 = 456; // Constant

pub trait VideoCallbacks {
    fn draw(&mut self, rgba_data: &[u8]);
}

#[derive(Clone, Copy, Default)]
pub struct Rgb24Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb24Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}
