#![warn(
  clippy::pedantic,
//   clippy::nursery,
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
//   clippy::float_arithmetic,
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
//   clippy::print_stderr,
//   clippy::print_stdout,
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

use {
    alloc::sync::Arc,
    anyhow::Context,
    ceres_core::Gb,
    clap::{builder::PossibleValuesParser, Arg, Command},
    core::time::Duration,
    std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
    },
    winit::{event_loop, window},
};

mod audio;
mod video;

extern crate alloc;

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
        // TODO: this unwrap should be correct as we assign a default value to the
        // argument
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

    let path = PathBuf::from(
        args.get_one::<String>("file")
            .context("couldn't get file string")?,
    );

    pollster::block_on(run(model, path))
}

#[allow(clippy::too_many_lines)]
pub async fn run(model: ceres_core::Model, mut path: PathBuf) -> anyhow::Result<()> {
    let (gb, sav_path) = {
        fn read_file(path: &Path) -> Result<Vec<u8>, std::io::Error> {
            fs::read(path)
        }

        let rom = read_file(&path).with_context(|| {
            format!(
                "couldn't open rom file in path: '{}'",
                path.to_str().unwrap_or("couldn't get path string")
            )
        })?;
        path.set_extension("sav");
        let ram = read_file(&path).ok();

        let cart = ceres_core::Cart::new(rom, ram)?;

        let gb = {
            let sample_rate = audio::Renderer::sample_rate();
            Arc::new(parking_lot::Mutex::new(Gb::new(model, sample_rate, cart)))
        };

        (gb, path)
    };

    let event_loop = event_loop::EventLoop::new();

    let mut video = {
        use winit::dpi::PhysicalSize;

        const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
        const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
        const MUL: u32 = 4;
        const INIT_WIDTH: u32 = PX_WIDTH * MUL;
        const INIT_HEIGHT: u32 = PX_HEIGHT * MUL;

        let window = window::WindowBuilder::new()
            .with_title(CERES_STYLIZED)
            .with_inner_size(PhysicalSize {
                width: INIT_WIDTH,
                height: INIT_HEIGHT,
            })
            .with_min_inner_size(PhysicalSize {
                width: PX_WIDTH,
                height: PX_HEIGHT,
            })
            .build(&event_loop)
            .context("couldn't create window")?;

        let mut video = video::State::new(window, PX_WIDTH, PX_HEIGHT)
            .await
            .context("couldn't initialize wgpu")?;

        video.resize(PhysicalSize {
            width: INIT_WIDTH,
            height: INIT_HEIGHT,
        });

        video
    };

    let _audio = {
        let gb = Arc::clone(&gb);
        audio::Renderer::new(gb)
    };

    event_loop.run(move |event, _, control_flow| {
        use winit::event::Event;

        match event {
            Event::Resumed => control_flow.set_poll(),
            Event::LoopDestroyed => {
                // save
                let mut gb = gb.lock();

                if let Some(save_data) = gb.cartridge().save_data() {
                    let sav_file = File::create(sav_path.clone());
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
            Event::WindowEvent { event: ref e, .. } => {
                use winit::event::WindowEvent;

                match e {
                    WindowEvent::Resized(size) => {
                        video.resize(*size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        video.resize(**new_inner_size);
                    }
                    WindowEvent::CloseRequested => control_flow.set_exit(),
                    WindowEvent::KeyboardInput { input, .. } if video.window().has_focus() => {
                        if let Some(key) = input.virtual_keycode {
                            use {
                                ceres_core::Button as B,
                                winit::event::{ElementState, VirtualKeyCode as KC},
                            };

                            match input.state {
                                ElementState::Pressed => match key {
                                    KC::W => gb.lock().press(B::Up),
                                    KC::A => gb.lock().press(B::Left),
                                    KC::S => gb.lock().press(B::Down),
                                    KC::D => gb.lock().press(B::Right),
                                    KC::K => gb.lock().press(B::A),
                                    KC::L => gb.lock().press(B::B),
                                    KC::Return => gb.lock().press(B::Start),
                                    KC::Back => gb.lock().press(B::Select),
                                    // System
                                    KC::F => match video.window().fullscreen() {
                                        Some(_) => video.window().set_fullscreen(None),
                                        None => video.window().set_fullscreen(Some(
                                            window::Fullscreen::Borderless(None),
                                        )),
                                    },
                                    KC::P => {
                                        #[cfg(feature = "screenshot")]
                                        {
                                            use core::str::FromStr;

                                            let time = chrono::Local::now();

                                            let mut stem = sav_path.file_stem().unwrap().to_owned();
                                            stem.push(" - ");
                                            stem.push(time.to_string());

                                            let img_path =
                                                PathBuf::from_str(stem.to_str().unwrap())
                                                    .unwrap()
                                                    .with_extension("png");

                                            // println!("{img_path:?}");

                                            let gb = gb.lock();

                                            image::save_buffer_with_format(
                                                img_path,
                                                gb.pixel_data_rgba(),
                                                u32::from(ceres_core::PX_WIDTH),
                                                u32::from(ceres_core::PX_HEIGHT),
                                                image::ColorType::Rgba8,
                                                image::ImageFormat::Png,
                                            )
                                            .unwrap();
                                        }
                                    }
                                    _ => (),
                                },
                                ElementState::Released => match key {
                                    KC::W => gb.lock().release(B::Up),
                                    KC::A => gb.lock().release(B::Left),
                                    KC::S => gb.lock().release(B::Down),
                                    KC::D => gb.lock().release(B::Right),
                                    KC::K => gb.lock().release(B::A),
                                    KC::L => gb.lock().release(B::B),
                                    KC::Return => gb.lock().release(B::Start),
                                    KC::Back => gb.lock().release(B::Select),
                                    _ => (),
                                },
                            }
                        }
                    }

                    _ => (),
                }
            }
            Event::RedrawRequested(window_id) if window_id == video.window().id() => {
                use wgpu::SurfaceError::{Lost, OutOfMemory, Outdated, Timeout};
                match video.render() {
                    Ok(_) => {}
                    Err(Lost | Outdated) => video.on_lost(),
                    Err(OutOfMemory) => control_flow.set_exit(),
                    Err(Timeout) => eprintln!("Surface timeout"),
                }
            }
            Event::MainEventsCleared => {
                video.update(gb.lock().pixel_data_rgba());
                video.window().request_redraw();
                std::thread::sleep(Duration::from_millis(1000 / 60));
            }
            _ => (),
        }
    });
}
