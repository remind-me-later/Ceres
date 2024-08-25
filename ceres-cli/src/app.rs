use crate::{
    audio,
    video::{self, Renderer},
    Scaling, CERES_STYLIZED, SCREEN_MUL,
};
use ceres_core::Cart;
use core::num::NonZeroU32;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;
use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{
        ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
    },
    display::GetGlDisplay,
    prelude::{GlDisplay, NotCurrentGlContext, PossiblyCurrentGlContext},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use glutin_winit::GlWindow;
use std::sync::RwLock;
use std::time::Instant;
use thread_priority::ThreadBuilderExt;
use winit::{
    dpi::PhysicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::ControlFlow,
    keyboard::NamedKey,
    raw_window_handle::HasWindowHandle,
    window::{Fullscreen, Window},
};
use {
    alloc::sync::Arc,
    anyhow::Context,
    ceres_core::Gb,
    core::time::Duration,
    std::{
        fs::File,
        io::Write,
        path::{Path, PathBuf},
    },
    winit::window,
};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const INIT_WIDTH: u32 = PX_WIDTH * SCREEN_MUL;
const INIT_HEIGHT: u32 = PX_HEIGHT * SCREEN_MUL;

struct AppState {
    renderer: video::Renderer,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    // NOTE: Window should be dropped after all resources created using its
    // raw-window-handle.
    window: Window,
}

impl AppState {
    fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        display_builder: glutin_winit::DisplayBuilder,
        template: ConfigTemplateBuilder,
        not_current_gl_context: &mut Option<NotCurrentContext>,
    ) -> Self {
        let (mut window, gl_config) = match display_builder.build(event_loop, template, |configs| {
            configs
                .filter(GlConfig::hardware_accelerated)
                .min_by_key(GlConfig::num_samples)
                .unwrap()
        }) {
            Ok(ok) => ok,
            Err(_e) => {
                // self.exit_state = Err(e);
                event_loop.exit();
                panic!("Failed to create window");
            }
        };

        let raw_window_handle = window
            .as_ref()
            .and_then(|w| w.window_handle().ok())
            .map(|handle| handle.as_raw());

        // XXX The display could be obtained from any object created by it, so we can
        // query it from the config.
        let gl_display = gl_config.display();

        // The context creation part.
        let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

        // Since glutin by default tries to create OpenGL core context, which may not be
        // present we should try gles.
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(Some(Version::new(3, 0))))
            .build(raw_window_handle);

        // Reuse the uncurrented context from a suspended() call if it exists, otherwise
        // this is the first time resumed() is called, where the context still
        // has to be created.
        let not_current_gl_context = not_current_gl_context.take().unwrap_or_else(|| unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .unwrap_or_else(|_| {
                    gl_display
                        .create_context(&gl_config, &fallback_context_attributes)
                        .expect("failed to create context")
                })
        });

        let window = window.take().unwrap_or_else(|| {
            let window_attributes = winit::window::Window::default_attributes()
                .with_title(CERES_STYLIZED)
                .with_inner_size(PhysicalSize {
                    width: INIT_WIDTH,
                    height: INIT_HEIGHT,
                })
                .with_min_inner_size(PhysicalSize {
                    width: PX_WIDTH,
                    height: PX_HEIGHT,
                });
            glutin_winit::finalize_window(event_loop, window_attributes, &gl_config).unwrap()
        });

        let attrs = window
            .build_surface_attributes(SurfaceAttributesBuilder::default())
            .expect("Failed to build surface attributes");
        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &attrs)
                .unwrap()
        };

        // Make it current.
        let gl_context = not_current_gl_context.make_current(&gl_surface).unwrap();

        // The context needs to be current for the Renderer to set up shaders and
        // buffers. It also performs function loading, which needs a current context on
        // WGL.
        let renderer = Renderer::new(&gl_display);

        // Try setting vsync.
        if let Err(res) = gl_surface
            .set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
        {
            eprintln!("Error setting vsync: {res}");
        }

        AppState {
            renderer,
            gl_context,
            gl_surface,
            window,
        }
    }
}

