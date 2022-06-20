#![warn(
    clippy::pedantic,
    clippy::as_underscore,
    clippy::clone_on_ref_ptr,
    clippy::decimal_literal_representation,
    clippy::deref_by_slicing,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::float_cmp_const,
    clippy::fn_to_numeric_cast_any,
    clippy::get_unwrap,
    clippy::if_then_some_else_none,
    clippy::let_underscore_must_use,
    clippy::lossy_float_literal,
    clippy::map_err_ignore,
    clippy::mem_forget,
    clippy::mixed_read_write_in_expression,
    clippy::modulo_arithmetic,
    clippy::non_ascii_literal,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_slice,
    clippy::string_to_string,
    clippy::try_err,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern
)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use {
    ceres_core::{Cartridge, Gb, Model, Sample},
    pico_args::Arguments,
    sdl2::{
        controller::GameController,
        event::{Event, WindowEvent},
        keyboard::Scancode,
        EventPump, Sdl,
    },
    std::{
        fs::File,
        io::{Read, Write},
        mem::MaybeUninit,
        path::{Path, PathBuf},
        time::Instant,
    },
};

mod audio;
mod video;

const CERES_STR: &str = "Ceres";
const HELP: &str = "TODO";

static mut EMU: MaybeUninit<Emu> = MaybeUninit::uninit();

fn main() {
    let args = match parse_args() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let model = args.model.map_or(Model::Cgb, |s| match s.as_str() {
        "dmg" => Model::Dmg,
        "mgb" => Model::Mgb,
        "cgb" => Model::Cgb,
        _ => panic!("invalid model"),
    });

    let rom_path = Path::new(&args.rom).to_path_buf();
    let emu = Emu::init(model, &rom_path);
    emu.run();
}

struct AppArgs {
    rom: String,
    model: Option<String>,
}

fn parse_args() -> Result<AppArgs, pico_args::Error> {
    let mut pargs = Arguments::from_env();

    // Help has a higher priority and should be handled
    // separately.
    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP);
        std::process::exit(0);
    }

    let args = AppArgs {
        // Parses an optional value that implements `FromStr`.
        model: pargs.opt_value_from_str(["-m", "--model"])?,
        // Parses an optional value from `&str` using a specified function.
        rom: pargs.free_from_str()?,
    };

    // FIXME: It's up to the caller what to do with the
    // remaining arguments.
    let remaining = pargs.finish();
    if !remaining.is_empty() {
        eprintln!("Warning: unused arguments left: {:?}.", remaining);
    }

    Ok(args)
}

pub struct Emu {
    sdl: Sdl,
    events: EventPump,
    gb: Gb,
    has_focus: bool,
    sav_path: PathBuf,
    video: video::Renderer,
    audio: audio::Renderer,
    last_frame: Instant,
    fullscreen: bool,
}

impl Emu {
    /// # Panics
    ///
    /// Will panic on invalid rom or ram file
    pub fn init(model: Model, rom_path: &Path) -> &'static mut Self {
        let sdl = sdl2::init().unwrap();

        let sav_path = rom_path.with_extension("sav");
        let cart = {
            fn read_file_into(path: &Path, buf: &mut [u8]) -> Result<(), std::io::Error> {
                let mut f = File::open(path)?;
                let _ = f.read(buf).unwrap();
                Ok(())
            }

            let cart = Cartridge::unique();
            read_file_into(rom_path, cart.mut_rom()).unwrap();
            read_file_into(&sav_path, cart.mut_ram()).ok();

            cart.init().unwrap();
            cart
        };

        let audio = audio::Renderer::new(&sdl);
        let video = video::Renderer::new(&sdl);
        let events = sdl.event_pump().unwrap();

        let gb = Gb::new(
            model,
            cart,
            ppu_frame_callback,
            apu_frame_callback,
            audio.sample_rate(),
        );

        let res = Self {
            sdl,
            events,
            gb,
            has_focus: false,
            sav_path,
            video,
            audio,
            last_frame: Instant::now(),
            fullscreen: false,
        };

        let _controller = res.init_controller();

