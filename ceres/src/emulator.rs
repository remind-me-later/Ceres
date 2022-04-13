use crate::error;

use super::{
    audio::{AudioCallbacks, AudioRenderer},
    error::Error,
};
use ceres_core::{BootRom, Cartridge, Gameboy, RumbleCallbacks, SCREEN_HEIGHT, SCREEN_WIDTH};
use sdl2::{
    controller::GameController,
    event::{Event, WindowEvent},
    keyboard::Scancode,
    rect::{Point, Rect},
    Sdl,
};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

pub struct RumbleWrapper {}

impl RumbleWrapper {
    pub fn new() -> Self {
        Self {}
    }
}

impl RumbleCallbacks for RumbleWrapper {
    fn start_rumble(&mut self) {}

    fn stop_rumble(&mut self) {}
}

pub struct Emulator {
    gameboy: Gameboy<AudioCallbacks, RumbleWrapper>,
    is_focused: bool,
    is_gui_paused: bool,
    sdl_context: Sdl,
    audio_renderer: AudioRenderer,
    sav_path: PathBuf,
}

impl Emulator {
    pub fn new(
        model: ceres_core::Model,
        boot_rom_path: &Path,
        rom_path: &Path,
    ) -> Result<Self, Error> {
        let sdl_context = sdl2::init().unwrap();

        let sav_path = rom_path.with_extension("sav");

        let rumble_wrapper = RumbleWrapper::new();

        let (cartridge, sav_path) = {
            let rom_buf = read_file(&rom_path)
                .unwrap_or_else(|e| error::print(e))
                .into_boxed_slice();

            let ram = if let Ok(sav_buf) = read_file(&sav_path) {
                Some(sav_buf.into_boxed_slice())
            } else {
                None
            };

            let cartridge =
                Cartridge::new(rom_buf, ram, rumble_wrapper).unwrap_or_else(|e| error::print(e));

            (cartridge, sav_path)
        };

        print!("{cartridge}");

        let boot_rom = {
            let boot_rom_buf = read_file(&boot_rom_path)
                .unwrap_or_else(|e| error::print(format!("could not load boot ROM {}", e)))
                .into_boxed_slice();

            BootRom::new(boot_rom_buf)
        };

        let (audio_renderer, audio_callbacks) = AudioRenderer::new(&sdl_context);
        let gameboy = ceres_core::Gameboy::new(
            model,
            cartridge,
            boot_rom,
            audio_callbacks,
            ceres_core::MonochromePaletteColors::Grayscale,
        );

        Ok(Self {
            sdl_context,
            gameboy,
            is_focused: false,
            is_gui_paused: false,
            audio_renderer,
            sav_path,
        })
    }

    fn init_controller(sdl_context: &Sdl) -> Option<GameController> {
        let game_controller_subsystem = sdl_context.game_controller().unwrap();

        let available = game_controller_subsystem
            .num_joysticks()
            .map_err(|e| format!("can't enumerate joysticks: {}", e))
            .unwrap();

        let controller = (0..available).find_map(|id| {
            if !game_controller_subsystem.is_game_controller(id) {
                return None;
            }

            match game_controller_subsystem.open(id) {
                Ok(c) => Some(c),
                Err(_) => None,
            }
        });

        controller
    }

