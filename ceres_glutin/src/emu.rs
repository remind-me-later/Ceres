use {
    crate::{audio, video},
    ceres_core::{Gb, Model},
    glutin::event_loop::EventLoop,
    std::{
        fs::File,
        io::Read,
        path::{Path, PathBuf},
    },
};

/// # Panics
///
/// Will panic on invalid rom or ram file
pub fn run(model: Model, mut rom_path: PathBuf) -> ! {
    fn read_file_into(path: &Path, buf: &mut [u8]) -> Result<(), std::io::Error> {
        let mut f = File::open(path)?;
        let _ = f.read(buf).unwrap();
        Ok(())
    }

    read_file_into(&rom_path, Gb::cartridge_rom_mut()).unwrap();

    let sav_path = {
        rom_path.set_extension("sav");
        rom_path
    };

    read_file_into(&sav_path, Gb::cartridge_ram_mut()).ok();

    let audio = audio::Renderer::init();

    let gb = Gb::new(
        model,
        imp::apu_frame_callback,
        audio::Renderer::sample_rate(),
    )
    .unwrap();

    let event_loop = EventLoop::new();
    let video = video::Renderer::init(&event_loop);

    let mut emu = imp::Emu::new(gb, video, audio, sav_path);

    event_loop.run(move |event, _, control_flow| emu.main_loop(event, control_flow));
}

mod imp {
    use {
        crate::{audio, video},
        ceres_core::{Gb, Sample},
        glutin::{
            event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
            event_loop::ControlFlow,
        },
        std::{fs::File, io::Write, path::PathBuf, ptr::null_mut},
    };

    static mut EMU: *mut Emu = null_mut();

    pub struct Emu {
        gb: &'static mut Gb,
        video: video::Renderer,
        audio: audio::Renderer,

        sav_path: PathBuf,
        has_focus: bool,
        paused: bool,
    }

    impl Emu {
        pub fn new(
            gb: &'static mut Gb,
            video: video::Renderer,
            audio: audio::Renderer,
            sav_path: PathBuf,
        ) -> Self {
            let mut emu = Emu {
                gb,
                sav_path,
                video,
                has_focus: true,
                audio,
                paused: false,
            };

            unsafe {
                EMU = &mut emu;
            };

            emu.audio.play();

            emu
        }

        pub fn main_loop(&mut self, event: Event<()>, control_flow: &mut ControlFlow) {
            match event {
                Event::LoopDestroyed => self.save(),
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(s) => self.resize(s.width as u32, s.height as u32),
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Focused(is_focused) => self.focus(is_focused),
                    WindowEvent::KeyboardInput { input, .. } => self.key_input(input),
                    _ => (),
                },
                Event::MainEventsCleared => self.main_cleared(control_flow),
                _ => (),
            }
        }

        pub fn resize(&mut self, width: u32, height: u32) {
            self.video.resize(width, height);
        }

        pub fn main_cleared(&mut self, control_flow: &mut ControlFlow) {
            if self.paused {
                *control_flow = ControlFlow::Wait;
                return;
            }

            self.gb.run_frame();
            let rgba = self.gb.pixel_data();
            self.video.draw_frame(rgba);
        }

        pub fn key_input(&mut self, input: KeyboardInput) {
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
                        VirtualKeyCode::Space => self.toggle_pause(),
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

        pub fn focus(&mut self, is_focused: bool) {
            self.has_focus = is_focused;
        }

        pub fn save(&mut self) {
            if Gb::cartridge_has_battery() {
                let mut f = File::create(&self.sav_path).unwrap();
                f.write_all(Gb::cartridge_ram()).unwrap();
            }
        }

        pub fn toggle_pause(&mut self) {
            if self.paused {
                self.paused = false;
                self.audio.play();
            } else {
                self.paused = true;
                self.audio.pause();
            }
        }
    }

    #[inline]
    pub fn apu_frame_callback(l: Sample, r: Sample) {
        let emu = unsafe { &mut *EMU };
        emu.audio.push_frame(l, r);
    }
}
