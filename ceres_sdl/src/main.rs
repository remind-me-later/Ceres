#![warn(
    clippy::pedantic,
    // clippy::nursery,
    // restriction
    clippy::alloc_instead_of_core,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    clippy::clone_on_ref_ptr,
    clippy::create_dir,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::default_union_representation,
    clippy::deref_by_slicing,
    clippy::else_if_without_else,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::exit,
    clippy::expect_used,
    clippy::filetype_is_file,
    // clippy::float_arithmetic,
    clippy::float_cmp_const,
    clippy::fn_to_numeric_cast_any,
    clippy::format_push_string,
    clippy::get_unwrap,
    clippy::if_then_some_else_none,
    clippy::let_underscore_must_use,
    clippy::lossy_float_literal,
    clippy::map_err_ignore,
    clippy::mem_forget,
    clippy::mixed_read_write_in_expression,
    clippy::modulo_arithmetic,
    clippy::mutex_atomic,
    clippy::non_ascii_literal,
    clippy::panic,
    clippy::partial_pub_fields,
    // clippy::print_stderr,
    // clippy::print_stdout,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_slice,
    clippy::string_to_string,
    clippy::todo,
    clippy::try_err,
    clippy::unimplemented,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern,
    clippy::unseparated_literal_suffix,
    clippy::use_debug,
    clippy::verbose_file_reads,
    // clippy::indexing_slicing,
    // clippy::unwrap_used,
    // clippy::integer_division,
)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::missing_safety_doc,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::verbose_bit_mask
)]

use core::time::Duration;
use std::thread;

use ceres_core::Button;
use parking_lot::Mutex;
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::{Canvas, Texture, TextureCreator},
    video::{FullscreenType, Window, WindowContext},
    EventPump,
};
use {
    alloc::sync::Arc,
    anyhow::Context,
    ceres_core::Gb,
    clap::{builder::PossibleValuesParser, Arg, Command},
    std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
    },
};

mod audio;

extern crate alloc;

const CERES_BIN: &str = "ceres";
const CERES_STYLIZED: &str = "Ceres";
const ABOUT: &str = "A (very experimental) Game Boy/Color emulator.";
const AFTER_HELP: &str = "GB bindings:

    | Gameboy | Emulator  |
    | ------- | --------- |
    | Dpad    | WASD      |
    | A       | K         |
    | B       | L         |
    | Start   | M         |
    | Select  | N         |
    
Other binsings:

    | System       | Emulator |
    | ------------ | -------- |
    | Fullscreen   | F        |
";

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const SCREEN_MUL: u32 = 3;

fn main() -> anyhow::Result<()> {
    let args = Command::new(CERES_BIN)
        .bin_name(CERES_BIN)
        .about(ABOUT)
        .after_help(AFTER_HELP)
        .arg(
            Arg::new("file")
                .required(true)
                .help("Game Boy/Color ROM file to emulate.")
                .long_help(
                    "Game Boy/Color ROM file to emulate. Extension doesn't matter, the \
           emulator will check the file is a valid Game Boy ROM reading its \
           header. Doesn't accept compressed (zip) files.",
                ),
        )
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .help("Game Boy model to emulate")
                .value_parser(PossibleValuesParser::new(["dmg", "mgb", "cgb"]))
                .default_value("cgb")
                .required(false),
        )
        .get_matches();

    let model = {
        let model_str = args
            .get_one::<String>("model")
            .context("couldn't get model string")?;

        let model = match model_str.as_str() {
            "dmg" => ceres_core::Model::Dmg,
            "mgb" => ceres_core::Model::Mgb,
            "cgb" => ceres_core::Model::Cgb,
            _ => unreachable!(),
        };

        model
    };

    let pathbuf = args.get_one::<String>("file").map(PathBuf::from).unwrap();

    let mut emu = Emu::new(model, pathbuf)?;
    emu.run();

    Ok(())
}

struct Emu {
    _audio: audio::Renderer,
    gb: Arc<Mutex<Gb>>,
    rom_path: PathBuf,
    is_focused: bool,
    do_resize: bool,
    event_pump: EventPump,
    canvas: Canvas<Window>,
    texture: Texture,
    _creator: TextureCreator<WindowContext>,
    blit_rect: Rect,
}

