use crate::{
    video::{self, State},
    Scaling, CERES_STYLIZED, PX_HEIGHT, PX_WIDTH, SCREEN_MUL,
};
use anyhow::Context;
use ceres_std::GbThread;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use winit::{
    dpi::PhysicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::ControlFlow,
    window::Fullscreen,
};
use {
    core::time::Duration,
    std::{fs::File, path::Path},
    winit::window,
};

pub struct PainterCallbackImpl {
    buffer: Arc<Mutex<Box<[u8]>>>,
}

impl PainterCallbackImpl {
    pub fn new(buffer: Arc<Mutex<Box<[u8]>>>) -> Self {
        Self { buffer }
    }
}

impl ceres_std::PainterCallback for PainterCallbackImpl {
    fn paint(&self, pixel_data_rgba: &[u8]) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.copy_from_slice(pixel_data_rgba);
        }
    }

    fn request_repaint(&self) {}
}

struct Windows<'a> {
    main: video::State<'a, { PX_WIDTH }, { PX_HEIGHT }>,
}

pub struct App<'a> {
    // Config parameters
    project_dirs: directories::ProjectDirs,
    scaling: Scaling,
    sav_path: Option<std::path::PathBuf>,
    pixel_data_rgba: Arc<Mutex<Box<[u8]>>>,

    // Contexts
    thread: GbThread,

    // NOTE: carries the `Window`, thus it should be dropped after everything else.
    windows: Option<Windows<'a>>,
}

impl App<'_> {
    pub fn new(
        project_dirs: directories::ProjectDirs,
        model: ceres_std::Model,
        rom_path: Option<&Path>,
        scaling: Scaling,
    ) -> anyhow::Result<Self> {
        let sav_path = if let Some(rom_path) = rom_path {
            let file_stem = rom_path.file_stem().context("couldn't get file stem")?;

            Some(
                project_dirs
                    .data_dir()
                    .join(file_stem)
                    .with_extension("sav"),
            )
        } else {
            None
        };

        let pixel_data_rgba = Arc::new(Mutex::new(
            vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
        ));

        let mut thread = GbThread::new(
            model,
            sav_path.as_deref(),
            rom_path,
            PainterCallbackImpl::new(Arc::clone(&pixel_data_rgba)),
        )?;

        thread.resume()?;

        Ok(Self {
            project_dirs,
            thread,
            scaling,
            windows: None,
            sav_path,
            pixel_data_rgba,
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
                    // TODO: keys
                    _ => (),
                }
            }
        }
    }

    fn save_data(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.project_dirs.data_dir())?;
        if let Some(sav_path) = &self.sav_path {
            let sav_file = File::create(sav_path);
            self.thread.save_data(&mut sav_file?)?;
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
                width: ceres_std::PX_WIDTH,
                height: ceres_std::PX_HEIGHT,
            })
            .with_resizable(true)
            .with_active(true);

        let main_window = event_loop
            .create_window(main_window_attributes)
            .expect("Could not create window");
        let main_window_state = pollster::block_on(State::new(main_window, self.scaling))
            .expect("Could not create renderer");

        self.windows.replace(Windows {
            main: main_window_state,
        });

        // event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));

        self.thread.resume().expect("Couldn't resume");
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
                if let Ok(pixel_data_rgba) = self.pixel_data_rgba.lock() {
                    windows.main.update_texture(&pixel_data_rgba);
                }

                windows.main.window().request_redraw();
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
                        _ => (),
                    }
                }
            }
            WindowEvent::DroppedFile(path) => {
                self.save_data().unwrap_or_else(|e| {
                    eprintln!("Error saving data: {e}");
                });

                let sav_path = {
                    let file_stem = path.file_stem().context("couldn't get file stem");

                    let file_stem = if let Ok(file_stem) = file_stem {
                        file_stem.to_string_lossy().to_string()
                    } else {
                        eprintln!("Couldn't get file stem");
                        return;
                    };

                    Some(
                        self.project_dirs
                            .data_dir()
                            .join(file_stem)
                            .with_extension("sav"),
                    )
                };

                self.thread
                    .change_rom(sav_path.as_deref(), &path)
                    .unwrap_or_else(|e| {
                        eprintln!("Error loading ROM: {e}");
                    });
            }
            _ => (),
        }
    }

    #[allow(clippy::expect_used)]
    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.thread.pause().expect("Couldn't pause");

        self.windows = None;
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn exiting(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        self.save_data().unwrap_or_else(|e| {
            eprintln!("Error saving data: {e}");
        });
        self.windows = None;
    }
}
