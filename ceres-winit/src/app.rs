use crate::{
    CERES_STYLIZED, CeresEvent,
    video::{self, State},
};
use anyhow::Context;
use ceres_std::wgpu_renderer;
use ceres_std::{GbThread, ShaderOption};
use ceres_std::{PX_HEIGHT, PX_WIDTH};
use std::time::Instant;
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

#[cfg(target_os = "macos")]
use std::path::PathBuf;

struct Windows<'a> {
    main: video::State<'a, { PX_WIDTH as u32 }, { PX_HEIGHT as u32 }>,
}

pub struct App<'a> {
    // Config parameters
    pixel_data_rgba: Box<[u8]>,
    pixel_perfect: bool,
    project_dirs: directories::ProjectDirs,
    #[cfg(target_os = "macos")]
    rom_path: Option<PathBuf>,
    sav_path: Option<std::path::PathBuf>,
    shader_option: ShaderOption,
    thread: GbThread,
    // NOTE: carries the `Window`, thus it should be dropped after everything else.
    windows: Option<Windows<'a>>,
}

impl App<'_> {
    #[allow(clippy::expect_used)]
    fn handle_key(&mut self, event: &KeyEvent) {
        use {winit::event::ElementState, winit::keyboard::Key};

        if let Some(windows) = &mut self.windows {
            if !windows.main.window().has_focus() {
                return;
            }

            if event.state == ElementState::Pressed {
                match event.logical_key.as_ref() {
                    Key::Character("f") => match windows.main.window().fullscreen() {
                        Some(_) => windows.main.window().set_fullscreen(None),
                        None => windows
                            .main
                            .window()
                            .set_fullscreen(Some(Fullscreen::Borderless(None))),
                    },
                    Key::Character("Escape") => {
                        windows.main.window().set_fullscreen(None);
                    }
                    _ => (),
                }
            }

            self.thread.press_release(|p| {
                match event.state {
                    ElementState::Pressed => match event.logical_key.as_ref() {
                        Key::Character("a") => p.press(ceres_std::Button::Left),
                        Key::Character("d") => p.press(ceres_std::Button::Right),
                        Key::Character("w") => p.press(ceres_std::Button::Up),
                        Key::Character("s") => p.press(ceres_std::Button::Down),
                        Key::Character("l") => p.press(ceres_std::Button::A),
                        Key::Character("k") => p.press(ceres_std::Button::B),
                        Key::Character("n") => p.press(ceres_std::Button::Select),
                        Key::Character("m") => p.press(ceres_std::Button::Start),
                        _ => return false,
                    },
                    ElementState::Released => match event.logical_key.as_ref() {
                        Key::Character("a") => p.release(ceres_std::Button::Left),
                        Key::Character("d") => p.release(ceres_std::Button::Right),
                        Key::Character("w") => p.release(ceres_std::Button::Up),
                        Key::Character("s") => p.release(ceres_std::Button::Down),
                        Key::Character("l") => p.release(ceres_std::Button::A),
                        Key::Character("k") => p.release(ceres_std::Button::B),
                        Key::Character("n") => p.release(ceres_std::Button::Select),
                        Key::Character("m") => p.release(ceres_std::Button::Start),
                        _ => return false,
                    },
                }

                true
            });
        }
    }

    pub fn new(
        project_dirs: directories::ProjectDirs,
        model: ceres_std::Model,
        rom_path: Option<&Path>,
        shader_option: ShaderOption,
        pixel_perfect: bool,
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

        let pixel_data_rgba = vec![0u8; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice();

        let mut thread = GbThread::new(model, sav_path.as_deref(), rom_path)?;

        thread.resume()?;

        Ok(Self {
            project_dirs,
            thread,
            shader_option,
            windows: None,
            sav_path,
            pixel_data_rgba,
            pixel_perfect,
            #[cfg(target_os = "macos")]
            rom_path: rom_path.map(std::path::Path::to_path_buf),
        })
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

impl winit::application::ApplicationHandler<CeresEvent> for App<'_> {
    fn exiting(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        self.save_data().unwrap_or_else(|e| {
            eprintln!("Error saving data: {e}");
        });
        self.windows = None;
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
                let _ = self.thread.copy_pixel_data_rgba(&mut self.pixel_data_rgba);
                windows.main.update_texture(&self.pixel_data_rgba);
                windows.main.window().request_redraw();
            }
        }
    }

    #[allow(clippy::expect_used)]
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let main_window_attributes = winit::window::Window::default_attributes()
            .with_title(CERES_STYLIZED)
            .with_inner_size(PhysicalSize {
                width: ceres_std::PX_WIDTH,
                height: ceres_std::PX_HEIGHT,
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

        #[cfg(target_os = "macos")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

            if let Ok(RawWindowHandle::AppKit(handle)) =
                main_window.window_handle().map(|h| h.as_raw())
            {
                let ns_view = handle.ns_view.as_ptr().cast::<objc2::runtime::AnyObject>();

                crate::macos::setup_ns_view(ns_view);
            }
        }

        let main_window_state = pollster::block_on(State::new(
            main_window,
            self.shader_option,
            self.pixel_perfect,
        ))
        .expect("Could not create renderer");

        self.windows.replace(Windows {
            main: main_window_state,
        });

        // event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now()));

        self.thread.resume().expect("Couldn't resume");
    }

    #[allow(clippy::expect_used)]
    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.thread.pause().expect("Couldn't pause");

        self.windows = None;
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    #[cfg(target_os = "macos")]
    fn user_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, event: CeresEvent) {
        match event {
            CeresEvent::ChangeShader(shader_option) => {
                self.shader_option = shader_option;
                // Update shader on the window if needed
                if let Some(windows) = &mut self.windows {
                    windows.main.set_shader(shader_option);
                    windows.main.window().request_redraw();
                }
            }
            CeresEvent::ChangeScaling(scaling_option) => {
                self.scaling_option = scaling_option;
                // Update scaling on the window if needed
                if let Some(windows) = &mut self.windows {
                    windows.main.set_scaling(scaling_option);
                    // Force a resize to apply the new scaling option
                    if let Some(size) = windows.main.window().inner_size().into() {
                        windows.main.resize(size);
                    }
                    windows.main.window().request_redraw();
                }
            }
            CeresEvent::ChangeSpeed(speed_multiplier) => {
                // Set the emulation speed
                self.thread.set_speed_multiplier(speed_multiplier);
            }
            CeresEvent::OpenRomFile => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("GameBoy", &["gbc", "gb"])
                    .pick_file()
                {
                    // First save the current game data if needed
                    self.save_data().unwrap_or_else(|e| {
                        eprintln!("Error saving data: {e}");
                    });

                    self.sav_path = path
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().to_string())
                        .map(|file_stem| {
                            self.project_dirs
                                .data_dir()
                                .join(&file_stem)
                                .with_extension("sav")
                        });

                    self.thread
                        .change_rom(self.sav_path.as_deref(), &path)
                        .unwrap_or_else(|e| {
                            eprintln!("Error loading ROM: {e}");
                        });

                    self.rom_path = Some(path);
                }
            }
            CeresEvent::TogglePause => {
                if let Err(e) = if self.thread.is_paused() {
                    self.thread.resume()
                } else {
                    self.thread.pause()
                } {
                    eprintln!("Error toggling pause: {e}");
                }
            }
            CeresEvent::ChangeModel(model) => {
                // First save the current game data if needed
                self.save_data().unwrap_or_else(|e| {
                    eprintln!("Error saving data: {e}");
                });

                // Load the new ROM
                self.thread
                    .change_model(
                        model.into(),
                        self.sav_path.as_deref(),
                        self.rom_path.as_deref(),
                    )
                    .unwrap_or_else(|e| {
                        eprintln!("Error loading ROM: {e}");
                    });
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
                use wgpu_renderer::wgpu::SurfaceError::{
                    Lost, Other, OutOfMemory, Outdated, Timeout,
                };

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
}
