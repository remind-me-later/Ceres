use {
    crate::{audio, video},
    ceres_core::{Gb, Model, Sample},
    glutin::{
        event::{ElementState, Event, VirtualKeyCode, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
    std::{
        fs::File,
        io::{Read, Write},
        path::{Path, PathBuf},
        ptr::null_mut,
    },
};

static mut EMU: *mut Emu = null_mut();

#[allow(clippy::struct_excessive_bools)]
pub struct Emu {
    event_loop: EventLoop<()>,
    gb: &'static mut Gb,
    sav_path: PathBuf,
    video: video::Renderer,
    has_focus: bool,
    audio: audio::Renderer,
    paused: bool,
}

impl Emu {
    /// # Panics
    ///
    /// Will panic on invalid rom or ram file
    pub fn run(model: Model, rom_path: &Path) -> ! {
        let event_loop = EventLoop::new();

        let sav_path = rom_path.with_extension("sav");

        {
            // initialize cartridge
            fn read_file_into(path: &Path, buf: &mut [u8]) -> Result<(), std::io::Error> {
                let mut f = File::open(path)?;
                let _ = f.read(buf).unwrap();
                Ok(())
            }

            read_file_into(rom_path, Gb::cartridge_rom_mut()).unwrap();
            read_file_into(&sav_path, Gb::cartridge_ram_mut()).ok();
        }

        let video = video::Renderer::init(&event_loop);
        let audio = audio::Renderer::init();

        let gb = Gb::new(model, apu_frame_callback, audio::Renderer::sample_rate()).unwrap();

        let mut res = Self {
            event_loop,
            gb,
            sav_path,
            video,
            has_focus: true,
            audio,
            paused: false,
        };

        unsafe {
            EMU = &mut res;
        }

        res.main_loop();
    }

    fn main_loop(mut self) -> ! {
        self.audio.play();

        self.event_loop
            .run(move |event, _, control_flow| match event {
                Event::LoopDestroyed => {
                    if Gb::cartridge_has_battery() {
                        let mut f = File::create(self.sav_path.clone()).unwrap();
                        f.write_all(Gb::cartridge_ram()).unwrap();
                    }
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(physical_size) => self
                        .video
                        .resize(physical_size.width as u32, physical_size.height as u32),
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Focused(is_focused) => self.has_focus = is_focused,
                    WindowEvent::KeyboardInput { input, .. } => {
                        use ceres_core::Button;

                        if !self.has_focus {
                            return;
                        }

                        if let Some(key) = input.virtual_keycode {
                            match input.state {
                                ElementState::Pressed => match key {
                                    VirtualKeyCode::W => self.gb.press(Button::Up),
                                    VirtualKeyCode::A => self.gb.press(Button::Left),
                                    VirtualKeyCode::S => self.gb.press(Button::Down),
                                    VirtualKeyCode::D => self.gb.press(Button::Right),
                                    VirtualKeyCode::K => self.gb.press(Button::A),
                                    VirtualKeyCode::L => self.gb.press(Button::B),
                                    VirtualKeyCode::Return => self.gb.press(Button::Start),
                                    VirtualKeyCode::Back => self.gb.press(Button::Select),
                                    // System
                                    VirtualKeyCode::F => self.video.toggle_fullscreen(),
                                    VirtualKeyCode::Space => {
                                        // Pause
                                        if self.paused {
                                            self.paused = false;
                                            self.audio.play();
                                        } else {
                                            self.paused = true;
                                            self.audio.pause();
                                        }
                                    }
                                    _ => (),
                                },
                                ElementState::Released => match key {
                                    VirtualKeyCode::W => self.gb.release(Button::Up),
                                    VirtualKeyCode::A => self.gb.release(Button::Left),
                                    VirtualKeyCode::S => self.gb.release(Button::Down),
                                    VirtualKeyCode::D => self.gb.release(Button::Right),
                                    VirtualKeyCode::K => self.gb.release(Button::A),
                                    VirtualKeyCode::L => self.gb.release(Button::B),
                                    VirtualKeyCode::Return => self.gb.release(Button::Start),
                                    VirtualKeyCode::Back => self.gb.release(Button::Select),
                                    _ => (),
                                },
                            }
                        }
                    }
                    _ => (),
                },
                Event::MainEventsCleared => {
                    if self.paused {
                        *control_flow = ControlFlow::Wait;
                        return;
                    }

                    self.gb.run_frame();
                    let rgba = self.gb.pixel_data();
                    self.video.draw_frame(rgba);
                }
                _ => (),
            });
    }
}

#[inline]
fn apu_frame_callback(l: Sample, r: Sample) {
    let emu = unsafe { &mut *EMU };
    emu.audio.push_frame(l, r);
}
