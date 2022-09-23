use {
    crate::{audio, video},
    ceres_core::{Button, Cartridge, Gb, Model},
    glutin::event_loop::EventLoop,
    glutin::{
        event::{ElementState, Event, VirtualKeyCode, WindowEvent},
        event_loop::ControlFlow,
    },
    quanta::Clock,
    std::{
        fs::File,
        io::{Read, Write},
        path::{Path, PathBuf},
    },
};

pub struct Emu {
    gb: Gb<audio::Renderer>,
    has_focus: bool,
    sav_path: PathBuf,
    video: video::Renderer,
    last_frame: u64,
    clock: Clock,
}

impl Emu {
    /// # Panics
    ///
    /// Will panic on invalid rom or ram file
    pub fn run(model: Model, mut path: PathBuf) -> ! {
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

        let events = EventLoop::new();

        let video = video::Renderer::new(&events);
        let audio = audio::Renderer::new();
        let sample_rate = audio.sample_rate();
        let gb = Gb::new(model, audio, sample_rate, cart);

        let clock = Clock::new();

        let mut emu = Self {
            gb,
            has_focus: false,
            sav_path: path,
            video,
            last_frame: clock.raw(),
            clock,
        };

        events.run(move |event, _, control_flow| match event {
            Event::LoopDestroyed => {
                // save
                if emu.gb.cartridge_has_battery() {
                    let mut f = File::create(emu.sav_path.clone()).unwrap();
                    f.write_all(emu.gb.cartridge_ram()).unwrap();
                }
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(s) => emu.video.resize(s.width, s.height),
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Focused(is_focused) => emu.has_focus = is_focused,
                WindowEvent::KeyboardInput { input, .. } => {
                    if !emu.has_focus {
                        return;
                    }

                    if let Some(key) = input.virtual_keycode {
                        match input.state {
                            ElementState::Pressed => match key {
                                VirtualKeyCode::W => emu.gb.press(Button::Up),
                                VirtualKeyCode::A => emu.gb.press(Button::Left),
                                VirtualKeyCode::S => emu.gb.press(Button::Down),
                                VirtualKeyCode::D => emu.gb.press(Button::Right),
                                VirtualKeyCode::K => emu.gb.press(Button::A),
                                VirtualKeyCode::L => emu.gb.press(Button::B),
                                VirtualKeyCode::Return => emu.gb.press(Button::Start),
                                VirtualKeyCode::Back => emu.gb.press(Button::Select),
                                // System
                                VirtualKeyCode::F => emu.video.toggle_fullscreen(),
                                _ => (),
                            },
                            ElementState::Released => match key {
                                VirtualKeyCode::W => emu.gb.release(Button::Up),
                                VirtualKeyCode::A => emu.gb.release(Button::Left),
                                VirtualKeyCode::S => emu.gb.release(Button::Down),
                                VirtualKeyCode::D => emu.gb.release(Button::Right),
                                VirtualKeyCode::K => emu.gb.release(Button::A),
                                VirtualKeyCode::L => emu.gb.release(Button::B),
                                VirtualKeyCode::Return => emu.gb.release(Button::Start),
                                VirtualKeyCode::Back => emu.gb.release(Button::Select),
                                _ => (),
                            },
                        }
                    }
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                let end = emu.clock.raw();
                let elapsed = emu.clock.delta(emu.last_frame, end);

                if elapsed < ceres_core::FRAME_DUR {
                    std::thread::sleep(ceres_core::FRAME_DUR - elapsed);
                }

                emu.gb.run_frame();
                emu.video.draw_frame(emu.gb.pixel_data_rgb());
                emu.last_frame = end;
            }
            _ => (),
        });
    }
}