        unsafe {
            EMU.write(res);
            EMU.assume_init_mut()
        }
    }

    #[inline]
    pub fn run(&mut self) -> ! {
        self.gb.run();
    }

    fn init_controller(&self) -> Option<GameController> {
        let gcs = self.sdl.game_controller().unwrap();
        let avail = gcs.num_joysticks().unwrap();

        (0..avail).find_map(|id| {
            gcs.is_game_controller(id)
                .then(|| gcs.open(id).ok())
                .flatten()
        })
    }

    fn handle_events(&mut self) {
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => self.quit(),
                Event::Window { win_event, .. } => match win_event {
                    WindowEvent::Resized(width, height) => {
                        self.video.resize_viewport(width as u32, height as u32);
                    }
                    WindowEvent::FocusGained => self.has_focus = true,
                    WindowEvent::FocusLost => self.has_focus = false,
                    _ => (),
                },
                Event::ControllerButtonDown { button, .. } => {
                    use {ceres_core::Button, sdl2::controller};

                    if !self.has_focus {
                        return;
                    }

                    match button {
                        controller::Button::DPadUp => self.gb.press(Button::Up),
                        controller::Button::DPadLeft => self.gb.press(Button::Left),
                        controller::Button::DPadDown => self.gb.press(Button::Down),
                        controller::Button::DPadRight => self.gb.press(Button::Right),
                        controller::Button::B => self.gb.press(Button::B),
                        controller::Button::A => self.gb.press(Button::A),
                        controller::Button::Start => self.gb.press(Button::Start),
                        controller::Button::Back => self.gb.press(Button::Select),
                        _ => (),
                    }
                }
                Event::ControllerButtonUp { button, .. } => {
                    use {ceres_core::Button, sdl2::controller};

                    if !self.has_focus {
                        return;
                    }

                    match button {
                        controller::Button::DPadUp => self.gb.release(Button::Up),
                        controller::Button::DPadLeft => self.gb.release(Button::Left),
                        controller::Button::DPadDown => self.gb.release(Button::Down),
                        controller::Button::DPadRight => self.gb.release(Button::Right),
                        controller::Button::B => self.gb.release(Button::B),
                        controller::Button::A => self.gb.release(Button::A),
                        controller::Button::Start => self.gb.release(Button::Start),
                        controller::Button::Back => self.gb.release(Button::Select),
                        _ => (),
                    }
                }
                Event::KeyDown { scancode, .. } => {
                    use ceres_core::Button;

                    if !self.has_focus {
                        return;
                    }

                    if let Some(key) = scancode {
                        match key {
                            Scancode::W => self.gb.press(Button::Up),
                            Scancode::A => self.gb.press(Button::Left),
                            Scancode::S => self.gb.press(Button::Down),
                            Scancode::D => self.gb.press(Button::Right),
                            Scancode::K => self.gb.press(Button::A),
                            Scancode::L => self.gb.press(Button::B),
                            Scancode::Return => self.gb.press(Button::Start),
                            Scancode::Backspace => self.gb.press(Button::Select),
                            // Gui
                            Scancode::F => {
                                self.fullscreen = !self.fullscreen;
                                self.video.toggle_fullscreen(self.fullscreen);
                            }
                            _ => (),
                        }
                    }
                }
                Event::KeyUp { scancode, .. } => {
                    use ceres_core::Button;

                    if !self.has_focus {
                        return;
                    }

                    if let Some(key) = scancode {
                        match key {
                            Scancode::W => self.gb.release(Button::Up),
                            Scancode::A => self.gb.release(Button::Left),
                            Scancode::S => self.gb.release(Button::Down),
                            Scancode::D => self.gb.release(Button::Right),
                            Scancode::K => self.gb.release(Button::A),
                            Scancode::L => self.gb.release(Button::B),
                            Scancode::Return => self.gb.release(Button::Start),
                            Scancode::Backspace => self.gb.release(Button::Select),
                            _ => (),
                        }
                    }
                }
                _ => (),
            }
        }
    }

    fn quit(&mut self) -> ! {
        if let Some(cart_ram) = self.gb.save_data() {
            let mut f = File::create(self.sav_path.clone()).unwrap();
            f.write_all(cart_ram).unwrap();
        }

        std::process::exit(0);
    }
}

#[inline]
fn apu_frame_callback(l: Sample, r: Sample) {
    let emu = unsafe { EMU.assume_init_mut() };
    emu.audio.push_frame(l, r);
}

#[inline]
fn ppu_frame_callback(rgba: *const u8) {
    let emu = unsafe { EMU.assume_init_mut() };

    emu.handle_events();

    let elapsed = emu.last_frame.elapsed();
    if elapsed < ceres_core::FRAME_DUR {
        std::thread::sleep(ceres_core::FRAME_DUR - elapsed);
        emu.last_frame = Instant::now();
    }

    emu.video.draw_frame(rgba);
}
