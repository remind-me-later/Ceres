use super::audio::{AudioCallbacks, AudioRenderer};
use super::error::Error;
use super::video;
use ceres_core::{BootRom, Cartridge, Gameboy};
use glutin::{
    dpi::PhysicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    ContextBuilder,
};
use std::{path::PathBuf, time::Instant};

pub struct Emulator {
    gameboy: Gameboy<AudioCallbacks>,
    event_loop: EventLoop<()>,
    is_focused: bool,
    is_gui_paused: bool,
    video_renderer: video::Renderer,
    audio_renderer: AudioRenderer,
}

impl Emulator {
    pub fn new(
        model: ceres_core::Model,
        cartridge: Cartridge,
        boot_rom: BootRom,
    ) -> Result<Self, Error> {
        let event_loop = EventLoop::new();
        let window_builder = WindowBuilder::new()
            .with_title(super::CERES_STR)
            .with_inner_size(PhysicalSize {
                width: ceres_core::SCREEN_WIDTH as i32 * 4,
                height: ceres_core::SCREEN_HEIGHT as i32 * 4,
            })
            .with_min_inner_size(PhysicalSize {
                width: ceres_core::SCREEN_WIDTH as i32,
                height: ceres_core::SCREEN_HEIGHT as i32,
            });

        let context_builder = ContextBuilder::new();

        let display = glium::Display::new(window_builder, context_builder, &event_loop).unwrap();

        let inner_size = display.gl_window().window().inner_size();

        let video_renderer = video::Renderer::new(display, inner_size.width, inner_size.height);

        let (audio_renderer, audio_callbacks) = AudioRenderer::new().map_err(Error::new)?;
        let gameboy = ceres_core::Gameboy::new(
            model,
            cartridge,
            boot_rom,
            audio_callbacks,
            ceres_core::MonochromePaletteColors::Grayscale,
        );

        Ok(Self {
            event_loop,
            gameboy,
            is_focused: false,
            is_gui_paused: false,
            video_renderer,
            audio_renderer,
        })
    }

    pub fn run(mut self, sav_path: PathBuf) -> ! {
        let mut next_frame = Instant::now();

        self.event_loop
            .run(move |event, _, control_flow| match event {
                Event::LoopDestroyed => {
                    let cartridge = self.gameboy.cartridge();
                    super::save_data(&sav_path, cartridge);

                    return;
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(physical_size) => self
                        .video_renderer
                        .resize_viewport(physical_size.width as u32, physical_size.height as u32),
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
                                    VirtualKeyCode::W => self.gameboy.press(Button::Up),
                                    VirtualKeyCode::A => self.gameboy.press(Button::Left),
                                    VirtualKeyCode::S => self.gameboy.press(Button::Down),
                                    VirtualKeyCode::D => self.gameboy.press(Button::Right),
                                    VirtualKeyCode::K => self.gameboy.press(Button::B),
                                    VirtualKeyCode::L => self.gameboy.press(Button::A),
                                    VirtualKeyCode::Return => self.gameboy.press(Button::Start),
                                    VirtualKeyCode::Back => self.gameboy.press(Button::Select),
                                    VirtualKeyCode::Space => {
                                        if self.is_gui_paused {
                                            self.audio_renderer.play();
                                            self.is_gui_paused = false;
                                        } else {
                                            self.audio_renderer.pause();
                                            self.is_gui_paused = true;
                                        }
                                    }
                                    _ => (),
                                },
                                ElementState::Released => match key {
                                    VirtualKeyCode::W => self.gameboy.release(Button::Up),
                                    VirtualKeyCode::A => self.gameboy.release(Button::Left),
                                    VirtualKeyCode::S => self.gameboy.release(Button::Down),
                                    VirtualKeyCode::D => self.gameboy.release(Button::Right),
                                    VirtualKeyCode::K => self.gameboy.release(Button::B),
                                    VirtualKeyCode::L => self.gameboy.release(Button::A),
                                    VirtualKeyCode::Return => self.gameboy.release(Button::Start),
                                    VirtualKeyCode::Back => self.gameboy.release(Button::Select),
                                    _ => (),
                                },
                            }
                        }
                    }
                    _ => (),
                },
                Event::MainEventsCleared => {
                    if self.is_gui_paused {
                        *control_flow = ControlFlow::Wait;
                        return;
                    }

                    let now = Instant::now();

                    if now >= next_frame {
                        self.gameboy.run_frame();

                        let pixel_data = std::mem::take(self.gameboy.mut_pixel_data());
                        self.video_renderer.update_texture(pixel_data.rgba());
                        self.video_renderer.draw();
                        next_frame = now + ceres_core::FRAME_DURATION;
                    }

                    *control_flow = ControlFlow::Poll;
                }
                _ => (),
            });
    }
}

// pub struct ContextWrapper {
//     windowed_context: WindowedContext<PossiblyCurrent>,
// }

// impl video::Context for ContextWrapper {
//     fn get_proc_address(&mut self, procname: &str) -> *const c_void {
//         self.windowed_context.get_proc_address(procname)
//     }

//     fn swap_buffers(&mut self) {
//         self.windowed_context.swap_buffers().unwrap();
//     }

//     fn make_current(&mut self) {}

//     fn resize(&mut self, width: u32, height: u32) {
//         self.windowed_context.resize(PhysicalSize { width, height });
//     }
// }
