use {
    crate::{audio::AudioRenderer, video::VideoRenderer},
    ceres_core::{Cartridge, Gb, Model, PX_HEIGHT, PX_WIDTH},
    sdl2::{
        controller::GameController,
        event::{Event, WindowEvent},
        keyboard::Scancode,
        Sdl,
    },
    std::{
        cell::RefCell,
        fs::{self, File},
        io::{Read, Write},
        path::{Path, PathBuf},
        rc::Rc,
    },
};

pub struct Emulator {
    sdl: Sdl,
    gb: Gb,
    is_focused: bool,
    sav_path: PathBuf,
    emu_win: Rc<RefCell<VideoRenderer<{ PX_WIDTH as u32 }, { PX_HEIGHT as u32 }, 4>>>,
}

impl Emulator {
    pub fn new(model: Model, rom_path: &Path) -> Self {
        let sdl = sdl2::init().unwrap();

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

        let audio_renderer = Rc::new(RefCell::new(AudioRenderer::new(&sdl)));
        let audio_callbacks = Rc::clone(&audio_renderer);

        let video_subsystem = sdl.video().unwrap();

        let emu_win: Rc<RefCell<VideoRenderer<{ PX_WIDTH as u32 }, { PX_HEIGHT as u32 }, 4>>> =
            Rc::new(RefCell::new(VideoRenderer::new(
                super::CERES_STR,
                &video_subsystem,
            )));

        let video_callbacks = Rc::clone(&emu_win);

        let gameboy = Gb::new(model, cartridge, audio_callbacks, video_callbacks);

        Self {
            sdl,
            gb: gameboy,
            is_focused: false,
            sav_path,
            emu_win,
        }
    }

    fn init_controller(&self) -> Option<GameController> {
        let game_controller_subsystem = self.sdl.game_controller().unwrap();

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

        let mut event_pump = self.sdl.event_pump().unwrap();

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

            self.gb.run_frame();
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
