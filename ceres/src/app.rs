use crate::{
    gb_area::GbArea,
    video::{self, State},
    Scaling, CERES_STYLIZED, PX_HEIGHT, PX_WIDTH, SCREEN_MUL, VRAM_PX_HEIGHT, VRAM_PX_WIDTH,
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

struct Windows<'a> {
    main: video::State<'a, { PX_WIDTH }, { PX_HEIGHT }>,
    vram: video::State<'a, { VRAM_PX_WIDTH }, { VRAM_PX_HEIGHT }>,
}

pub struct App<'a> {
    // Config parameters
    project_dirs: directories::ProjectDirs,
    scaling: Scaling,

    // Contexts
    gb_ctx: Option<GbArea>,

    // Rendering
    _audio: ceres_audio::State,
    // NOTE: carries the `Window`, thus it should be dropped after everything else.
    windows: Option<Windows<'a>>,
}

impl App<'_> {
    pub fn new(
        project_dirs: directories::ProjectDirs,
        model: ceres_core::Model,
        rom_path: Option<&Path>,
        scaling: Scaling,
    ) -> anyhow::Result<Self> {
        let audio = ceres_audio::State::new()?;
        let gb_ctx = GbArea::new(model, &project_dirs, rom_path, &audio)?;

        Ok(Self {
            project_dirs,
            gb_ctx: Some(gb_ctx),
            _audio: audio,
            scaling,
            windows: None,
        })
    }

    #[allow(clippy::expect_used)]
    fn handle_key(&mut self, event: &KeyEvent) {
        use {winit::event::ElementState, winit::keyboard::Key};

        if let Some(windows) = &mut self.windows {
            if !windows.main.window().has_focus() {
                return;
            }

            if let ElementState::Pressed = event.state {
                match event.logical_key.as_ref() {
                    Key::Character("f") => match windows.main.window().fullscreen() {
                        Some(_) => windows.main.window().set_fullscreen(None),
                        None => windows
                            .main
                            .window()
                            .set_fullscreen(Some(Fullscreen::Borderless(None))),
                    },
                    Key::Character("z") => {
                        self.scaling = self.scaling.next();
                        windows.main.choose_scale_mode(self.scaling);
                    }
                    Key::Named(NamedKey::Space) => {
                        if let Some(gb_ctx) = &mut self.gb_ctx {
                            if gb_ctx.is_paused() {
                                gb_ctx.resume().expect("Couldn't resume");
                            } else {
                                gb_ctx.pause().expect("Couldn't pause");
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

    fn save_data(&self) -> anyhow::Result<()> {
        if let Some(gb_ctx) = &self.gb_ctx {
            if let Ok(gb) = gb_ctx.gb_lock() {
                if let Some(save_data) = gb.cartridge().save_data() {
                    std::fs::create_dir_all(self.project_dirs.data_dir())
                        .map_err(|e| anyhow::anyhow!("couldn't create data directory: {e}"))?;

                    let sav_file = File::create(
                        self.project_dirs
                            .data_dir()
                            .join(gb_ctx.rom_ident())
                            .with_extension("sav"),
                    );

                    sav_file
                        .map_err(|e| anyhow::anyhow!("couldn't open save file: {e}"))
                        .and_then(|mut f| {
                            f.write_all(save_data).map_err(|e| {
                                anyhow::anyhow!("couldn't save data in save file: {e}")
                            })
                        })?;
                }
            }
        }

        Ok(())
    }
}

impl winit::application::ApplicationHandler for App<'_> {
    #[allow(clippy::expect_used)]
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let main_window_attributes = winit::window::Window::default_attributes()
            .with_title(CERES_STYLIZED)
            .with_inner_size(PhysicalSize {
                width: PX_WIDTH * SCREEN_MUL,
                height: PX_HEIGHT * SCREEN_MUL,
            })
            .with_min_inner_size(PhysicalSize {
                width: ceres_core::PX_WIDTH,
                height: ceres_core::PX_HEIGHT,
            })
            .with_resizable(true)
            .with_active(true);

        let main_window = event_loop
            .create_window(main_window_attributes)
            .expect("Could not create window");
        let main_window_state = pollster::block_on(State::new(main_window, self.scaling))
            .expect("Could not create renderer");

        let vram_window_attributes = winit::window::Window::default_attributes()
            .with_title(format!("{CERES_STYLIZED} - VRAM"))
            .with_inner_size(PhysicalSize {
                width: VRAM_PX_WIDTH * SCREEN_MUL,
                height: VRAM_PX_HEIGHT * SCREEN_MUL,
            })
            .with_min_inner_size(PhysicalSize {
                width: ceres_core::VRAM_PX_WIDTH,
                height: ceres_core::VRAM_PX_HEIGHT,
            })
            .with_resizable(true)
            .with_active(false);

        let vram_window = event_loop
            .create_window(vram_window_attributes)
            .expect("Could not create window");

        let vram_window_state = pollster::block_on(State::new(vram_window, self.scaling))
            .expect("Could not create renderer");

        self.windows.replace(Windows {
            main: main_window_state,
            vram: vram_window_state,
        });

        // event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));

        if let Some(gb_ctx) = &mut self.gb_ctx {
            gb_ctx.resume().expect("Couldn't resume");
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

            if let Some(windows) = self.windows.as_mut() {
                if let Some(gb_ctx) = &self.gb_ctx {
                    if let Ok(gb) = gb_ctx.gb_lock() {
                        windows.main.update_texture(gb.pixel_data_rgba());
                        windows.vram.update_texture(gb.vram_data_rgba());
                    }
                }

                windows.main.window().request_redraw();
                windows.vram.window().request_redraw();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        win_id: window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                if width != 0 && height != 0 {
                    // Some platforms like EGL require resizing GL surface to update the size
                    // Notable platforms here are Wayland and macOS, other don't require it
                    // and the function is no-op, but it's wise to resize it for portability
                    // reasons.
                    if let Some(windows) = self.windows.as_mut() {
                        match win_id {
                            id if id == windows.main.window().id() => {
                                windows.main.resize(PhysicalSize { width, height });
                            }
                            id if id == windows.vram.window().id() => {
                                windows.vram.resize(PhysicalSize { width, height });
                            }
                            _ => (),
                        }
                    }
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => self.handle_key(&key_event),
            WindowEvent::RedrawRequested => {
                use wgpu::SurfaceError::{Lost, Other, OutOfMemory, Outdated, Timeout};

                if let Some(windows) = self.windows.as_mut() {
                    match win_id {
                        id if id == windows.main.window().id() => match windows.main.render() {
                            Ok(()) => {}
                            Err(Lost | Outdated) => windows.main.on_lost(),
                            Err(OutOfMemory) => event_loop.exit(),
                            Err(Timeout) => eprintln!("Surface timeout"),
                            Err(Other) => eprintln!("Surface error: other"),
                        },
                        id if id == windows.vram.window().id() => match windows.vram.render() {
                            Ok(()) => {}
                            Err(Lost | Outdated) => windows.vram.on_lost(),
                            Err(OutOfMemory) => event_loop.exit(),
                            Err(Timeout) => eprintln!("Surface timeout"),
                            Err(Other) => eprintln!("Surface error: other"),
                        },
                        _ => (),
                    }
                }
            }
            WindowEvent::DroppedFile(path) => {
                self.save_data().unwrap_or_else(|e| {
                    eprintln!("Error saving data: {e}");
                });

                if let Some(gb_ctx) = &mut self.gb_ctx {
                    gb_ctx
                        .change_rom(&path, &self.project_dirs)
                        .unwrap_or_else(|e| {
                            eprintln!("Error loading ROM: {e}");
                        });
                }
            }
            _ => (),
        }
    }

    #[allow(clippy::expect_used)]
    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(gb_ctx) = &mut self.gb_ctx {
            gb_ctx.pause().expect("Couldn't pause");
        }
        self.windows = None;
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn exiting(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        self.save_data().unwrap_or_else(|e| {
            eprintln!("Error saving data: {e}");
        });
        self.windows = None;
        self.gb_ctx = None;
    }
}
