use {
    super::{audio::AudioRenderer, video},
    ceres_core::{Cartridge, Gb, PX_HEIGHT, PX_WIDTH},
    glutin::{
        dpi::PhysicalSize,
        event::{ElementState, Event, VirtualKeyCode, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
        ContextBuilder,
    },
    std::{
        fs,
        fs::File,
        io::{Read, Write},
        path::{Path, PathBuf},
    },
};

pub struct Emulator {
    gb: Gb,
    event_loop: EventLoop<()>,
    is_focused: bool,
    video_renderer: *mut video::Renderer,
    sav_path: PathBuf,
}

impl Emulator {
    pub fn new(model: ceres_core::Model, rom_path: &Path) -> Self {
        let event_loop = EventLoop::new();

        let sav_path = rom_path.with_extension("sav");

        let cart = {
            let rom = read_file(rom_path).unwrap().into_boxed_slice();
            let ram = read_file(&sav_path).ok().map(Vec::into_boxed_slice);
            Cartridge::new(rom, ram).unwrap()
        };

        let mut video_callbacks = Box::new(Self::create_renderer(&event_loop));
        let audio_callbacks = Box::new(AudioRenderer::new());

        let video: *mut _ = video_callbacks.as_mut();

        let gb = Gb::new(model, cart, audio_callbacks, video_callbacks);

        Self {
            event_loop,
            gb,
            is_focused: false,
            video_renderer: video,
            sav_path,
        }
    }

    fn create_renderer(event_loop: &EventLoop<()>) -> video::Renderer {
        let window_builder = WindowBuilder::new()
            .with_title(super::CERES_STR)
            .with_inner_size(PhysicalSize {
                width: PX_WIDTH as i32 * 4,
                height: PX_HEIGHT as i32 * 4,
            })
            .with_min_inner_size(PhysicalSize {
                width: PX_WIDTH as i32,
                height: PX_HEIGHT as i32,
            });

        let context_builder = ContextBuilder::new();

        let display = glium::Display::new(window_builder, context_builder, event_loop).unwrap();

        let inner_size = display.gl_window().window().inner_size();

        video::Renderer::new(display, inner_size.width, inner_size.height)
    }

    pub fn run(mut self) -> ! {
        self.event_loop
            .run(move |event, _, control_flow| match event {
                Event::LoopDestroyed => {
                    if let Some(cart_ram) = self.gb.save_data() {
                        let mut f = File::create(self.sav_path.clone()).unwrap();
                        f.write_all(cart_ram).unwrap();
                    }
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(physical_size) => unsafe {
                        (*self.video_renderer).resize_viewport(
                            physical_size.width as u32,
                            physical_size.height as u32,
                        )
                    },
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Focused(is_focused) => self.is_focused = is_focused,
                    WindowEvent::KeyboardInput { input, .. } => {
                        use ceres_core::Button;

                        if !self.is_focused {
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
                Event::MainEventsCleared => self.gb.run_frame(),
                _ => (),
            });
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, ()> {
    let mut f = File::open(path).map_err(|_| ())?;
    let metadata = fs::metadata(&path).unwrap();
    let mut buffer = vec![0; metadata.len() as usize];
    f.read_exact(&mut buffer).unwrap();

    Ok(buffer)
}
