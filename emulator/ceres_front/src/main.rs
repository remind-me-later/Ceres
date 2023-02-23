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
    clippy::cast_possible_wrap
)]

// )]

use {
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
        window::WindowBuilder,
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

    pollster::block_on(run(model, path));
}

fn print_err<E>(err: E)
where
    E: core::fmt::Display,
{
    println!("error: {err}");
}

/// # Errors
/// # Panics
#[allow(clippy::too_many_lines)]
pub async fn run(model: Model, mut path: PathBuf) {
    fn read_file(path: &Path) -> Result<Box<[u8]>, std::io::Error> {
        fs::read(path).map(Vec::into_boxed_slice)
    }

    // initialize cartridge
    let rom = read_file(&path).map_err(print_err).unwrap();
    path.set_extension("sav");
    let save_file = read_file(&path).ok();
    let cart = Cartridge::new(rom, save_file).map_err(print_err).unwrap();

    let gb = {
        let sample_rate = audio::Renderer::sample_rate();
        Arc::new(Mutex::new(Gb::new(model, sample_rate, cart)))
    };

    let sav_path: PathBuf = path;

    let mut audio = {
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
    .await;

    video.resize(PhysicalSize {
        width:  init_width as u32,
        height: init_height as u32,
    });

    let mut is_focused = true;
    let mut in_buf = InputBuffer::new();

    event_loop.run(move |event, _, control_flow| match event {
        Event::Resumed => control_flow.set_poll(),
        Event::LoopDestroyed => {
            audio.pause();
            // save
            let mut gb = gb.lock();

            if let Some(save_data) = gb.cartridge().save_data() {
                let mut f = File::create(sav_path.clone())
                    .map_err(|e| e.to_string())
                    .unwrap();
                f.write_all(save_data).map_err(|e| e.to_string()).unwrap();
            }
        }
        Event::WindowEvent { event: e, .. } => match e {
            WindowEvent::Resized(size) => {
                video.resize(size);
            }
            WindowEvent::CloseRequested => control_flow.set_exit(),
            WindowEvent::Focused(f) => is_focused = f,
            WindowEvent::KeyboardInput { input, .. } => {
                if !is_focused {
                    return;
                }

                if let Some(key) = input.virtual_keycode {
                    match input.state {
                        ElementState::Pressed => match key {
                            VKC::W => in_buf.press(Button::Up),
                            VKC::A => in_buf.press(Button::Left),
                            VKC::S => in_buf.press(Button::Down),
                            VKC::D => in_buf.press(Button::Right),
                            VKC::K => in_buf.press(Button::A),
                            VKC::L => in_buf.press(Button::B),
                            VKC::Return => in_buf.press(Button::Start),
                            VKC::Back => in_buf.press(Button::Select),
                            _ => (),
                        },
                        ElementState::Released => match key {
                            VKC::W => in_buf.release(Button::Up),
                            VKC::A => in_buf.release(Button::Left),
                            VKC::S => in_buf.release(Button::Down),
                            VKC::D => in_buf.release(Button::Right),
                            VKC::K => in_buf.release(Button::A),
                            VKC::L => in_buf.release(Button::B),
                            VKC::Return => in_buf.release(Button::Start),
                            VKC::Back => in_buf.release(Button::Select),
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
                // Reconfigure the surface if it's lost or outdated
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => video.on_lost(),
                // The system is out of memory, we should probably quit
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                // We're ignoring timeouts
                Err(wgpu::SurfaceError::Timeout) => println!("Surface timeout"),
            }
        }
        Event::MainEventsCleared => {
            let mut gb = gb.lock();
            in_buf.flush(&mut gb);
            video.update(gb.pixel_data_rgb());
            video.window().request_redraw();
        }
        _ => (),
    });
}

struct InputBuffer {
    press_vec: [ceres_core::Button; 16],
    press_idx: u8,

    rel_vec: [ceres_core::Button; 16],
    rel_idx: u8,
}

impl InputBuffer {
    const fn new() -> Self {
        let press_vec = [Button::A; 16];
        let rel_vec = [Button::A; 16];

        Self {
            press_vec,
            press_idx: 0,

            rel_vec,
            rel_idx: 0,
        }
    }

    fn press(&mut self, button: Button) {
        if self.press_idx >= 16 {
            return;
        }

        self.press_vec[self.press_idx as usize] = button;
        self.press_idx += 1;
    }

    fn release(&mut self, button: Button) {
        if self.rel_idx >= 16 {
            return;
        }

        self.rel_vec[self.rel_idx as usize] = button;
        self.rel_idx += 1;
    }

    fn flush(&mut self, gb: &mut Gb) {
        for i in 0..self.press_idx {
            gb.press(self.press_vec[i as usize]);
        }

        for i in 0..self.rel_idx {
            gb.release(self.rel_vec[i as usize]);
        }

        self.press_idx = 0;
        self.rel_idx = 0;
    }
}