pub struct App {
    audio: audio::Renderer,
    gb: Arc<RwLock<Gb<audio::RingBuffer>>>,
    scaling: Scaling,
    rom_path: PathBuf,
    display_builder: glutin_winit::DisplayBuilder,
    not_current_gl_context: Option<NotCurrentContext>,
    template: ConfigTemplateBuilder,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    exit: Arc<AtomicBool>,
    pause_thread: Arc<AtomicBool>,
    // NOTE: `AppState` carries the `Window`, thus it should be dropped after everything else.
    state: Option<AppState>,
}

impl App {
    pub fn new(
        model: ceres_core::Model,
        rom_path: PathBuf,
        scaling: Scaling,
        template: ConfigTemplateBuilder,
        display_builder: glutin_winit::DisplayBuilder,
    ) -> anyhow::Result<Self> {
        fn init_gb(
            model: ceres_core::Model,
            rom_path: &Path,
            audio_callback: audio::RingBuffer,
        ) -> anyhow::Result<Gb<audio::RingBuffer>> {
            let rom = {
                std::fs::read(rom_path)
                    .map(Vec::into_boxed_slice)
                    .context("no such file")?
            };

            let ram = std::fs::read(rom_path.with_extension("sav"))
                .map(Vec::into_boxed_slice)
                .ok();

            // TODO: core error
            let cart = Cart::new(rom, ram).unwrap();

            let sample_rate = audio::Renderer::sample_rate();

            Ok(Gb::new(model, sample_rate, cart, audio_callback))
        }

        fn gb_loop(
            gb: Arc<RwLock<Gb<audio::RingBuffer>>>,
            exit: Arc<AtomicBool>,
            pause_thread: Arc<AtomicBool>,
        ) {
            loop {
                let begin = std::time::Instant::now();

                if exit.load(Relaxed) {
                    break;
                }

                let mut duration = ceres_core::FRAME_DURATION;

                if !pause_thread.load(Relaxed) {
                    if let Ok(mut gb) = gb.write() {
                        duration = gb.run_frame();
                    }
                }

                let elapsed = begin.elapsed();

                if elapsed < duration {
                    spin_sleep::sleep(duration - elapsed);
                }
            }

            // FIXME: clippy says we have to drop
            drop(gb);
            drop(exit);
            drop(pause_thread);
        }

        let exit = Arc::new(AtomicBool::new(false));
        let pause_thread = Arc::new(AtomicBool::new(true));

        let audio = {
            let exit = Arc::clone(&exit);
            audio::Renderer::new(exit)?
        };
        let ring_buffer = audio.get_ring_buffer();

        let gb = Arc::new(RwLock::new(init_gb(model, &rom_path, ring_buffer)?));

        let thread_builder = std::thread::Builder::new().name("gb_loop".to_string());

        let thread_handle = {
            let gb = Arc::clone(&gb);
            let exit = Arc::clone(&exit);
            let pause_thread = Arc::clone(&pause_thread);

            // std::thread::spawn(move || gb_loop(gb, exit, pause_thread))
            thread_builder.spawn_with_priority(thread_priority::ThreadPriority::Max, move |_| {
                gb_loop(gb, exit, pause_thread)
            })?
        };

        Ok(Self {
            gb,
            audio,
            scaling,
            rom_path,
            template,
            display_builder,
            thread_handle: Some(thread_handle),
            state: None,
            not_current_gl_context: None,
            exit,
            pause_thread,
        })
    }