    pub fn run(mut self) {
        let mut next_frame = Instant::now();

        let _controller = Self::init_controller(&self.sdl_context);

        let video_subsystem = self.sdl_context.video().unwrap();

        let window = video_subsystem
            .window(
                super::CERES_STR,
                SCREEN_WIDTH as u32 * 4,
                SCREEN_HEIGHT as u32 * 4,
            )
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let mut canvas = window.into_canvas().build().unwrap();

        let texture_creator = canvas.texture_creator();
        let mut texture = texture_creator
            .create_texture_streaming(
                sdl2::pixels::PixelFormatEnum::RGBA32,
                SCREEN_WIDTH as u32,
                SCREEN_HEIGHT as u32,
            )
            .unwrap();

        let mut event_pump = self.sdl_context.event_pump().unwrap();
        let mut render_rect =
            Self::compute_new_size(SCREEN_WIDTH as u32 * 4, SCREEN_HEIGHT as u32 * 4);

        'main_loop: loop {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => break 'main_loop,
                    Event::Window { win_event, .. } => match win_event {
                        WindowEvent::Resized(width, height) => {
                            render_rect = Self::compute_new_size(width as u32, height as u32);
                        }
                        WindowEvent::Close => break 'main_loop,
                        WindowEvent::FocusGained => self.is_focused = true,
                        WindowEvent::FocusLost => self.is_focused = false,
                        _ => (),
                    },
                    Event::ControllerButtonDown { button, .. } => {
                        use ceres_core::Button;
                        use sdl2::controller;

                        if !self.is_focused {
                            return;
                        }

                        match button {
                            controller::Button::DPadUp => self.gameboy.press(Button::Up),
                            controller::Button::DPadLeft => self.gameboy.press(Button::Left),
                            controller::Button::DPadDown => self.gameboy.press(Button::Down),
                            controller::Button::DPadRight => self.gameboy.press(Button::Right),
                            controller::Button::B => self.gameboy.press(Button::B),
                            controller::Button::A => self.gameboy.press(Button::A),
                            controller::Button::Start => self.gameboy.press(Button::Start),
                            controller::Button::Back => self.gameboy.press(Button::Select),
                            _ => (),
                        }
                    }
                    Event::ControllerButtonUp { button, .. } => {
                        use ceres_core::Button;
                        use sdl2::controller;

                        if !self.is_focused {
                            return;
                        }

                        match button {
                            controller::Button::DPadUp => self.gameboy.release(Button::Up),
                            controller::Button::DPadLeft => self.gameboy.release(Button::Left),
                            controller::Button::DPadDown => self.gameboy.release(Button::Down),
                            controller::Button::DPadRight => self.gameboy.release(Button::Right),
                            controller::Button::B => self.gameboy.release(Button::B),
                            controller::Button::A => self.gameboy.release(Button::A),
                            controller::Button::Start => self.gameboy.release(Button::Start),
                            controller::Button::Back => self.gameboy.release(Button::Select),
                            _ => (),
                        }
                    }
                    Event::KeyDown { scancode, .. } => {
                        use ceres_core::Button;

                        if !self.is_focused {
                            return;
                        }

                        if let Some(key) = scancode {
                            match key {
                                Scancode::W => self.gameboy.press(Button::Up),
                                Scancode::A => self.gameboy.press(Button::Left),
                                Scancode::S => self.gameboy.press(Button::Down),
                                Scancode::D => self.gameboy.press(Button::Right),
                                Scancode::K => self.gameboy.press(Button::B),
                                Scancode::L => self.gameboy.press(Button::A),
                                Scancode::Return => self.gameboy.press(Button::Start),
                                Scancode::Backspace => self.gameboy.press(Button::Select),
                                Scancode::Space => {
                                    if self.is_gui_paused {
                                        self.audio_renderer.play();
                                        self.is_gui_paused = false;
                                    } else {
                                        self.audio_renderer.pause();
                                        self.is_gui_paused = true;
                                    }
                                }
                                _ => (),
                            }
                        }
                    }
                    Event::KeyUp { scancode, .. } => {
                        use ceres_core::Button;

                        if !self.is_focused {
                            return;
                        }

                        if let Some(key) = scancode {
                            match key {
                                Scancode::W => self.gameboy.release(Button::Up),
                                Scancode::A => self.gameboy.release(Button::Left),
                                Scancode::S => self.gameboy.release(Button::Down),
                                Scancode::D => self.gameboy.release(Button::Right),
                                Scancode::K => self.gameboy.release(Button::B),
                                Scancode::L => self.gameboy.release(Button::A),
                                Scancode::Return => self.gameboy.release(Button::Start),
                                Scancode::Backspace => self.gameboy.release(Button::Select),
                                _ => (),
                            }
                        }
                    }

                    _ => (),
                }
            }

            if self.is_gui_paused {
                continue;
            }

            let now = Instant::now();

            if now >= next_frame {
                self.gameboy.run_frame();
                let pixel_data = std::mem::take(self.gameboy.mut_pixel_data());
                let pixel_data = pixel_data.rgba();
                texture
                    .with_lock(None, move |buf, _pitch| {
                        for i in 0..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize * 4 {
                            buf[i] = pixel_data[i];
                        }
                    })
                    .unwrap();
                canvas.clear();
                canvas.copy(&texture, None, render_rect).unwrap();
                canvas.present();

                next_frame = now + ceres_core::FRAME_DURATION;
            }
        }

        let cartridge = self.gameboy.cartridge();
        save_data(&self.sav_path, cartridge);
    }

    fn compute_new_size(width: u32, height: u32) -> Rect {
        let multiplier = core::cmp::min(width / SCREEN_WIDTH as u32, height / SCREEN_HEIGHT as u32);
        let surface_width = SCREEN_WIDTH as u32 * multiplier;
        let surface_height = SCREEN_HEIGHT as u32 * multiplier;
        let center = Point::new(width as i32 / 2, height as i32 / 2);

        Rect::from_center(center, surface_width, surface_height)
    }
}

pub fn save_data<R: RumbleCallbacks>(sav_path: &Path, cartridge: &Cartridge<R>) {
    let mut f = File::create(sav_path)
        .unwrap_or_else(|_| error::print(Error::new("unable to open save file")));

    f.write_all(cartridge.ram())
        .unwrap_or_else(|_| error::print(Error::new("buffer overflow")));
}

fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
    let mut f = File::open(path).map_err(|_| Error::new("no file found"))?;
    let metadata = fs::metadata(&path).map_err(|_| Error::new("unable to read metadata"))?;
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer)
        .map_err(|_| Error::new("buffer overflow"))?;

    Ok(buffer)
}
