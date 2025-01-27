// FIXME: useless duplication

const BPP: u32 = 4; // bytes per pixel
const TILE_WIDTH: u16 = 24;
const TILE_HEIGHT: u16 = 16 * 2;
const PX_PER_TILE: u16 = 8 * 8;
const TILE_TOTAL: u16 = TILE_WIDTH * TILE_HEIGHT;
const PX_TOTAL: u16 = TILE_TOTAL * PX_PER_TILE;
const RGB_BUF_SIZE: u32 = PX_TOTAL as u32 * BPP;

pub const VRAM_PX_WIDTH: u16 = TILE_WIDTH * 8;
pub const VRAM_PX_HEIGHT: u16 = TILE_HEIGHT * 8;

#[derive(Clone, Debug)]
pub(super) struct RgbaBuf {
    data: [u8; RGB_BUF_SIZE as usize],
}

impl Default for RgbaBuf {
    fn default() -> Self {
        Self {
            data: [0xff; RGB_BUF_SIZE as usize],
        }
    }
}

impl RgbaBuf {
    #[inline]
    pub(super) fn set_px(&mut self, index: u32, rgb: (u8, u8, u8)) {
        let base = index * BPP;
        self.data[base as usize] = rgb.0;
        self.data[base as usize + 1] = rgb.1;
        self.data[base as usize + 2] = rgb.2;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn pixel_data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Default, Debug)]
pub struct VramRenderer {
    rgba_buf: RgbaBuf,
}

impl VramRenderer {
    pub fn draw_vram(&mut self, vram: &[u8]) {
        for tile in 0..TILE_TOTAL as usize {
            // Each tile occupies 16 bytes
            let tile_idx = tile * 16;
            for i in 0..8 {
                for j in 0..8 {
                    let most_byte = vram[tile_idx + 2 * i];
                    let least_byte = vram[tile_idx + 2 * i + 1];

                    let most_bit = (most_byte & (1 << (7 - j))) != 0;
                    let least_bit = (least_byte & (1 << (7 - j))) != 0;
                    let color_idx = (most_bit as u8) << 1 | least_bit as u8;

                    let color = super::color_palette::GRAYSCALE_PALETTE[color_idx as usize];

                    let leftmost_px = tile as u32 % TILE_WIDTH as u32 * 8;
                    let topmost_px = tile as u32 / TILE_WIDTH as u32 * 8;
                    let px_idx =
                        (topmost_px + i as u32) * VRAM_PX_WIDTH as u32 + leftmost_px + j as u32;
                    self.rgba_buf.set_px(px_idx, color);
                }
            }
        }
    }

    #[must_use]
    pub const fn vram_data_rgba(&self) -> &[u8] {
        self.rgba_buf.pixel_data()
    }
}
