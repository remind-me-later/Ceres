#![warn(
    clippy::pedantic,
    clippy::nursery,
    // restriction
    clippy::alloc_instead_of_core,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    clippy::clone_on_ref_ptr,
    clippy::decimal_literal_representation,
    clippy::default_union_representation,
    clippy::deref_by_slicing,
    clippy::else_if_without_else,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::exit,
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
    clippy::non_ascii_literal,
    clippy::panic,
    clippy::partial_pub_fields,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    // clippy::std_instead_of_alloc,
    clippy::std_instead_of_core,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_slice,
    clippy::string_to_string,
    clippy::try_err,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern,
    clippy::unseparated_literal_suffix,
    // clippy::unwrap_used,
    clippy::verbose_file_reads,
)]
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

// )]

use {
    ceres_core::{Button, Cartridge, Gb, Model},
    clap::{builder::PossibleValuesParser, Arg, Command},
    sdl2::{
        event::{Event, WindowEvent},
        keyboard::Keycode,
    },
    std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    },
};

mod audio;
mod video;

const CERES_BIN: &str = "ceres";
const CERES_STYLIZED: &str = "Ceres";
const ABOUT: &str = "A (very experimental) Game Boy/Color emulator.";
const AFTER_HELP: &str = "KEY BINDINGS:

    | Gameboy | Emulator  |
    | ------- | --------- |
    | Dpad    | WASD      |
    | A       | K         |
    | B       | L         |
    | Start   | Return    |
    | Select  | Backspace |";

fn main() {
    let args = Command::new(CERES_BIN)
        .bin_name(CERES_BIN)
        .about(ABOUT)
        .after_help(AFTER_HELP)
        .arg(
            Arg::new("file")
                .required(true)
                .help("Game Boy/Color ROM file to emulate.")
                .long_help(
                    "Game Boy/Color ROM file to emulate. Extension doesn't matter, the emulator \
                     will check the file is a valid Game Boy ROM reading its header. Doesn't \
                     accept compressed (zip) files.",
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

    let model_str = args.get_one::<String>("model").unwrap();
    let model = match model_str.as_str() {
        "dmg" => Model::Dmg,
        "mgb" => Model::Mgb,
        "cgb" => Model::Cgb,
        _ => unreachable!(),
    };

    let path = PathBuf::from(args.get_one::<String>("file").unwrap());

    if let Err(err) = run(model, path) {
        println!("Error: {err}");
    }
}

/// # Errors
pub fn run(model: Model, mut path: PathBuf) -> Result<(), String> {
    fn read_file(path: &Path) -> Result<Box<[u8]>, std::io::Error> {
        fs::read(path).map(Vec::into_boxed_slice)
    }

    // initialize cartridge
    let rom = read_file(&path).map_err(|e| e.to_string())?;
    path.set_extension("sav");
    let save_file = read_file(&path).ok();
    let cart = Cartridge::new(rom, save_file).map_err(|e| e.to_string())?;

    let gb = {
        let sample_rate = audio::Renderer::sample_rate();
        Arc::new(Mutex::new(Gb::new(model, sample_rate, cart)))
    };

    let sav_path: PathBuf = path;

    let sdl_context = sdl2::init()?;
    let mut audio = {
        let gb = Arc::clone(&gb);
        audio::Renderer::new(&sdl_context, gb)
    };
    let mut video = video::Renderer::new(&sdl_context);
    let mut event_pump = sdl_context.event_pump()?;
    let mut is_focused = true;

    'running: loop {
        if let Ok(mut gb) = gb.lock() {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => break 'running,
                    Event::KeyDown {
                        keycode: Some(keycode),
                        repeat: false,
                        ..
                    } if is_focused => match keycode {
                        Keycode::W => gb.press(Button::Up),
                        Keycode::A => gb.press(Button::Left),
                        Keycode::S => gb.press(Button::Down),
                        Keycode::D => gb.press(Button::Right),
                        Keycode::K => gb.press(Button::A),
                        Keycode::L => gb.press(Button::B),
                        Keycode::Return => gb.press(Button::Start),
                        Keycode::Backspace => gb.press(Button::Select),
                        _ => (),
                    },
                    Event::KeyUp {
                        keycode: Some(keycode),
                        repeat: false,
                        ..
                    } if is_focused => match keycode {
                        Keycode::W => gb.release(Button::Up),
                        Keycode::A => gb.release(Button::Left),
                        Keycode::S => gb.release(Button::Down),
                        Keycode::D => gb.release(Button::Right),
                        Keycode::K => gb.release(Button::A),
                        Keycode::L => gb.release(Button::B),
                        Keycode::Return => gb.release(Button::Start),
                        Keycode::Backspace => gb.release(Button::Select),
                        _ => (),
                    },
                    Event::Window { win_event, .. } => match win_event {
                        WindowEvent::FocusGained => is_focused = true,
                        WindowEvent::FocusLost => is_focused = false,
                        WindowEvent::Resized(width, height) if width != 0 && height != 0 => {
                            video.resize(width as u32, height as u32);
                        }
                        _ => (),
                    },
                    _ => (),
                }
            }

            video.render(gb.pixel_data_rgb());
        }

        // TODO: sleep better
        std::thread::sleep(core::time::Duration::new(0, 1_000_000_000_u32 / 60));
    }

    // Cleanup
    audio.pause();

    if let Ok(mut gb) = gb.lock() {
        if let Some(save_data) = gb.cartridge().save_data() {
            let mut f = File::create(sav_path).map_err(|e| e.to_string())?;
            f.write_all(save_data).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
