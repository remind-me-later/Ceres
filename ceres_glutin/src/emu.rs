use {
    crate::{audio, video},
    ceres_core::{Gb, Model, Sample},
    sdl2::{
        controller::GameController,
        event::{Event, WindowEvent},
        keyboard::Scancode,
        EventPump, Sdl,
    },
    std::{
        fs::File,
        io::{Read, Write},
        mem::MaybeUninit,
        path::{Path, PathBuf},
        time::Instant,
    },
};

static mut EMU: MaybeUninit<Emu> = MaybeUninit::uninit();

pub struct Emu<'a> {
    sdl: Sdl,
    events: EventPump,
    gb: &'a mut Gb,
    has_focus: bool,
    sav_path: PathBuf,
    video: video::Renderer,
    audio: audio::Renderer,
    last_frame: Instant,
    quit: bool,
}

impl<'a> Emu<'a> {
    /// # Panics
    ///
    /// Will panic on invalid rom or ram file
    #[must_use]
    pub fn init(model: Model, mut rom_path: PathBuf) -> &'static mut Self {
        fn read_file_into(path: &Path, buf: &mut [u8]) -> Result<(), std::io::Error> {
            let mut f = File::open(path)?;
            let _ = f.read(buf).unwrap();
            Ok(())
        }

        let sdl = sdl2::init().unwrap();

        // initialize cartridge
        read_file_into(&rom_path, Gb::cartridge_rom_mut()).unwrap();

        rom_path.set_extension("sav");
        let sav_path = rom_path;

        read_file_into(&sav_path, Gb::cartridge_ram_mut()).ok();

        let audio = audio::Renderer::new(&sdl);
        let video = video::Renderer::new(&sdl);
        let events = sdl.event_pump().unwrap();

        let gb = Gb::new(model, apu_frame_callback, audio.sample_rate()).unwrap();

        let res = Self {
            sdl,
            events,
            gb,
            has_focus: false,
            sav_path,
            video,
            audio,
            last_frame: Instant::now(),
            quit: false,
        };

        let _controller = res.init_controller();

        unsafe {
            EMU.write(res);
            EMU.assume_init_mut()
        }
    }

    #[inline]
    pub fn run(&mut self) {
        let emu = unsafe { EMU.assume_init_mut() };

        while !self.quit {
            emu.handle_events();

            self.gb.run_frame();

            let elapsed = emu.last_frame.elapsed();
            if elapsed < ceres_core::FRAME_DUR {
                std::thread::sleep(ceres_core::FRAME_DUR - elapsed);
                emu.last_frame = Instant::now();
            }

            let rgba = self.gb.pixel_data();

            emu.video.draw_frame(rgba);
        }

        // save
        if Gb::cartridge_has_battery() {
            let mut f = File::create(self.sav_path.clone()).unwrap();
            f.write_all(Gb::cartridge_ram()).unwrap();
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

#[inline]
fn apu_frame_callback(l: Sample, r: Sample) {
    let emu = unsafe { EMU.assume_init_mut() };
    emu.audio.push_frame(l, r);
}
