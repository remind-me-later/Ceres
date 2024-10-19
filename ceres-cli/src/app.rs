use crate::{
    gb_context::GbContext,
    video::{self, State},
    Scaling, CERES_STYLIZED, INIT_HEIGHT, INIT_WIDTH,
};
use std::time::Instant;
use winit::{
    dpi::PhysicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::ControlFlow,
    keyboard::NamedKey,
    window::Fullscreen,
};
use {
    core::time::Duration,
    std::{fs::File, io::Write, path::Path},
    winit::window,
};

pub struct App<'a> {
    // Config parameters
    project_dirs: directories::ProjectDirs,
    scaling: Scaling,

    // Contexts
    gb_ctx: Option<GbContext>,

    // Rendering
    _audio: ceres_audio::State,
    // NOTE: carries the `Window`, thus it should be dropped after everything else.
    video: Option<video::State<'a>>,
}

impl<'a> App<'a> {
    pub fn new(
        project_dirs: directories::ProjectDirs,
        model: ceres_core::Model,
        rom_path: Option<&Path>,
        scaling: Scaling,
    ) -> anyhow::Result<Self> {
        let audio = ceres_audio::State::new().unwrap();
        let gb_ctx = GbContext::new(model, &project_dirs, rom_path, &audio)?;

        Ok(Self {
            project_dirs,
            gb_ctx: Some(gb_ctx),
            _audio: audio,
            scaling,
            video: None,
        })
    }

    fn handle_key(&mut self, event: &KeyEvent) {
        use {winit::event::ElementState, winit::keyboard::Key};

        if let Some(video) = &mut self.video {
            if !video.window().has_focus() {
                return;
            }

            if let ElementState::Pressed = event.state {
                match event.logical_key.as_ref() {
                    Key::Character("f") => match video.window().fullscreen() {
                        Some(_) => video.window().set_fullscreen(None),
                        None => video
                            .window()
                            .set_fullscreen(Some(Fullscreen::Borderless(None))),
                    },
                    Key::Character("z") => {
                        self.scaling = self.scaling.next();
                        video.choose_scale_mode(self.scaling);
                    }
                    Key::Named(NamedKey::Space) => {
                        if let Some(gb_ctx) = &mut self.gb_ctx {
                            if gb_ctx.is_paused() {
                                gb_ctx.resume();
                            } else {
                                gb_ctx.pause();
                            }
                        }
                    }
                    _ => (),
                }
            }
            if let Some(gb_ctx) = &mut self.gb_ctx {
                gb_ctx.handle_key(event);
            }
        }
    }

    fn save_data(&self) {
        if let Some(gb_ctx) = &self.gb_ctx {
            let gb = gb_ctx.gb_lock();
            if let Some(save_data) = gb.cartridge().save_data() {
                std::fs::create_dir_all(self.project_dirs.data_dir())
                    .expect("couldn't create data directory");
                let sav_file = File::create(
                    self.project_dirs
                        .data_dir()
                        .join(gb_ctx.rom_ident())
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
        let window_attributes = winit::window::Window::default_attributes()
            .with_title(CERES_STYLIZED)
            .with_inner_size(PhysicalSize {
                width: INIT_WIDTH,
                height: INIT_HEIGHT,
            });

        let window = event_loop.create_window(window_attributes).unwrap();
        let video = pollster::block_on(State::new(window, self.scaling))
            .expect("Could not create renderer");

        self.video.replace(video);

        // event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));

        if let Some(gb_ctx) = &mut self.gb_ctx {
            gb_ctx.resume();
        }
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

            if let Some(video) = self.video.as_mut() {
                if let Some(gb_ctx) = &self.gb_ctx {
                    let gb = gb_ctx.gb_lock();
                    video.update_texture(gb.pixel_data_rgb());
                }

                video.window().request_redraw();
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
                    if let Some(video) = self.video.as_mut() {
                        video.resize(PhysicalSize { width, height });
                    }
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => self.handle_key(&key_event),
            WindowEvent::RedrawRequested => {
                use wgpu::SurfaceError::{Lost, OutOfMemory, Outdated, Timeout};
                if let Some(video) = self.video.as_mut() {
                    match video.render() {
                        Ok(()) => {}
                        Err(Lost | Outdated) => video.on_lost(),
                        Err(OutOfMemory) => event_loop.exit(),
                        Err(Timeout) => eprintln!("Surface timeout"),
                    }
                }
            }
            _ => (),
        }
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(gb_ctx) = &mut self.gb_ctx {
            gb_ctx.pause();
        }
        self.video = None;
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn exiting(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        self.save_data();
        self.video = None;
        self.gb_ctx = None;
    }
}
