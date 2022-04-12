use super::audio::{AudioCallbacks, AudioRenderer};
use super::error::Error;
use ceres_core::{BootRom, Cartridge, Gameboy, SCREEN_HEIGHT, SCREEN_WIDTH};
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Scancode;
use sdl2::rect::{Point, Rect};
use std::{path::PathBuf, time::Instant};

pub struct Emulator {
    gameboy: Gameboy<AudioCallbacks>,
    is_focused: bool,
    is_gui_paused: bool,
    sdl_context: sdl2::Sdl,
    audio_renderer: AudioRenderer,
}

impl Emulator {
    pub fn new(
        model: ceres_core::Model,
        cartridge: Cartridge,
        boot_rom: BootRom,
    ) -> Result<Self, Error> {
        let sdl_context = sdl2::init().unwrap();

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
        })
    }

    pub fn run(mut self, sav_path: PathBuf) {
        let mut next_frame = Instant::now();

        let video_subsystem = self.sdl_context.video().unwrap();

        let mut window = video_subsystem
            .window(
                super::CERES_STR,
                SCREEN_WIDTH as u32 * 4,
                SCREEN_HEIGHT as u32 * 4,
            )
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        window
            .set_minimum_size(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
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

        'running: loop {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => break 'running,
                    Event::Window { win_event, .. } => match win_event {
                        WindowEvent::Resized(width, height) => {
                            render_rect = Self::compute_new_size(width as u32, height as u32);
                        }
                        WindowEvent::Close => break 'running,
                        WindowEvent::FocusGained => self.is_focused = true,
                        WindowEvent::FocusLost => self.is_focused = false,
                        _ => (),
                    },
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
        super::save_data(&sav_path, cartridge);
    }

    fn compute_new_size(width: u32, height: u32) -> Rect {
        let multiplier = core::cmp::min(width / SCREEN_WIDTH as u32, height / SCREEN_HEIGHT as u32);
        let surface_width = SCREEN_WIDTH as u32 * multiplier;
        let surface_height = SCREEN_HEIGHT as u32 * multiplier;
        let center_x = width as i32 / 2;
        let center_y = height as i32 / 2;
        let center = Point::new(center_x, center_y);

        Rect::from_center(center, surface_width, surface_height)
    }
}
