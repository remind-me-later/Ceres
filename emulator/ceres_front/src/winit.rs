use crate::{gl::Gl, CERES_STYLIZED};
use parking_lot::Mutex;
use std::sync::Arc;
use winit::{dpi::PhysicalSize, event_loop::EventLoop, window::WindowBuilder};
use {
    crate::audio,
    ceres_core::{Button, Cartridge, Gb, Model},
    std::{
        fs::File,
        io::{Read, Write},
        path::{Path, PathBuf},
    },
    winit::event::{ElementState, Event, VirtualKeyCode, WindowEvent},
};

struct InputBuffer {
    press_vec: [ceres_core::Button; 16],
    press_idx: u8,

    rel_vec: [ceres_core::Button; 16],
    rel_idx: u8,
}

impl InputBuffer {
    fn new() -> InputBuffer {
        let press_vec = [Button::A; 16];
        let rel_vec = [Button::A; 16];

        InputBuffer {
            press_vec,
            press_idx: 0,

            rel_vec,
            rel_idx: 0,
        }
    }

    fn press(&mut self, button: Button) {
        if self.press_idx >= 16 {
            return;
        }

        self.press_vec[self.press_idx as usize] = button;
        self.press_idx += 1;
    }

    fn release(&mut self, button: Button) {
        if self.rel_idx >= 16 {
            return;
        }

        self.rel_vec[self.rel_idx as usize] = button;
        self.rel_idx += 1;
    }

    fn flush(&mut self, gb: &mut Gb) {
        for i in 0..self.press_idx {
            gb.press(self.press_vec[i as usize]);
        }

        for i in 0..self.rel_idx {
            gb.release(self.rel_vec[i as usize]);
        }

        self.press_idx = 0;
        self.rel_idx = 0;
    }
}

pub struct Emu {
    gb: Arc<Mutex<Gb>>,
    sav_path: PathBuf,
    _audio: audio::Renderer,
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

        let audio = {
            let gb = Arc::clone(&gb);
            audio::Renderer::new(gb)
        };

        Self {
            gb,
            sav_path,
            _audio: audio,
        }
    }

    pub fn run(self) -> ! {
        let event_loop = EventLoop::new();

        let window_builder = WindowBuilder::new()
            .with_title(CERES_STYLIZED)
            .with_inner_size(PhysicalSize {
                width: ceres_core::PX_WIDTH as i32 * 4,
                height: ceres_core::PX_HEIGHT as i32 * 4,
            })
            .with_min_inner_size(PhysicalSize {
                width: ceres_core::PX_WIDTH as i32,
                height: ceres_core::PX_HEIGHT as i32,
            });

        let mut gl = Gl::new(&event_loop, window_builder);

        let mut is_focused = true;
        let mut in_buf = InputBuffer::new();

        event_loop.run(move |event, _, control_flow| match event {
            Event::Resumed => {
                control_flow.set_poll();
                gl.make_current();
            }
            Event::LoopDestroyed => {
                // save
                let gb = self.gb.lock();
                if gb.cartridge_has_battery() {
                    let mut f = File::create(self.sav_path.clone()).unwrap();
                    f.write_all(gb.cartridge_ram()).unwrap();
                }
            }
            Event::WindowEvent { event: e, .. } => match e {
                WindowEvent::Resized(size) => {
                    gl.resize(size.width, size.height);
                }
                WindowEvent::CloseRequested => control_flow.set_exit(),
                WindowEvent::Focused(f) => is_focused = f,
                WindowEvent::KeyboardInput { input, .. } => {
                    if !is_focused {
                        return;
                    }

                    if let Some(key) = input.virtual_keycode {
                        match input.state {
                            ElementState::Pressed => match key {
                                VirtualKeyCode::W => in_buf.press(Button::Up),
                                VirtualKeyCode::A => in_buf.press(Button::Left),
                                VirtualKeyCode::S => in_buf.press(Button::Down),
                                VirtualKeyCode::D => in_buf.press(Button::Right),
                                VirtualKeyCode::K => in_buf.press(Button::A),
                                VirtualKeyCode::L => in_buf.press(Button::B),
                                VirtualKeyCode::Return => in_buf.press(Button::Start),
                                VirtualKeyCode::Back => in_buf.press(Button::Select),
                                _ => (),
                            },
                            ElementState::Released => match key {
                                VirtualKeyCode::W => in_buf.release(Button::Up),
                                VirtualKeyCode::A => in_buf.release(Button::Left),
                                VirtualKeyCode::S => in_buf.release(Button::Down),
                                VirtualKeyCode::D => in_buf.release(Button::Right),
                                VirtualKeyCode::K => in_buf.release(Button::A),
                                VirtualKeyCode::L => in_buf.release(Button::B),
                                VirtualKeyCode::Return => in_buf.release(Button::Start),
                                VirtualKeyCode::Back => in_buf.release(Button::Select),
                                _ => (),
                            },
                        }
                    }
                }
                _ => (),
            },
            Event::MainEventsCleared => gl.render(|| {
                let mut gb = self.gb.lock();
                in_buf.flush(&mut gb);
                gb.pixel_data_rgb()
            }),
            _ => (),
        });
    }
}
