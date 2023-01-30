use std::sync::{Arc, Mutex};

use {
    crate::{audio, opengl, CERES_STYLIZED},
    ceres_core::{Button, Cartridge, Gb, Model},
    glutin::{
        config::{Config, ConfigTemplateBuilder},
        context::{
            ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext,
        },
        display::{Display, GetGlDisplay},
        prelude::{GlConfig, GlDisplay, NotCurrentGlContextSurfaceAccessor},
        surface::{GlSurface, SwapInterval},
    },
    glutin_winit::DisplayBuilder,
    std::{
        fs::File,
        io::{Read, Write},
        num::NonZeroU32,
        path::{Path, PathBuf},
    },
    winit::{
        dpi::PhysicalSize,
        event::{ElementState, Event, VirtualKeyCode, WindowEvent},
        event_loop::EventLoop,
        window::Window,
        window::WindowBuilder,
    },
};

struct InputBuffer {
    press_vec: [ceres_core::Button; 16],
    press_idx: usize,

    rel_vec: [ceres_core::Button; 16],
    rel_idx: usize,
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

        self.press_vec[self.press_idx] = button;
        self.press_idx += 1;
    }

    fn release(&mut self, button: Button) {
        if self.rel_idx >= 16 {
            return;
        }

        self.rel_vec[self.rel_idx] = button;
        self.rel_idx += 1;
    }

    fn flush(&mut self, gb: &mut Gb) {
        for i in 0..self.press_idx {
            gb.press(self.press_vec[i]);
        }

        for i in 0..self.rel_idx {
            gb.release(self.rel_vec[i]);
        }

        self.press_idx = 0;
        self.rel_idx = 0;
    }
}

pub struct Emu {
    gb: Arc<Mutex<Gb>>,
    has_focus: bool,
    sav_path: PathBuf,
    state: Option<(PossiblyCurrentContext, opengl::GlWindow)>,
    renderer: Option<opengl::Renderer>,
    event_loop: EventLoop<()>,
    window: Option<Window>,
    gl_config: Config,
    not_current_gl_context: Option<NotCurrentContext>,
    gl_display: Display,
    _audio: audio::Renderer,
    in_buf: InputBuffer,
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

        let event_loop = EventLoop::new();

        // ################################################### BEGIN COPYPASTA

        let window_builder = Some(
            WindowBuilder::new()
                .with_title(CERES_STYLIZED)
                .with_inner_size(PhysicalSize {
                    width: ceres_core::PX_WIDTH as i32 * 4,
                    height: ceres_core::PX_HEIGHT as i32 * 4,
                })
                .with_min_inner_size(PhysicalSize {
                    width: ceres_core::PX_WIDTH as i32,
                    height: ceres_core::PX_HEIGHT as i32,
                }),
        );

        let template = ConfigTemplateBuilder::new();
        let display_builder = DisplayBuilder::new().with_window_builder(window_builder);

        let (window, gl_config) = display_builder
            .build(&event_loop, template, |configs| {
                // Find the config with the maximum number of samples, so our triangle will
                // be smooth.
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() > accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .unwrap()
            })
            .unwrap();

        let raw_window_handle = window
            .as_ref()
            .map(raw_window_handle::HasRawWindowHandle::raw_window_handle);

        // XXX The display could be obtained from the any object created by it, so we
        // can query it from the config.docs.rs/winit/
        let gl_display = gl_config.display();

        // The context creation part. It can be created before surface and that's how
        // it's expected in multithreaded + multiwindow operation mode, since you
        // can send NotCurrentContext, but not Surface.
        let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

        // Since glutin by default tries to create OpenGL core context, which may not be
        // present we should try gles.
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(raw_window_handle);

