use parking_lot::Mutex;
use raw_window_handle::HasRawWindowHandle;
use std::sync::Arc;
use {
    crate::{audio, video, CERES_STYLIZED},
    ceres_core::{Button, Cartridge, Gb, Model},
    glutin::{
        config::{Config, ConfigTemplateBuilder},
        context::{ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext},
        display::{Display, GetGlDisplay},
        prelude::{GlDisplay, NotCurrentGlContextSurfaceAccessor},
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
    has_focus: bool,
    sav_path: PathBuf,
    renderer: Option<video::Renderer>,
    _audio: audio::Renderer,
    in_buf: InputBuffer,

    ctx: GlutinCtx,
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

        let ctx = GlutinCtx::new();

        let gb = {
            let sample_rate = audio::Renderer::sample_rate();
            Arc::new(Mutex::new(Gb::new(model, sample_rate, cart)))
        };

        let audio = {
            let gb = Arc::clone(&gb);
            audio::Renderer::new(gb)
        };

        let sav_path: PathBuf = path;

        let renderer: Option<video::Renderer> = None;

        Self {
            gb,
            has_focus: false,
            sav_path,
            renderer,
            _audio: audio,
            in_buf: InputBuffer::new(),
            ctx,
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn run(mut self) -> ! {
        self.ctx
            .event_loop
            .run(move |event, _, control_flow| match event {
                Event::Resumed => {
                    control_flow.set_poll();

                    let gl_window =
                        video::GlWindow::new(self.ctx.window.take().unwrap(), &self.ctx.gl_config);

                    let gl_context = self
                        .ctx
                        .not_current_gl_context
                        .take()
                        .unwrap()
                        .make_current(&gl_window.surface)
                        .unwrap();

                    // The context needs to be current for the Renderer to set up shaders and
                    // buffers. It also performs function loading, which needs a current context on
                    // WGL.
                    self.renderer
                        .get_or_insert_with(|| video::Renderer::new(&self.ctx.gl_display));

                    // Try setting vsync.
                    if let Err(res) = gl_window.surface.set_swap_interval(
                        &gl_context,
                        SwapInterval::Wait(NonZeroU32::new(1).unwrap()),
                    ) {
                        eprintln!("Error setting vsync: {res:?}");
                    }

                    assert!(self.ctx.state.replace((gl_context, gl_window)).is_none());
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
                        if size.width != 0 && size.height != 0 {
                            if let Some((gl_context, gl_window)) = &self.ctx.state {
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
                    if let Some((gl_context, gl_window)) = &self.ctx.state {
                        let renderer = self.renderer.as_ref().unwrap();

                        {
                            let mut gb = self.gb.lock();
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

struct GlutinCtx {
    event_loop: EventLoop<()>,
    window: Option<Window>,
    gl_config: Config,
    not_current_gl_context: Option<NotCurrentContext>,
    gl_display: Display,
    state: Option<(PossiblyCurrentContext, video::GlWindow)>,
}

impl GlutinCtx {
    fn new() -> Self {
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

        let template = ConfigTemplateBuilder::new();
        let display_builder = DisplayBuilder::new().with_window_builder(Some(window_builder));

        let (window, gl_config) = display_builder
            .build(&event_loop, template, |mut confs| confs.next().unwrap())
            .unwrap();

        let raw_window_handle = window.as_ref().map(HasRawWindowHandle::raw_window_handle);
        let gl_display = gl_config.display();
        let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);
        let not_current_gl_context = Some(unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .expect("failed to create context")
        });

        let state = None;

        Self {
            event_loop,
            window,
            gl_config,
            not_current_gl_context,
            gl_display,
            state,
        }
    }
}