    fn handle_key(&mut self, event: &KeyEvent) {
        use {ceres_core::Button as B, winit::event::ElementState, winit::keyboard::Key};

        if let Some(state) = &mut self.state {
            if !state.window.has_focus() {
                return;
            }

            if let Ok(mut gb) = self.gb.write() {
                let key = &event.logical_key;

                match event.state {
                    ElementState::Pressed => match key.as_ref() {
                        Key::Character("w") => gb.press(B::Up),
                        Key::Character("a") => gb.press(B::Left),
                        Key::Character("s") => gb.press(B::Down),
                        Key::Character("d") => gb.press(B::Right),
                        Key::Character("k") => gb.press(B::A),
                        Key::Character("l") => gb.press(B::B),
                        Key::Character("m") => gb.press(B::Start),
                        Key::Character("n") => gb.press(B::Select),
                        // System
                        Key::Character("f") => match state.window.fullscreen() {
                            Some(_) => state.window.set_fullscreen(None),
                            None => state
                                .window
                                .set_fullscreen(Some(Fullscreen::Borderless(None))),
                        },
                        Key::Character("z") => {
                            self.scaling = self.scaling.next();
                            state.renderer.choose_scale_mode(self.scaling);
                        }
                        Key::Named(NamedKey::Space) => {
                            let is_paused = self.pause_thread.load(Relaxed);

                            if is_paused {
                                self.pause_thread.store(false, Relaxed);
                                self.audio.resume().unwrap();
                            } else {
                                self.pause_thread.store(true, Relaxed);
                                self.audio.pause().unwrap();
                            }
                        }
                        _ => (),
                    },
                    ElementState::Released => match key.as_ref() {
                        Key::Character("w") => gb.release(B::Up),
                        Key::Character("a") => gb.release(B::Left),
                        Key::Character("s") => gb.release(B::Down),
                        Key::Character("d") => gb.release(B::Right),
                        Key::Character("k") => gb.release(B::A),
                        Key::Character("l") => gb.release(B::B),
                        Key::Character("m") => gb.release(B::Start),
                        Key::Character("n") => gb.release(B::Select),
                        _ => (),
                    },
                }
            }
        }
    }

    fn save_data(&self) {
        if let Ok(gb) = self.gb.read() {
            if let Some(save_data) = gb.cartridge().save_data() {
                let sav_file = File::create(self.rom_path.with_extension("sav"));
                match sav_file {
                    Ok(mut f) => {
                        if let Err(e) = f.write_all(save_data) {
                            eprintln!("couldn't save data in save file: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("couldn't open save file: {e}");
                    }
                }
            }
        }
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.pause_thread.store(false, Relaxed);

        self.state.replace(AppState::new(
            event_loop,
            self.display_builder.clone(),
            self.template.clone(),
            &mut self.not_current_gl_context,
        ));

        // event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));

        self.audio.resume().unwrap();
    }

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        if let winit::event::StartCause::ResumeTimeReached { .. } = cause {
            event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_secs_f64(
                1.0 / 60.0,
            )));

            if let Some(AppState {
                renderer,
                gl_context,
                gl_surface,
                window,
            }) = self.state.as_mut()
            {
                if let Ok(gb) = self.gb.read() {
                    renderer.draw_frame(gb.pixel_data_rgb());
                }

                window.request_redraw();

                gl_surface.swap_buffers(gl_context).unwrap();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                if width != 0 && height != 0 {
                    // Some platforms like EGL require resizing GL surface to update the size
                    // Notable platforms here are Wayland and macOS, other don't require it
                    // and the function is no-op, but it's wise to resize it for portability
                    // reasons.
                    if let Some(AppState {
                        renderer,
                        gl_context,
                        gl_surface,
                        ..
                    }) = self.state.as_mut()
                    {
                        gl_surface.resize(
                            gl_context,
                            NonZeroU32::new(width).unwrap(),
                            NonZeroU32::new(height).unwrap(),
                        );
                        renderer.resize_viewport(width, height);
                    }
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => self.handle_key(&key_event),
            _ => (),
        }
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.audio.pause().unwrap();

        self.pause_thread.store(true, Relaxed);

        // This event is only raised on Android, where the backing NativeWindow for a GL
        // Surface can appear and disappear at any moment.
        println!("Android window removed");

        // Destroy the GL Surface and un-current the GL Context before ndk-glue releases
        // the window back to the system.
        let gl_context = self.state.take().unwrap().gl_context;
        assert!(self
            .not_current_gl_context
            .replace(gl_context.make_not_current().unwrap())
            .is_none());

        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn exiting(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        self.audio.pause().unwrap();
        self.exit.store(true, Relaxed);
        self.save_data();
        self.state = None;
        self.thread_handle.take().unwrap().join().unwrap();
    }
}
