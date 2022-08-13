use {
    crate::{audio, video},
    ceres_core::{Cartridge, Gb, Model},
    sdl2::{
        controller::GameController,
        event::{Event, WindowEvent},
        keyboard::Scancode,
        EventPump, Sdl,
    },
    std::{
        fs::File,
        io::{Read, Write},
        path::{Path, PathBuf},
        time::{Duration, Instant},
    },
};

pub struct Emu {
    sdl: Sdl,
    events: EventPump,
    gb: Gb<audio::Renderer>,
    has_focus: bool,
    sav_path: PathBuf,
    video: video::Renderer,
    last_frame: Instant,
    quit: bool,
}

impl Emu {
    /// # Panics
    ///
    /// Will panic on invalid rom or ram file
    #[must_use]
    pub fn new(model: Model, mut path: PathBuf) -> Self {
        fn read_file_into(path: &Path) -> Result<Box<[u8]>, std::io::Error> {
            let mut f = File::open(path)?;
            let metadata = f.metadata().unwrap();
            let len = metadata.len();
            let mut buf = vec![0; len as usize].into_boxed_slice();
            let _ = f.read(&mut buf).unwrap();
            Ok(buf)
        }

        let sdl = sdl2::init().unwrap();

        // initialize cartridge
        let rom = read_file_into(&path).unwrap();

        path.set_extension("sav");
        let ram = read_file_into(&path).ok();

        let cart = Cartridge::new(rom, ram).unwrap();

        let video = video::Renderer::new(&sdl);
        let audio = audio::Renderer::new(&sdl);
        let sample_rate = audio.sample_rate();
        let gb = Gb::new(model, audio, sample_rate, cart);

        let events = sdl.event_pump().unwrap();

        let res = Self {
            sdl,
            events,
            gb,
            has_focus: false,
            sav_path: path,
            video,
            last_frame: Instant::now() - Duration::from_secs(1),
            quit: false,
        };

        let _controller = res.init_controller();

        res
    }

    #[inline]
    pub fn run(&mut self) {
        while !self.quit {
            let elapsed = self.last_frame.elapsed();
            if elapsed >= ceres_core::FRAME_DUR - Duration::from_millis(4) {
                self.handle_events();
                self.gb.run_frame();
                self.video.draw_frame(self.gb.pixel_data_rgb());
                self.last_frame = Instant::now();
            }
        }

        // save
        if self.gb.cartridge_has_battery() {
            let mut f = File::create(self.sav_path.clone()).unwrap();
            f.write_all(self.gb.cartridge_ram()).unwrap();
        }
    }

    fn init_controller(&self) -> Option<GameController> {
        let gcs = self.sdl.game_controller().unwrap();
        let avail = gcs.num_joysticks().unwrap();

        (0..avail).find_map(|id| {
            gcs.is_game_controller(id)
                .then(|| gcs.open(id).ok())
                .flatten()
        })
    }

    fn handle_events(&mut self) {
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => self.quit = true,
                Event::Window { win_event, .. } => match win_event {
                    WindowEvent::Resized(width, height) => {
                        self.video.resize(width as u32, height as u32);
                    }
                    WindowEvent::FocusGained => self.has_focus = true,
                    WindowEvent::FocusLost => self.has_focus = false,
                    WindowEvent::Close => self.quit = true,
                    _ => (),
                },
                Event::ControllerButtonDown { button, .. } => {
                    use {ceres_core::Button, sdl2::controller};

                    if !self.has_focus {
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

                    if !self.has_focus {
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

                    if !self.has_focus {
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
                            // Gui
                            Scancode::F => self.video.toggle_fullscreen(),
                            _ => (),
                        }
                    }
                }
                Event::KeyUp { scancode, .. } => {
                    use ceres_core::Button;

                    if !self.has_focus {
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
    }
}
