use ceres_cli::error::Error;
use ceres_core::{Cartridge, Gameboy};
use glfw::ffi::*;
use std::{ffi::CString, os::raw::c_int, time::Instant};

static mut GBGF: *mut CeresGlfw = std::ptr::null_mut();

pub struct CeresGlfw {
    gameboy: Gameboy<ceres_cpal::Callbacks>,
    window: *mut GLFWwindow,
    is_focused: bool,
    video_renderer: ceres_opengl::Renderer<GlfwContextWrapper>,
    audio_renderer: ceres_cpal::Renderer,
    is_gui_paused: bool,
    frame_multiplier: u8,
}

impl CeresGlfw {
    pub fn new(
        model: ceres_core::Model,
        cartridge: ceres_core::Cartridge,
        boot_rom: Option<ceres_core::BootRom>,
    ) -> Result<Self, Error> {
        unsafe {
            if glfwInit() == 0 {
                return Err(Error::new("couldn't initialize GLFW"));
            }

            let window_title = CString::new(ceres_cli::CERES_STR).map_err(Error::new)?;

            let window = glfwCreateWindow(
                ceres_core::SCREEN_WIDTH as i32 * 4,
                ceres_core::SCREEN_HEIGHT as i32 * 4,
                window_title.as_ptr(),
                std::ptr::null_mut() as *mut GLFWmonitor,
                std::ptr::null_mut() as *mut GLFWwindow,
            );

            glfwSetWindowSizeLimits(
                window,
                ceres_core::SCREEN_WIDTH as i32,
                ceres_core::SCREEN_HEIGHT as i32,
                DONT_CARE,
                DONT_CARE,
            );

            let context = GlfwContextWrapper { window };

            let mut width: i32 = 0;
            let mut height: i32 = 0;

            glfwGetWindowSize(window, &mut width as *mut i32, &mut height as *mut i32);

            let video_renderer = ceres_opengl::Renderer::new(context, width as u32, height as u32)
                .map_err(Error::new)?;

            let (audio_renderer, audio_callbacks) =
                ceres_cpal::Renderer::new().map_err(Error::new)?;
            let gameboy = ceres_core::Gameboy::new(model, cartridge, boot_rom, audio_callbacks);

            glfwSwapInterval(1);

            Ok(Self {
                gameboy,
                window,
                is_focused: false,
                video_renderer,
                audio_renderer,
                is_gui_paused: false,
                frame_multiplier: 1,
            })
        }
    }

    extern "C" fn handle_key(
        _window: *mut GLFWwindow,
        key: c_int,
        _scancode: c_int,
        action: c_int,
        _mods: c_int,
    ) {
        use ceres_core::Button;

        unsafe {
            if !(*GBGF).is_focused {
                return;
            }

            if action == PRESS {
                match key {
                    KEY_UP => (*GBGF).gameboy.press(Button::Up),
                    KEY_LEFT => (*GBGF).gameboy.press(Button::Left),
                    KEY_DOWN => (*GBGF).gameboy.press(Button::Down),
                    KEY_RIGHT => (*GBGF).gameboy.press(Button::Right),
                    KEY_Z => (*GBGF).gameboy.press(Button::B),
                    KEY_X => (*GBGF).gameboy.press(Button::A),
                    KEY_ENTER => (*GBGF).gameboy.press(Button::Start),
                    KEY_BACKSPACE => (*GBGF).gameboy.press(Button::Select),
                    KEY_U => {
                        (*GBGF).frame_multiplier = if (*GBGF).frame_multiplier > 1 { 1 } else { 4 }
                    }
                    KEY_SPACE => (*GBGF).pause(),
                    _ => (),
                }
            } else if action == RELEASE {
                match key {
                    KEY_UP => (*GBGF).gameboy.release(Button::Up),
                    KEY_LEFT => (*GBGF).gameboy.release(Button::Left),
                    KEY_DOWN => (*GBGF).gameboy.release(Button::Down),
                    KEY_RIGHT => (*GBGF).gameboy.release(Button::Right),
                    KEY_Z => (*GBGF).gameboy.release(Button::B),
                    KEY_X => (*GBGF).gameboy.release(Button::A),
                    KEY_ENTER => (*GBGF).gameboy.release(Button::Start),
                    KEY_BACKSPACE => (*GBGF).gameboy.release(Button::Select),
                    _ => (),
                }
            }
        }
    }

    extern "C" fn handle_resize(_window: *mut GLFWwindow, width: c_int, height: c_int) {
        unsafe {
            (*GBGF)
                .video_renderer
                .resize_viewport(width as u32, height as u32)
        }
    }

    extern "C" fn handle_focus(_window: *mut GLFWwindow, focus: c_int) {
        unsafe {
            (*GBGF).is_focused = focus != 0;
        }
    }

    fn pause(&mut self) {
        if self.is_gui_paused {
            self.audio_renderer.play();
            self.is_gui_paused = false;
        } else {
            self.audio_renderer.pause();
            self.is_gui_paused = true;
        }
    }

    pub fn run(mut self) -> Cartridge {
        unsafe {
            let mut next_frame = Instant::now();

            GBGF = &mut self;

            glfwSetKeyCallback(self.window, Some(Self::handle_key));
            glfwSetWindowSizeCallback(self.window, Some(Self::handle_resize));
            glfwSetWindowFocusCallback(self.window, Some(Self::handle_focus));

            while glfwWindowShouldClose(self.window) == 0 {
                if self.is_gui_paused {
                    glfwWaitEvents();
                } else {
                    let now = Instant::now();

                    if now >= next_frame {
                        for _ in 1..self.frame_multiplier {
                            self.gameboy.run_frame_but_dont_render()
                        }

                        self.gameboy.run_frame();

                        let pixel_data = std::mem::take(self.gameboy.mut_pixel_data());
                        self.video_renderer.update_texture(pixel_data.rgba());
                        self.video_renderer.draw();
                        next_frame = now + ceres_core::FRAME_DURATION;
                    }

                    glfwPollEvents();
                }
            }
        }

        self.gameboy.take_cartridge()
    }
}

pub struct GlfwContextWrapper {
    window: *mut GLFWwindow,
}

impl ceres_opengl::Context for GlfwContextWrapper {
    fn get_proc_address(&mut self, procname: &str) -> *const std::ffi::c_void {
        let c_str = CString::new(procname).unwrap();
        unsafe { glfwGetProcAddress(c_str.as_ptr()) }
    }

    fn swap_buffers(&mut self) {
        unsafe {
            glfwSwapBuffers(self.window);
        }
    }

    fn make_current(&mut self) {
        unsafe {
            glfwMakeContextCurrent(self.window);
        }
    }
}
