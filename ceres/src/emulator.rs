use {
    super::audio::AudioRenderer,
    ceres_core::{Cartridge, Gb, VideoCallbacks, SCREEN_HEIGHT, SCREEN_WIDTH},
    sdl2::{
        controller::GameController,
        event::{Event, WindowEvent},
        keyboard::Scancode,
        rect::{Point, Rect},
        render::{Canvas, Texture, TextureCreator},
        video::{Window, WindowContext},
        Sdl, VideoSubsystem,
    },
    std::{
        cell::RefCell,
        fs::{self, File},
        io::{Read, Write},
        path::{Path, PathBuf},
        rc::Rc,
        time::Instant,
    },
};

pub struct Emulator {
    gb: Gb,
    is_focused: bool,
    is_gui_paused: bool,
    sdl_context: Sdl,
    audio_renderer: Rc<RefCell<AudioRenderer>>,
    sav_path: PathBuf,
    emu_win: Rc<RefCell<EmuWindow<{ SCREEN_WIDTH as u32 }, { SCREEN_HEIGHT as u32 }, 4>>>,
}

impl Emulator {
    pub fn new(model: ceres_core::Model, rom_path: &Path) -> Self {
        let sdl_context = sdl2::init().unwrap();

        let sav_path = rom_path.with_extension("sav");

        let (cartridge, sav_path) = {
            let rom_buf = read_file(rom_path).unwrap().into_boxed_slice();

            let ram = if let Ok(sav_buf) = read_file(&sav_path) {
                Some(sav_buf.into_boxed_slice())
            } else {
                None
            };

            let cartridge = Cartridge::new(rom_buf, ram).unwrap();

            (cartridge, sav_path)
        };

        let audio_renderer = Rc::new(RefCell::new(AudioRenderer::new(&sdl_context)));
        let audio_callbacks = Rc::clone(&audio_renderer);

        let video_subsystem = sdl_context.video().unwrap();

        let emu_win: Rc<RefCell<EmuWindow<{ SCREEN_WIDTH as u32 }, { SCREEN_HEIGHT as u32 }, 4>>> =
            Rc::new(RefCell::new(EmuWindow::new(
                super::CERES_STR,
                &video_subsystem,
            )));

        let video_callbacks = Rc::clone(&emu_win);

        let gameboy = Gb::new(model, cartridge, audio_callbacks, video_callbacks);

        Self {
            sdl_context,
            gb: gameboy,
            is_focused: false,
            is_gui_paused: false,
            audio_renderer,
            sav_path,
            emu_win,
        }
    }

    fn init_controller(&self) -> Option<GameController> {
        let game_controller_subsystem = self.sdl_context.game_controller().unwrap();

        let available = game_controller_subsystem
            .num_joysticks()
            .map_err(|e| format!("can't enumerate joysticks: {}", e))
            .unwrap();

        (0..available).find_map(|id| {
            if !game_controller_subsystem.is_game_controller(id) {
                return None;
            }

            match game_controller_subsystem.open(id) {
                Ok(c) => Some(c),
                Err(_) => None,
            }
        })
    }