        let not_current_gl_context = Some(unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .unwrap_or_else(|_| {
                    gl_display
                        .create_context(&gl_config, &fallback_context_attributes)
                        .expect("failed to create context")
                })
        });

        // ############################################# END COPYPASTA

        let gb = {
            let sample_rate = audio::Renderer::sample_rate();
            Arc::new(Mutex::new(Gb::new(model, sample_rate, cart)))
        };

        let audio = {
            let gb = Arc::clone(&gb);
            audio::Renderer::new(gb)
        };

        let has_focus: bool = false;
        let sav_path: PathBuf = path;

        let state: Option<(PossiblyCurrentContext, opengl::GlWindow)> = None;
        let renderer: Option<opengl::Renderer> = None;

        Self {
            gb,
            has_focus,
            sav_path,
            state,
            renderer,
            event_loop,
            window,
            gl_config,
            not_current_gl_context,
            gl_display,
            _audio: audio,
            in_buf: InputBuffer::new(),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn run(mut self) -> ! {
        self.event_loop
            .run(move |event, window_target, control_flow| match event {
                Event::Resumed => {
                    control_flow.set_poll();

                    let window = self.window.take().unwrap_or_else(|| {
                        let window_builder = WindowBuilder::new().with_transparent(true);
                        glutin_winit::finalize_window(
                            window_target,
                            window_builder,
                            &self.gl_config,
                        )
                        .unwrap()
                    });

                    let gl_window = opengl::GlWindow::new(window, &self.gl_config);

                    // Make it current.
                    let gl_context = self
                        .not_current_gl_context
                        .take()
                        .unwrap()
                        .make_current(&gl_window.surface)
                        .unwrap();

                    // The context needs to be current for the Renderer to set up shaders and
                    // buffers. It also performs function loading, which needs a current context on
                    // WGL.
                    self.renderer
                        .get_or_insert_with(|| opengl::Renderer::new(&self.gl_display));

                    // Try setting vsync.
                    if let Err(res) = gl_window.surface.set_swap_interval(
                        &gl_context,
                        SwapInterval::Wait(NonZeroU32::new(1).unwrap()),
                    ) {
                        eprintln!("Error setting vsync: {res:?}");
                    }

                    assert!(self.state.replace((gl_context, gl_window)).is_none());
                }
                Event::LoopDestroyed => {
                    // save
                    if let Ok(gb) = self.gb.lock() {
                        if gb.cartridge_has_battery() {
                            let mut f = File::create(self.sav_path.clone()).unwrap();
                            f.write_all(gb.cartridge_ram()).unwrap();
                        }
                    }
                }
                Event::WindowEvent { event: e, .. } => match e {
                    WindowEvent::Resized(size) => {
                        if size.width != 0 && size.height != 0 {
                            // Some platforms like EGL require resizing GL surface to update the size
                            // Notable platforms here are Wayland and macOS, other don't require it
                            // and the function is no-op, but it's wise to resize it for portability
                            // reasons.
                            if let Some((gl_context, gl_window)) = &self.state {
                                gl_window.surface.resize(
                                    gl_context,
                                    NonZeroU32::new(size.width).unwrap(),
                                    NonZeroU32::new(size.height).unwrap(),
                                );
                                let renderer = self.renderer.as_ref().unwrap();
                                renderer.resize(size.width, size.height);
                            }
                        }
                    }
                    WindowEvent::CloseRequested => control_flow.set_exit(),
                    WindowEvent::Focused(is_focused) => self.has_focus = is_focused,
                    WindowEvent::KeyboardInput { input, .. } => {
                        if !self.has_focus {
                            return;
                        }

                        if let Some(key) = input.virtual_keycode {
                            match input.state {
                                ElementState::Pressed => match key {
                                    VirtualKeyCode::W => self.in_buf.press(Button::Up),
                                    VirtualKeyCode::A => self.in_buf.press(Button::Left),
                                    VirtualKeyCode::S => self.in_buf.press(Button::Down),
                                    VirtualKeyCode::D => self.in_buf.press(Button::Right),
                                    VirtualKeyCode::K => self.in_buf.press(Button::A),
                                    VirtualKeyCode::L => self.in_buf.press(Button::B),
                                    VirtualKeyCode::Return => self.in_buf.press(Button::Start),
                                    VirtualKeyCode::Back => self.in_buf.press(Button::Select),
                                    _ => (),
                                },
                                ElementState::Released => match key {
                                    VirtualKeyCode::W => self.in_buf.release(Button::Up),
                                    VirtualKeyCode::A => self.in_buf.release(Button::Left),
                                    VirtualKeyCode::S => self.in_buf.release(Button::Down),
                                    VirtualKeyCode::D => self.in_buf.release(Button::Right),
                                    VirtualKeyCode::K => self.in_buf.release(Button::A),
                                    VirtualKeyCode::L => self.in_buf.release(Button::B),
                                    VirtualKeyCode::Return => self.in_buf.release(Button::Start),
                                    VirtualKeyCode::Back => self.in_buf.release(Button::Select),
                                    _ => (),
                                },
                            }
                        }
                    }
                    _ => (),
                },
                Event::MainEventsCleared => {
                    if let Some((gl_context, gl_window)) = &self.state {
                        let renderer = self.renderer.as_ref().unwrap();

                        if let Ok(mut gb) = self.gb.lock() {
                            self.in_buf.flush(&mut gb);
                            renderer.draw_frame(gb.pixel_data_rgb());
                        }

                        gl_window.surface.swap_buffers(gl_context).unwrap();
                    }
                }
                _ => (),
            });
    }
}
