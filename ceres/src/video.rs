use {
    ceres_core::VideoCallbacks,
    sdl2::{
        rect::{Point, Rect},
        render::{Canvas, Texture, TextureCreator},
        video::{Window, WindowContext},
        VideoSubsystem,
    },
    std::time::Instant,
};

pub struct Renderer<const W: u32, const H: u32, const MUL: u32> {
    canvas: Canvas<Window>,
    _texture_creator: TextureCreator<WindowContext>,
    texture: Texture,
    render_rect: Rect,
    next_frame: Instant,
}

impl<'a, const W: u32, const H: u32, const MUL: u32> Renderer<W, H, MUL> {
    pub fn new(title: &str, video_subsystem: &'a VideoSubsystem) -> Self {
        let window = video_subsystem
            .window(title, W * MUL, H * MUL)
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let canvas = window.into_canvas().build().unwrap();

        let texture_creator = canvas.texture_creator();

        let texture = texture_creator
            .create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGBA32, W, H)
            .unwrap();

        let render_rect = Self::resize_texture(W * MUL, H * MUL);

        Self {
            canvas,
            _texture_creator: texture_creator,
            texture,
            render_rect,
            next_frame: Instant::now(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.render_rect = Self::resize_texture(width, height);
    }

    fn resize_texture(width: u32, height: u32) -> Rect {
        let multiplier = core::cmp::min(width / W, height / H);
        let surface_width = W * multiplier;
        let surface_height = H * multiplier;
        let center = Point::new(width as i32 / 2, height as i32 / 2);

        Rect::from_center(center, surface_width, surface_height)
    }
}

impl<const W: u32, const H: u32, const MUL: u32> VideoCallbacks for Renderer<W, H, MUL> {
    fn draw(&mut self, rgba_data: &[u8]) {
        self.texture
            .with_lock(None, move |buf, _pitch| {
                buf[..(W as usize * H as usize * 4)]
                    .copy_from_slice(&rgba_data[..(W as usize * H as usize * 4)]);
            })
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

        self.next_frame += ceres_core::FRAME_DURATION;
    }
}
