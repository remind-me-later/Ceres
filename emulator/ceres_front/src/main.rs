#![warn(
    clippy::pedantic,
    // clippy::nursery,
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
    // clippy::panic,
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
    clippy::cast_possible_wrap,
    // TODO: Weird warning on bytemuck derive
    clippy::extra_unused_type_parameters
)]

use {
    anyhow::Context,
    ceres_core::{Button, Cartridge, Gb, Model},
    clap::{builder::PossibleValuesParser, Arg, Command},
    parking_lot::Mutex,
    std::{
        fs::{self, File},
        io::Write,
        path::{Path, PathBuf},
        sync::Arc,
    },
    winit::{
        dpi::PhysicalSize,
        event::{ElementState, Event, VirtualKeyCode as VKC, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::{Fullscreen, WindowBuilder},
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

    // TODO: this unwrap should be correct as we assign a default value to the
    // argument
    let model_str = args.get_one::<String>("model").unwrap();
    let model = match model_str.as_str() {
        "dmg" => Model::Dmg,
        "mgb" => Model::Mgb,
        "cgb" => Model::Cgb,
        _ => unreachable!(),
    };

    let path = PathBuf::from(args.get_one::<String>("file").unwrap());

    pollster::block_on(run(model, path))
}

/// # Errors
/// # Panics
#[allow(clippy::too_many_lines)]
pub async fn run(model: Model, mut path: PathBuf) -> anyhow::Result<()> {
    fn read_file(path: &Path) -> Result<Box<[u8]>, std::io::Error> {
        fs::read(path).map(Vec::into_boxed_slice)
    }

    // initialize cartridge
    let rom = read_file(&path).with_context(|| {
        format!(
            "couldn't open rom file in path: '{}'",
            path.to_str().unwrap_or("couldn't get path string")
        )
    })?;
    path.set_extension("sav");
    let save_file = read_file(&path).ok();
    let cart = Cartridge::new(rom, save_file).context("invalid cartridge")?;

    let gb = {
        let sample_rate = audio::Renderer::sample_rate();
        Arc::new(Mutex::new(Gb::new(model, sample_rate, cart)))
    };

    let sav_path: PathBuf = path;

    let _audio = {
        let gb = Arc::clone(&gb);
        audio::Renderer::new(gb)
    };

    let event_loop = EventLoop::new();

    let init_width = i32::from(ceres_core::PX_WIDTH) * 4;
    let init_height = i32::from(ceres_core::PX_HEIGHT) * 4;

    let window = WindowBuilder::new()
        .with_title(CERES_STYLIZED)
        .with_inner_size(PhysicalSize {
            width:  init_width,
            height: init_height,
        })
        .with_min_inner_size(PhysicalSize {
            width:  i32::from(ceres_core::PX_WIDTH),
            height: i32::from(ceres_core::PX_HEIGHT),
        })
        .build(&event_loop)
        .unwrap();

    let mut video = video::State::new(
        window,
        u32::from(ceres_core::PX_WIDTH),
        u32::from(ceres_core::PX_HEIGHT),
    )
    .await
    .context("couldn't initialize wgpu")?;

    video.resize(PhysicalSize {
        width:  init_width as u32,
        height: init_height as u32,
    });

    event_loop.run(move |event, _, control_flow| match event {
        Event::Resumed => control_flow.set_poll(),
        Event::LoopDestroyed => {
            // save
            let mut gb = gb.lock();

            if let Some(save_data) = gb.cartridge().save_data() {
                let mut f = File::create(sav_path.clone())
                    .map_err(|e| e.to_string())
                    .unwrap();
                f.write_all(save_data).map_err(|e| e.to_string()).unwrap();
            }
        }
        Event::WindowEvent { event: ref e, .. } => match e {
            WindowEvent::Resized(size) => {
                video.resize(*size);
            }
            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                // new_inner_size is &mut so w have to dereference it twice
                video.resize(**new_inner_size);
            }
            WindowEvent::CloseRequested => control_flow.set_exit(),
            WindowEvent::KeyboardInput { input, .. } => {
                if !video.window().has_focus() {
                    return;
                }

                if let Some(key) = input.virtual_keycode {
                    match input.state {
                        ElementState::Pressed => match key {
                            VKC::W => gb.lock().press(Button::Up),
                            VKC::A => gb.lock().press(Button::Left),
                            VKC::S => gb.lock().press(Button::Down),
                            VKC::D => gb.lock().press(Button::Right),
                            VKC::K => gb.lock().press(Button::A),
                            VKC::L => gb.lock().press(Button::B),
                            VKC::Return => gb.lock().press(Button::Start),
                            VKC::Back => gb.lock().press(Button::Select),
                            // System
                            VKC::F => match video.window().fullscreen() {
                                Some(_) => video.window().set_fullscreen(None),
                                None => video
                                    .window()
                                    .set_fullscreen(Some(Fullscreen::Borderless(None))),
                            },
                            _ => (),
                        },
                        ElementState::Released => match key {
                            VKC::W => gb.lock().release(Button::Up),
                            VKC::A => gb.lock().release(Button::Left),
                            VKC::S => gb.lock().release(Button::Down),
                            VKC::D => gb.lock().release(Button::Right),
                            VKC::K => gb.lock().release(Button::A),
                            VKC::L => gb.lock().release(Button::B),
                            VKC::Return => gb.lock().release(Button::Start),
                            VKC::Back => gb.lock().release(Button::Select),
                            _ => (),
                        },
                    }
                }
            }
            _ => (),
        },
        Event::RedrawRequested(window_id) if window_id == video.window().id() => {
            match video.render() {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => video.on_lost(),
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                Err(wgpu::SurfaceError::Timeout) => println!("Surface timeout"),
            }
        }
        Event::MainEventsCleared => {
            video.update(gb.lock().pixel_data_rgb());
            video.window().request_redraw();
        }
        _ => (),
    });
}
