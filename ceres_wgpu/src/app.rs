use crate::{
    audio,
    video::{self, Scaling},
    CERES_STYLIZED, SCREEN_MUL,
};
use parking_lot::Mutex;
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
    gb: Arc<Mutex<Gb>>,
    scaling: Scaling,
    rom_path: PathBuf,
}

impl<'a> App<'a> {
    pub fn new(
        model: ceres_core::Model,
        rom_path: PathBuf,
        scaling: Scaling,
    ) -> anyhow::Result<Self> {
        fn init_audio(gb: &Arc<Mutex<Gb>>) -> anyhow::Result<audio::Renderer> {
            audio::Renderer::new(Arc::clone(gb))
        }

        fn init_gb(model: ceres_core::Model, rom_path: &Path) -> anyhow::Result<Gb> {
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

            Ok(Gb::new(model, sample_rate, cart))
        }

        let gb = Arc::new(Mutex::new(init_gb(model, &rom_path)?));
        let audio = init_audio(&gb)?;

        Ok(Self {
            gb,
            audio,
            scaling,
            rom_path,
            video: None,
        })
    }

    fn handle_key(&mut self, event: &KeyEvent) {
        use {ceres_core::Button as B, winit::event::ElementState, winit::keyboard::Key};

        if let Some(video) = &mut self.video {
            if !video.window().has_focus() {
                return;
            }

            let key = &event.logical_key;

            match event.state {
                ElementState::Pressed => match key.as_ref() {
                    Key::Character("w") => self.gb.lock().press(B::Up),
                    Key::Character("a") => self.gb.lock().press(B::Left),
                    Key::Character("s") => self.gb.lock().press(B::Down),
                    Key::Character("d") => self.gb.lock().press(B::Right),
                    Key::Character("k") => self.gb.lock().press(B::A),
                    Key::Character("l") => self.gb.lock().press(B::B),
                    Key::Character("n") => self.gb.lock().press(B::Start),
                    Key::Character("m") => self.gb.lock().press(B::Select),
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
                    Key::Character("w") => self.gb.lock().release(B::Up),
                    Key::Character("a") => self.gb.lock().release(B::Left),
                    Key::Character("s") => self.gb.lock().release(B::Down),
                    Key::Character("d") => self.gb.lock().release(B::Right),
                    Key::Character("k") => self.gb.lock().release(B::A),
                    Key::Character("l") => self.gb.lock().release(B::B),
                    Key::Character("n") => self.gb.lock().release(B::Start),
                    Key::Character("m") => self.gb.lock().release(B::Select),
                    _ => (),
                },
            }
        }
    }

    fn save_data(&self) {
        let mut gb = self.gb.lock();

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

        event_loop.set_control_flow(ControlFlow::Poll);

        self.audio.resume().unwrap();
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

    fn about_to_wait(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        if let Some(video) = self.video.as_mut() {
            video.update(self.gb.lock().pixel_data_rgba());
            video.window().request_redraw();

            std::thread::sleep(Duration::from_millis(1000 / 60));
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
