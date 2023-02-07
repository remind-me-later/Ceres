use crate::video;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use std::sync::Arc;
use std::sync::Mutex;
use {
    crate::audio,
    ceres_core::{Button, Cartridge, Gb, Model},
    std::{
        fs::File,
        io::{Read, Write},
        path::{Path, PathBuf},
    },
};

pub struct Emu {
    gb: Arc<Mutex<Gb>>,
    sav_path: PathBuf,
}

impl Emu {
    /// # Panics
    ///
    /// Will panic on invalid rom or ram file
    pub fn new(model: Model, mut path: PathBuf) -> Self {
        fn read_file_into(path: &Path) -> Result<Box<[u8]>, std::io::Error> {
            let mut f = File::open(path)?;
            let metadata = f.metadata().unwrap();
            let len = metadata.len();
            let mut buf = vec![0; len as usize].into_boxed_slice();
            let _ = f.read(&mut buf).unwrap();
            Ok(buf)
        }

        // initialize cartridge
        let rom = read_file_into(&path).unwrap();
        path.set_extension("sav");
        let ram = read_file_into(&path).ok();
        let cart = Cartridge::new(rom, ram).unwrap();

        let gb = {
            let sample_rate = audio::Renderer::sample_rate();
            Arc::new(Mutex::new(Gb::new(model, sample_rate, cart)))
        };

        let sav_path: PathBuf = path;

        Self { gb, sav_path }
    }

    pub fn run(self) {
        let sdl_context = sdl2::init().unwrap();
        let mut audio = {
            let gb = Arc::clone(&self.gb);
            audio::Renderer::new(&sdl_context, gb)
        };
        let mut video = video::Renderer::new(&sdl_context);
        let mut event_pump = sdl_context.event_pump().unwrap();
        let mut is_focused = true;

        'running: loop {
            if let Ok(mut gb) = self.gb.lock() {
                for event in event_pump.poll_iter() {
                    match event {
                        Event::Quit { .. } => break 'running,
                        Event::KeyDown {
                            keycode: Some(keycode),
                            repeat: false,
                            ..
                        } if is_focused => match keycode {
                            Keycode::W => gb.press(Button::Up),
                            Keycode::A => gb.press(Button::Left),
                            Keycode::S => gb.press(Button::Down),
                            Keycode::D => gb.press(Button::Right),
                            Keycode::K => gb.press(Button::A),
                            Keycode::L => gb.press(Button::B),
                            Keycode::Return => gb.press(Button::Start),
                            Keycode::Backspace => gb.press(Button::Select),
                            _ => (),
                        },
                        Event::KeyUp {
                            keycode: Some(keycode),
                            repeat: false,
                            ..
                        } if is_focused => match keycode {
                            Keycode::W => gb.release(Button::Up),
                            Keycode::A => gb.release(Button::Left),
                            Keycode::S => gb.release(Button::Down),
                            Keycode::D => gb.release(Button::Right),
                            Keycode::K => gb.release(Button::A),
                            Keycode::L => gb.release(Button::B),
                            Keycode::Return => gb.release(Button::Start),
                            Keycode::Backspace => gb.release(Button::Select),
                            _ => (),
                        },
                        Event::Window { win_event, .. } => match win_event {
                            WindowEvent::FocusGained => is_focused = true,
                            WindowEvent::FocusLost => is_focused = false,
                            WindowEvent::Resized(width, height) if width != 0 && height != 0 => {
                                video.resize(width as u32, height as u32);
                            }
                            _ => (),
                        },
                        _ => (),
                    }
                }

                video.render(gb.pixel_data_rgb());
            }

            // TODO: sleep better
            std::thread::sleep(std::time::Duration::new(0, 1_000_000_000u32 / 120));
        }

        // Cleanup
        audio.pause();

        if let Ok(gb) = self.gb.lock() {
            if gb.cartridge_has_battery() {
                let mut f = File::create(self.sav_path.clone()).unwrap();
                f.write_all(gb.cartridge_ram()).unwrap();
            }
        }
    }
}