    pub fn run(mut self) {
        let _controller = self.init_controller();

        let mut event_pump = self.sdl_context.event_pump().unwrap();

        'main_loop: loop {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => break 'main_loop,
                    Event::Window { win_event, .. } => match win_event {
                        WindowEvent::Resized(width, height) => {
                            self.emu_win
                                .borrow_mut()
                                .resize(width as u32, height as u32);
                        }
                        WindowEvent::Close => break 'main_loop,
                        WindowEvent::FocusGained => self.is_focused = true,
                        WindowEvent::FocusLost => self.is_focused = false,
                        _ => (),
                    },
                    Event::ControllerButtonDown { button, .. } => {
                        use {ceres_core::Button, sdl2::controller};

                        if !self.is_focused {
                            return;
                        }

                        match button {
                            controller::Button::DPadUp => self.gb.press(Button::Up),
                            controller::Button::DPadLeft => self.gb.press(Button::Left),
                            controller::Button::DPadDown => self.gb.press(Button::Down),
                            controller::Button::DPadRight => self.gb.press(Button::Right),
                            controller::Button::B => self.gb.press(Button::B),
                            controller::Button::A => self.gb.press(Button::A),
                            controller::Button::Start => self.gb.press(Button::Start),
                            controller::Button::Back => self.gb.press(Button::Select),
                            _ => (),
                        }
                    }
                    Event::ControllerButtonUp { button, .. } => {
                        use {ceres_core::Button, sdl2::controller};

                        if !self.is_focused {
                            return;
                        }

                        match button {
                            controller::Button::DPadUp => self.gb.release(Button::Up),
                            controller::Button::DPadLeft => self.gb.release(Button::Left),
                            controller::Button::DPadDown => self.gb.release(Button::Down),
                            controller::Button::DPadRight => self.gb.release(Button::Right),
                            controller::Button::B => self.gb.release(Button::B),
                            controller::Button::A => self.gb.release(Button::A),
                            controller::Button::Start => self.gb.release(Button::Start),
                            controller::Button::Back => self.gb.release(Button::Select),
                            _ => (),
                        }
                    }
                    Event::KeyDown { scancode, .. } => {
                        use ceres_core::Button;

                        if !self.is_focused {
                            return;
                        }

                        if let Some(key) = scancode {
                            match key {
                                Scancode::W => self.gb.press(Button::Up),
                                Scancode::A => self.gb.press(Button::Left),
                                Scancode::S => self.gb.press(Button::Down),
                                Scancode::D => self.gb.press(Button::Right),
                                Scancode::K => self.gb.press(Button::A),
                                Scancode::L => self.gb.press(Button::B),
                                Scancode::Return => self.gb.press(Button::Start),
                                Scancode::Backspace => self.gb.press(Button::Select),
                                Scancode::Space => {
                                    if self.is_gui_paused {
                                        self.audio_renderer.borrow_mut().play();
                                        self.is_gui_paused = false;
                                    } else {
                                        self.audio_renderer.borrow_mut().pause();
                                        self.is_gui_paused = true;
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                    Event::KeyUp { scancode, .. } => {
                        use ceres_core::Button;

                        if !self.is_focused {
                            return;
                        }

                        if let Some(key) = scancode {
                            match key {
                                Scancode::W => self.gb.release(Button::Up),
                                Scancode::A => self.gb.release(Button::Left),
                                Scancode::S => self.gb.release(Button::Down),
                                Scancode::D => self.gb.release(Button::Right),
                                Scancode::K => self.gb.release(Button::A),
                                Scancode::L => self.gb.release(Button::B),
                                Scancode::Return => self.gb.release(Button::Start),
                                Scancode::Backspace => self.gb.release(Button::Select),
                                _ => (),
                            }
                        }
                    }

                    _ => (),
                }
            }

            if !self.is_gui_paused {
                self.gb.run_frame();
            }
        }

        if let Some(cart_ram) = self.gb.save_data() {
            let mut f = File::create(self.sav_path).unwrap();
            f.write_all(cart_ram).unwrap();
        }
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, ()> {
    let mut f = File::open(path).map_err(|_| ())?;
    let metadata = fs::metadata(&path).unwrap();
    let mut buffer = vec![0; metadata.len() as usize];
    f.read_exact(&mut buffer).unwrap();

    Ok(buffer)
}

struct EmuWindow<const WIDTH: u32, const HEIGHT: u32, const MUL: u32> {
    canvas: Canvas<Window>,
    _texture_creator: TextureCreator<WindowContext>,
    texture: Texture,
    render_rect: Rect,
    next_frame: Instant,
}

impl<'a, const WIDTH: u32, const HEIGHT: u32, const MUL: u32> EmuWindow<WIDTH, HEIGHT, MUL> {
    pub fn new(title: &str, video_subsystem: &'a VideoSubsystem) -> Self {
        let window = video_subsystem
            .window(title, WIDTH * MUL, HEIGHT * MUL)
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let canvas = window.into_canvas().build().unwrap();

        let texture_creator = canvas.texture_creator();

        let texture = texture_creator
            .create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGBA32, WIDTH, HEIGHT)
            .unwrap();

        let render_rect = Self::resize_texture(WIDTH * MUL, HEIGHT * MUL);

        Self {
            canvas,
            _texture_creator: texture_creator,
            texture,
            render_rect,
            next_frame: Instant::now(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.render_rect = Self::resize_texture(width, height)
    }

    fn resize_texture(width: u32, height: u32) -> Rect {
        let multiplier = core::cmp::min(width / WIDTH, height / HEIGHT);
        let surface_width = WIDTH * multiplier;
        let surface_height = HEIGHT * multiplier;
        let center = Point::new(width as i32 / 2, height as i32 / 2);

        Rect::from_center(center, surface_width, surface_height)
    }
}

impl<const WIDTH: u32, const HEIGHT: u32, const MUL: u32> VideoCallbacks
    for EmuWindow<WIDTH, HEIGHT, MUL>
{
    fn draw(&mut self, rgba_data: &[u8]) {
        self.texture
            .with_lock(None, move |buf, _pitch| {
                buf[..(WIDTH as usize * HEIGHT as usize * 4)]
                    .copy_from_slice(&rgba_data[..(WIDTH as usize * HEIGHT as usize * 4)]);
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
