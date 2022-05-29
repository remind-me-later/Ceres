use {
    core::cmp::min,
    sdl2::{
        rect::{Point, Rect},
        render::{Canvas, Texture},
        video::Window,
        Sdl,
    },
    std::time::Instant,
};

const MUL: u32 = 4;
const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;

pub struct Renderer {
    canvas: Canvas<Window>,
    texture: Texture,
    render_rect: Rect,
    next_frame: Instant,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Self {
        let video_subsystem = sdl.video().unwrap();

        let window = video_subsystem
            .window(crate::CERES_STR, PX_WIDTH * MUL, PX_HEIGHT * MUL)
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let canvas = window
            .into_canvas()
            .present_vsync()
            .accelerated()
            .build()
            .unwrap();
        let texture = canvas
            .texture_creator()
            .create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGBA32, PX_WIDTH, PX_HEIGHT)
            .unwrap();

        let render_rect = Self::resize_rect(PX_WIDTH * MUL, PX_HEIGHT * MUL);

        Self {
            canvas,
            texture,
            render_rect,
            next_frame: Instant::now(),
        }
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        self.render_rect = Self::resize_rect(width, height);
    }

    fn resize_rect(win_width: u32, win_height: u32) -> Rect {
        let multiplier = min(win_width / PX_WIDTH, win_height / PX_HEIGHT);
        let width = PX_WIDTH * multiplier;
        let height = PX_HEIGHT * multiplier;
        let center = Point::new((win_width / 2) as i32, (win_height / 2) as i32);
        Rect::from_center(center, width, height)
    }

    pub fn draw_frame(&mut self, rgba: &[u8]) {
        self.texture
            .with_lock(None, |t, _| t.copy_from_slice(rgba))
            .unwrap();

        let now = Instant::now();
        if now < self.next_frame {
            std::thread::sleep(self.next_frame - now);
        }

        self.canvas.clear();
        self.canvas
            .copy(&self.texture, None, self.render_rect)
            .unwrap();
        self.canvas.present();

        self.next_frame += ceres_core::FRAME_DUR;
    }
}
