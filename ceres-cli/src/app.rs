use crate::{
    audio,
    video::{self, Renderer},
    Scaling, CERES_STYLIZED, INIT_HEIGHT, INIT_WIDTH, PX_HEIGHT, PX_WIDTH,
};
use ceres_core::Cart;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;
use std::time::Instant;
use std::{io::Read, sync::RwLock};
use thread_priority::ThreadBuilderExt;
use winit::{
    dpi::PhysicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::ControlFlow,
    keyboard::NamedKey,
    window::Fullscreen,
};
use {
    alloc::sync::Arc,
    anyhow::Context,
    ceres_core::Gb,
    core::time::Duration,
    std::{fs::File, io::Write, path::Path},
    winit::window,
};

struct AppState<'a> {
    renderer: video::Renderer<'a>,
}

impl<'a> AppState<'a> {
    fn new(event_loop: &winit::event_loop::ActiveEventLoop, scaling: Scaling) -> Self {
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

        let window = event_loop.create_window(window_attributes).unwrap();

        let mut video =
            pollster::block_on(Renderer::new(window, scaling)).expect("Could not create renderer");

        video.resize(PhysicalSize {
            width: INIT_WIDTH,
            height: INIT_HEIGHT,
        });

        AppState { renderer: video }
    }
}

pub struct App<'a> {
    project_dirs: directories::ProjectDirs,
    audio: audio::Renderer,
    gb: Arc<RwLock<Gb<audio::RingBuffer>>>,
    scaling: Scaling,
    rom_ident: String,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    exit: Arc<AtomicBool>,
    pause_thread: Arc<AtomicBool>,
    // NOTE: `AppState` carries the `Window`, thus it should be dropped after everything else.
    state: Option<AppState<'a>>,
}

impl<'a> App<'a> {
    pub fn new(
        project_dirs: directories::ProjectDirs,
        model: ceres_core::Model,
        rom_path: &Path,
        scaling: Scaling,
    ) -> anyhow::Result<Self> {
        fn init_gb(
            model: ceres_core::Model,
            project_dirs: &directories::ProjectDirs,
            rom_path: &Path,
            audio_callback: audio::RingBuffer,
        ) -> anyhow::Result<(Gb<audio::RingBuffer>, String)> {
            let rom = {
                std::fs::read(rom_path)
                    .map(Vec::into_boxed_slice)
                    .context("no such file")?
            };

            // TODO: core error
            let mut cart = Cart::new(rom).unwrap();
            let ident = {
                let mut ident = String::new();
                cart.ascii_title().read_to_string(&mut ident).unwrap();
                ident.push('-');
                ident.push_str(cart.version().to_string().as_str());
                ident.push('-');
                ident.push_str(cart.header_checksum().to_string().as_str());
                ident.push('-');
                ident.push_str(cart.global_checksum().to_string().as_str());

                ident
            };

            if let Ok(ram) =
                std::fs::read(project_dirs.data_dir().join(&ident).with_extension("sav"))
                    .map(Vec::into_boxed_slice)
            {
                cart.set_ram(ram).unwrap();
            }

            let sample_rate = audio::Renderer::sample_rate();

            Ok((Gb::new(model, sample_rate, cart, audio_callback), ident))
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

        let (gb, rom_ident) = init_gb(model, &project_dirs, rom_path, ring_buffer)?;
        let gb = Arc::new(RwLock::new(gb));

        let thread_builder = std::thread::Builder::new().name("gb_loop".to_owned());

        let thread_handle = {
            let gb = Arc::clone(&gb);
            let exit = Arc::clone(&exit);
            let pause_thread = Arc::clone(&pause_thread);

            // std::thread::spawn(move || gb_loop(gb, exit, pause_thread))
            thread_builder.spawn_with_priority(thread_priority::ThreadPriority::Max, move |_| {
                gb_loop(gb, exit, pause_thread);
            })?
        };

        Ok(Self {
            project_dirs,
            gb,
            audio,
            scaling,
            rom_ident,
            thread_handle: Some(thread_handle),
            state: None,
            exit,
            pause_thread,
        })
    }

    fn handle_key(&mut self, event: &KeyEvent) {
        use {ceres_core::Button, winit::event::ElementState, winit::keyboard::Key};

        if let Some(AppState { renderer }) = &mut self.state {
            if !renderer.window().has_focus() {
                return;
            }

            if let Ok(mut gb) = self.gb.write() {
                let key = &event.logical_key;

                match event.state {
                    ElementState::Pressed => match key.as_ref() {
                        Key::Character("w") => gb.press(Button::Up),
                        Key::Character("a") => gb.press(Button::Left),
                        Key::Character("s") => gb.press(Button::Down),
                        Key::Character("d") => gb.press(Button::Right),
                        Key::Character("k") => gb.press(Button::A),
                        Key::Character("l") => gb.press(Button::B),
                        Key::Character("m") => gb.press(Button::Start),
                        Key::Character("n") => gb.press(Button::Select),
                        // System
                        Key::Character("f") => match renderer.window().fullscreen() {
                            Some(_) => renderer.window().set_fullscreen(None),
                            None => renderer
                                .window()
                                .set_fullscreen(Some(Fullscreen::Borderless(None))),
                        },
                        Key::Character("z") => {
                            self.scaling = self.scaling.next();
                            renderer.choose_scale_mode(self.scaling);
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
                        Key::Character("w") => gb.release(Button::Up),
                        Key::Character("a") => gb.release(Button::Left),
                        Key::Character("s") => gb.release(Button::Down),
                        Key::Character("d") => gb.release(Button::Right),
                        Key::Character("k") => gb.release(Button::A),
                        Key::Character("l") => gb.release(Button::B),
                        Key::Character("m") => gb.release(Button::Start),
                        Key::Character("n") => gb.release(Button::Select),
                        _ => (),
                    },
                }
            }
        }
    }

    fn save_data(&self) {
        if let Ok(gb) = self.gb.read() {
            if let Some(save_data) = gb.cartridge().save_data() {
                std::fs::create_dir_all(self.project_dirs.data_dir())
                    .expect("couldn't create data directory");
                let sav_file = File::create(
                    self.project_dirs
                        .data_dir()
                        .join(&self.rom_ident)
                        .with_extension("sav"),
                );
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

impl<'a> winit::application::ApplicationHandler for App<'a> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.pause_thread.store(false, Relaxed);

        self.state.replace(AppState::new(event_loop, self.scaling));

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

            if let Some(AppState { renderer }) = self.state.as_mut() {
                if let Ok(gb) = self.gb.read() {
                    renderer.update_texture(gb.pixel_data_rgb());
                }

                renderer.window().request_redraw();
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
                    if let Some(AppState { renderer }) = self.state.as_mut() {
                        renderer.resize(PhysicalSize { width, height });
                    }
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => self.handle_key(&key_event),
            WindowEvent::RedrawRequested => {
                use wgpu::SurfaceError::{Lost, OutOfMemory, Outdated, Timeout};
                if let Some(AppState { renderer }) = self.state.as_mut() {
                    match renderer.render() {
                        Ok(()) => {}
                        Err(Lost | Outdated) => renderer.on_lost(),
                        Err(OutOfMemory) => event_loop.exit(),
                        Err(Timeout) => eprintln!("Surface timeout"),
                    }
                }
            }
            _ => (),
        }
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.audio.pause().unwrap();
        self.pause_thread.store(true, Relaxed);
        self.state = None;
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
