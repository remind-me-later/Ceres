use {
    crate::{audio, video},
    ceres_core::{Cartridge, Gb, Model, PX_HEIGHT, PX_WIDTH},
    sdl2::{
        controller::GameController,
        event::{Event, WindowEvent},
        keyboard::Scancode,
        Sdl,
    },
    std::{
        fs::{self, File},
        io::{Read, Write},
        path::{Path, PathBuf},
        vec::Vec,
    },
};

pub struct Emulator {
    sdl: Sdl,
    gb: Gb,
    has_focus: bool,
    sav_path: PathBuf,
    video: *mut video::Renderer<{ PX_WIDTH as u32 }, { PX_HEIGHT as u32 }, 4>,
}

impl Emulator {
    pub fn new(model: Model, rom_path: &Path) -> Self {
        let sdl = sdl2::init().unwrap();
        let video_subsystem = sdl.video().unwrap();

        let sav_path = rom_path.with_extension("sav");

        let cart = {
            let rom = read_file(rom_path).unwrap().into_boxed_slice();
            let ram = read_file(&sav_path).ok().map(Vec::into_boxed_slice);
            Cartridge::new(rom, ram).unwrap()
        };

        let audio_callbacks = Box::new(audio::Renderer::new(&sdl));
        let mut video_callbacks =
            Box::new(video::Renderer::new(super::CERES_STR, &video_subsystem));
        let video: *mut _ = video_callbacks.as_mut();

        let gb = Gb::new(model, cart, audio_callbacks, video_callbacks);

        Self {
            sdl,
            gb,
            has_focus: false,
            sav_path,
            video,
        }
    }

    fn init_controller(&self) -> Option<GameController> {
        let gcss = self.sdl.game_controller().unwrap();
        let avail = gcss.num_joysticks().unwrap();

        (0..avail).find_map(|id| {
            gcss.is_game_controller(id)
                .then(|| gcss.open(id).ok())
                .flatten()
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
                        WindowEvent::Resized(width, height) => unsafe {
                            (*self.video).resize(width as u32, height as u32);
                        },
                        WindowEvent::Close => break 'main_loop,
                        WindowEvent::FocusGained => self.has_focus = true,
                        WindowEvent::FocusLost => self.has_focus = false,
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