impl Emu {
    fn new(model: ceres_core::Model, rom_path: PathBuf) -> anyhow::Result<Self> {
        // Try to create GB before creating window
        let gb = Self::init_gb(model, &rom_path)?;

        let gb = Arc::new(Mutex::new(gb));

        let sdl_context = sdl2::init().unwrap();

        let audio = {
            let gb = Arc::clone(&gb);
            audio::Renderer::new(&sdl_context, gb)
        };

        let event_pump = sdl_context.event_pump().unwrap();

        let video_subsystem = sdl_context.video().unwrap();

        let window = video_subsystem
            .window(
                CERES_STYLIZED,
                PX_WIDTH * SCREEN_MUL,
                PX_HEIGHT * SCREEN_MUL,
            )
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let mut canvas = window.into_canvas().build().unwrap();

        canvas.set_draw_color(Color::BLACK);
        canvas.clear();

        let creator = canvas.texture_creator();
        let texture =
            creator.create_texture_streaming(PixelFormatEnum::RGB24, PX_WIDTH, PX_HEIGHT)?;

        Ok(Self {
            _audio: audio,
            gb,
            rom_path,
            is_focused: true,
            do_resize: true,
            event_pump,
            canvas,
            texture,
            _creator: creator,
            blit_rect: Rect::new(0, 0, 0, 0),
        })
    }

    fn init_gb(model: ceres_core::Model, rom_path: &Path) -> anyhow::Result<Gb> {
        let rom = fs::read(rom_path).map(Vec::into_boxed_slice)?;

        let ram = fs::read(rom_path.with_extension("sav"))
            .map(Vec::into_boxed_slice)
            .ok();

        let cart = ceres_core::Cart::new(rom, ram).context("invalid rom header")?;
        let sample_rate = audio::Renderer::sample_rate();

        Ok(Gb::new(model, sample_rate, cart))
    }

    pub fn run(&mut self) {
        'running: loop {
            {
                let mut gb = self.gb.lock();

                for event in self.event_pump.poll_iter() {
                    match event {
                        Event::Quit { .. } => break 'running,
                        Event::KeyDown {
                            keycode: Some(keycode),
                            repeat: false,
                            ..
                        } if self.is_focused => match keycode {
                            Keycode::W => gb.press(Button::Up),
                            Keycode::A => gb.press(Button::Left),
                            Keycode::S => gb.press(Button::Down),
                            Keycode::D => gb.press(Button::Right),
                            Keycode::K => gb.press(Button::A),
                            Keycode::L => gb.press(Button::B),
                            Keycode::M => gb.press(Button::Start),
                            Keycode::N => gb.press(Button::Select),
                            // system
                            Keycode::F => {
                                let win = self.canvas.window_mut();
                                let fs = win.fullscreen_state();
                                let fs = match fs {
                                    FullscreenType::Off => FullscreenType::Desktop,
                                    FullscreenType::True | FullscreenType::Desktop => {
                                        FullscreenType::Off
                                    }
                                };

                                win.set_fullscreen(fs).unwrap();
                            }
                            _ => (),
                        },
                        Event::KeyUp {
                            keycode: Some(keycode),
                            repeat: false,
                            ..
                        } if self.is_focused => match keycode {
                            Keycode::W => gb.release(Button::Up),
                            Keycode::A => gb.release(Button::Left),
                            Keycode::S => gb.release(Button::Down),
                            Keycode::D => gb.release(Button::Right),
                            Keycode::K => gb.release(Button::A),
                            Keycode::L => gb.release(Button::B),
                            Keycode::M => gb.release(Button::Start),
                            Keycode::N => gb.release(Button::Select),
                            _ => (),
                        },
                        Event::Window { win_event, .. } => match win_event {
                            WindowEvent::FocusGained => self.is_focused = true,
                            WindowEvent::FocusLost => self.is_focused = false,
                            WindowEvent::Resized(..) => {
                                self.do_resize = true;
                            }
                            _ => (),
                        },
                        _ => (),
                    }
                }

                let buf = gb.pixel_data_rgba();
                self.texture
                    .update(None, buf, 3 * PX_WIDTH as usize)
                    .unwrap();

                self.canvas.clear();

                if self.do_resize {
                    let viewport = self.canvas.viewport();

                    let mul =
                        core::cmp::min(viewport.width() / PX_WIDTH, viewport.height() / PX_HEIGHT);
                    let width = PX_WIDTH * mul;
                    let height = PX_HEIGHT * mul;

                    self.blit_rect = Rect::from_center(viewport.center(), width, height);
                    self.do_resize = false;
                }

                self.canvas
                    .copy(&self.texture, None, self.blit_rect)
                    .unwrap();

                self.canvas.present();
            }

            thread::sleep(Duration::from_millis(1000 / 60));
        }

        // Cleanup
        self.save_data();
    }

    fn save_data(&self) {
        let mut gb = self.gb.lock();

        if let Some(save_data) = gb.cartridge().save_data() {
            let sav_path = self.rom_path.with_extension("sav");
            let sav_file = File::create(sav_path);
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
