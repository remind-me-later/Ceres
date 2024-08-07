use crate::{
    audio,
    video::{self},
    Scaling, CERES_STYLIZED, SCREEN_MUL,
};
use std::{sync::Mutex, time::Instant};
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
    std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
    },
    winit::window,
};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const INIT_WIDTH: u32 = PX_WIDTH * SCREEN_MUL;
const INIT_HEIGHT: u32 = PX_HEIGHT * SCREEN_MUL;

pub struct App<'a> {
    video: Option<video::Renderer<'a>>,
    audio: audio::Renderer,
    gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
    scaling: Scaling,
    rom_path: PathBuf,
    _thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl<'a> App<'a> {
    pub fn new(
        model: ceres_core::Model,
        rom_path: PathBuf,
        scaling: Scaling,
    ) -> anyhow::Result<Self> {
        fn init_gb(
            model: ceres_core::Model,
            rom_path: &Path,
            audio_callback: audio::RingBuffer,
        ) -> anyhow::Result<Gb<audio::RingBuffer>> {
            let rom = {
                fs::read(rom_path)
                    .map(Vec::into_boxed_slice)
                    .context("no such file")?
            };

            let ram = fs::read(rom_path.with_extension("sav"))
                .map(Vec::into_boxed_slice)
                .ok();

            // TODO: core error
            let cart = ceres_core::Cart::new(rom, ram).unwrap();

            let sample_rate = audio::Renderer::sample_rate();

            Ok(Gb::new(model, sample_rate, cart, audio_callback))
        }

        let audio = audio::Renderer::new()?;
        let ring_buffer = audio.get_ring_buffer();

        let gb = Arc::new(Mutex::new(init_gb(model, &rom_path, ring_buffer)?));
        let gb_clone = Arc::clone(&gb);

        let thread_handle = std::thread::spawn(move || loop {
            // TODO: kill thread gracefully

            let frame_duration = Duration::from_secs_f32(1.0 / ceres_core::FPS);
            let begin = std::time::Instant::now();

            if let Ok(mut gb) = gb_clone.lock() {
                gb.run_frame();
            }

            let elapsed = begin.elapsed();

            if elapsed < frame_duration {
                spin_sleep::sleep(frame_duration - elapsed);
            }
        });

        Ok(Self {
            gb,
            audio,
            scaling,
            rom_path,
            video: None,
            _thread_handle: Some(thread_handle),
        })
    }

    fn handle_key(&mut self, event: &KeyEvent) {
        use {ceres_core::Button as B, winit::event::ElementState, winit::keyboard::Key};

        if let Some(video) = &mut self.video {
            if !video.window().has_focus() {
                return;
            }

            if let Ok(mut gb) = self.gb.lock() {
                let key = &event.logical_key;

                match event.state {
                    ElementState::Pressed => match key.as_ref() {
                        Key::Character("w") => gb.press(B::Up),
                        Key::Character("a") => gb.press(B::Left),
                        Key::Character("s") => gb.press(B::Down),
                        Key::Character("d") => gb.press(B::Right),
                        Key::Character("k") => gb.press(B::A),
                        Key::Character("l") => gb.press(B::B),
                        Key::Character("n") => gb.press(B::Start),
                        Key::Character("m") => gb.press(B::Select),
                        // System
                        Key::Character("f") => match video.window().fullscreen() {
                            Some(_) => video.window().set_fullscreen(None),
                            None => video
                                .window()
                                .set_fullscreen(Some(Fullscreen::Borderless(None))),
                        },
                        Key::Character("z") => video.cycle_scale_mode(),
                        Key::Named(NamedKey::Space) => {
                            self.audio.toggle().unwrap();
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
                        Key::Character("n") => gb.release(B::Start),
                        Key::Character("m") => gb.release(B::Select),
                        _ => (),
                    },
                }
            }
        }
    }

    fn save_data(&self) {
        if let Ok(mut gb) = self.gb.lock() {
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

impl winit::application::ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
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

        let mut video = pollster::block_on(video::Renderer::new(window, self.scaling)).unwrap();

        video.resize(PhysicalSize {
            width: INIT_WIDTH,
            height: INIT_HEIGHT,
        });

        self.video = Some(video);

        // event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));

        self.audio.resume().unwrap();
    }

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        match cause {
            winit::event::StartCause::ResumeTimeReached { .. } => {
                event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_secs_f64(
                    1.0 / 60.0,
                )));

                if let Some(video) = self.video.as_mut() {
                    if let Ok(gb) = self.gb.lock() {
                        video.update(gb.pixel_data_rgba());
                    }

                    video.window().request_redraw();
                }
            }
            _ => {}
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: window::WindowId,
        event: WindowEvent,
    ) {
        if let Some(video) = self.video.as_mut() {
            match event {
                WindowEvent::Resized(size) => video.resize(size),
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput {
                    event: key_event, ..
                } => self.handle_key(&key_event),
                WindowEvent::RedrawRequested => {
                    use wgpu::SurfaceError::{Lost, OutOfMemory, Outdated, Timeout};

                    match video.render() {
                        Ok(()) => {}
                        Err(Lost | Outdated) => video.on_lost(),
                        Err(OutOfMemory) => event_loop.exit(),
                        Err(Timeout) => eprintln!("Surface timeout"),
                    }
                }
                _ => (),
            }
        }
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.audio.pause().unwrap();
        self.video = None;
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn exiting(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        self.save_data();
        self.audio.pause().unwrap();
        self.video = None;
    }
}
