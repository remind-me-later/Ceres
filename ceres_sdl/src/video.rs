use {
    core::cmp::min,
    sdl2::{
        rect::{Point, Rect},
        render::{Canvas, Texture, TextureCreator},
        video::{Window, WindowContext},
        VideoSubsystem,
    },
    std::{ptr, time::Instant},
};

const MUL: u32 = 4;
const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;

static mut VREND: *mut Renderer = ptr::null_mut();

pub struct Renderer {
    canvas: Canvas<Window>,
    _texture_creator: TextureCreator<WindowContext>,
    texture: Texture,
    render_rect: Rect,
    next_frame: Instant,
}

impl Renderer {
    pub fn new(title: &str, video_subsystem: &VideoSubsystem) -> Box<Self> {
        let window = video_subsystem
            .window(title, PX_WIDTH * MUL, PX_HEIGHT * MUL)
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let canvas = window.into_canvas().build().unwrap();

        let texture_creator = canvas.texture_creator();

        let texture = texture_creator
            .create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGBA32, PX_WIDTH, PX_HEIGHT)
            .unwrap();

        let render_rect = Self::resize_texture(PX_WIDTH * MUL, PX_HEIGHT * MUL);

        let mut rend = Box::new(Self {
            canvas,
            _texture_creator: texture_creator,
            texture,
            render_rect,
            next_frame: Instant::now(),
        });

        unsafe {
            VREND = rend.as_mut();
        }

        rend
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.render_rect = Self::resize_texture(width, height);
    }

    fn resize_texture(width: u32, height: u32) -> Rect {
        let multiplier = min(width / PX_WIDTH, height / PX_HEIGHT);
        let surface_width = PX_WIDTH * multiplier;
        let surface_height = PX_HEIGHT * multiplier;
        let center = Point::new(width as i32 / 2, height as i32 / 2);

        Rect::from_center(center, surface_width, surface_height)
    }
}

pub fn ppu_frame_callback(rgba_data: &[u8]) {
    unsafe {
        (*VREND)
            .texture
            .with_lock(None, move |buf, _pitch| {
                buf[..(PX_WIDTH as usize * PX_HEIGHT as usize * 4)]
                    .copy_from_slice(&rgba_data[..(PX_WIDTH as usize * PX_HEIGHT as usize * 4)]);
            })
            .unwrap();

        let now = Instant::now();

        if now < (*VREND).next_frame {
            std::thread::sleep((*VREND).next_frame - now);
        }

        (*VREND).canvas.clear();
        (*VREND)
            .canvas
            .copy(&(*VREND).texture, None, (*VREND).render_rect)
            .unwrap();
        (*VREND).canvas.present();

        (*VREND).next_frame += ceres_core::FRAME_DUR;
    }
}
