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

use parking_lot::Mutex;
use winit::{
    event::{Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Fullscreen,
};

use crate::video::Scaling;
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
    winit::window,
};

mod audio;
mod video;

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
    | Start   | Return    |
    | Select  | Backspace |
    
Other binsings:

    | System       | Emulator |
    | ------------ | -------- |
    | Fullscreen   | F        |
    | Open file    | O        |
    | Scale filter | Z        |
";

const SCREEN_MUL: u32 = 3;

fn main() -> anyhow::Result<()> {
    let args = Command::new(CERES_BIN)
        .bin_name(CERES_BIN)
        .about(ABOUT)
        .after_help(AFTER_HELP)
        .arg(
            Arg::new("file")
                .required(false)
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
        .arg(
            Arg::new("scaling")
                .short('s')
                .long("scaling")
                .help("Scaling algorithm used")
                .value_parser(PossibleValuesParser::new(["nearest", "scale2x", "scale3x"]))
                .default_value("nearest")
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

    let scaling = {
        let scaling_str = args
            .get_one::<String>("scaling")
            .context("couldn't get scaling string")?;

        let scaling = match scaling_str.as_str() {
            "nearest" => Scaling::Nearest,
            "scale2x" => Scaling::Scale2x,
            "scale3x" => Scaling::Scale3x,
            _ => unreachable!(),
        };

        scaling
    };

    let pathbuf = args.get_one::<String>("file").map(PathBuf::from);

    let emu = pollster::block_on(Emu::new(model, pathbuf, scaling))?;
    emu.run();

    Ok(())
}

struct Emu {
    event_loop: Option<EventLoop<()>>,
    video: video::Renderer,
    audio: audio::Renderer,
    gb: Arc<Mutex<Gb>>,
    rom_path: Option<PathBuf>,
    model: ceres_core::Model,
}

impl Emu {
    async fn new(
        model: ceres_core::Model,
        rom_path: Option<PathBuf>,
        scaling: Scaling,
    ) -> anyhow::Result<Self> {
        async fn init_video(
            event_loop: &EventLoop<()>,
            scaling: Scaling,
        ) -> anyhow::Result<video::Renderer> {
            use winit::dpi::PhysicalSize;

            const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
            const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
            const INIT_WIDTH: u32 = PX_WIDTH * SCREEN_MUL;
            const INIT_HEIGHT: u32 = PX_HEIGHT * SCREEN_MUL;

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
                .build(event_loop)
                .context("couldn't create window")?;

            let mut video = video::Renderer::new(window, scaling)
                .await
                .context("couldn't initialize wgpu")?;

            video.resize(PhysicalSize {
                width: INIT_WIDTH,
                height: INIT_HEIGHT,
            });

            Ok(video)
        }

        fn init_audio(gb: &Arc<Mutex<Gb>>) -> anyhow::Result<audio::Renderer> {
            audio::Renderer::new(Arc::clone(gb))
        }

        // Try to create GB before creating window
        let gb = if let Some(rom_path) = &rom_path {
            Self::init_gb(model, Some(rom_path))?
        } else {
            Self::init_gb(model, None)?
        };

        let gb = Arc::new(Mutex::new(gb));

        let audio = init_audio(&gb)?;

        let event_loop = EventLoop::new();
        let video = init_video(&event_loop, scaling).await?;

        Ok(Self {
            event_loop: Some(event_loop),
            gb,
            video,
            audio,
            rom_path,
            model,
        })
    }

    fn init_gb(model: ceres_core::Model, rom_path: Option<&Path>) -> anyhow::Result<Gb> {
        let rom = rom_path.map(|p| fs::read(p).map(Vec::into_boxed_slice).unwrap());

        let ram = rom_path
            .map(|p| p.with_extension("sav"))
            .and_then(|p| fs::read(p).map(Vec::into_boxed_slice).ok());

        let cart = if let Some(rom) = rom {
            ceres_core::Cart::new(rom, ram).context("invalid rom header")?
        } else {
            ceres_core::Cart::default()
        };

        let sample_rate = audio::Renderer::sample_rate();

        Ok(Gb::new(model, sample_rate, cart))
    }

    fn run(mut self) {
        let event_loop = self.event_loop.take().unwrap();
        event_loop.run(move |event, _, control_flow| {
            self.loop_cb(&event, control_flow);
        });
    }

    fn loop_cb(&mut self, event: &Event<()>, control_flow: &mut ControlFlow) {
        match event {
            Event::Resumed => control_flow.set_poll(),
            Event::LoopDestroyed => self.save_data(),
            Event::WindowEvent { event: ref e, .. } => match e {
                WindowEvent::Resized(size) => self.video.resize(*size),
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    self.video.resize(**new_inner_size);
                }
                WindowEvent::CloseRequested => control_flow.set_exit(),
                WindowEvent::KeyboardInput { input, .. } => self.handle_key(input),
                _ => (),
            },
            Event::RedrawRequested(window_id) if *window_id == self.video.window().id() => {
                use wgpu::SurfaceError::{Lost, OutOfMemory, Outdated, Timeout};
                match self.video.render() {
                    Ok(_) => {}
                    Err(Lost | Outdated) => self.video.on_lost(),
                    Err(OutOfMemory) => control_flow.set_exit(),
                    Err(Timeout) => eprintln!("Surface timeout"),
                }
            }
            Event::MainEventsCleared => {
                self.video.update(self.gb.lock().pixel_data_rgba());
                self.video.window().request_redraw();
                std::thread::sleep(Duration::from_millis(1000 / 60));
            }
            _ => (),
        }
    }

    fn handle_key(&mut self, input: &KeyboardInput) {
        if !self.video.window().has_focus() {
            return;
        }

        if let Some(key) = input.virtual_keycode {
            use {
                ceres_core::Button as B,
                winit::event::{ElementState, VirtualKeyCode as KC},
            };

            match input.state {
                ElementState::Pressed => match key {
                    KC::W => self.gb.lock().press(B::Up),
                    KC::A => self.gb.lock().press(B::Left),
                    KC::S => self.gb.lock().press(B::Down),
                    KC::D => self.gb.lock().press(B::Right),
                    KC::K => self.gb.lock().press(B::A),
                    KC::L => self.gb.lock().press(B::B),
                    KC::Return => self.gb.lock().press(B::Start),
                    KC::Back => self.gb.lock().press(B::Select),
                    // System
                    KC::F => match self.video.window().fullscreen() {
                        Some(_) => self.video.window().set_fullscreen(None),
                        None => self
                            .video
                            .window()
                            .set_fullscreen(Some(Fullscreen::Borderless(None))),
                    },
                    KC::Z => {
                        self.video.cycle_scale_mode();
                    }
                    KC::P => {
                        #[cfg(feature = "screenshot")]
                        self.screenshot();
                    }
                    KC::O => {
                        self.audio.pause();

                        let file = rfd::FileDialog::new()
                            .add_filter("gameboy", &["gb", "gbc"])
                            .pick_file();

                        if let Some(f) = file {
                            let rf = &f;
                            if let Ok(mut gb) = Self::init_gb(self.model, Some(rf)) {
                                let mut lock = self.gb.lock();
                                core::mem::swap(&mut *lock, &mut gb);
                            }
                        }

                        self.audio.resume();
                    }
                    _ => (),
                },
                ElementState::Released => match key {
                    KC::W => self.gb.lock().release(B::Up),
                    KC::A => self.gb.lock().release(B::Left),
                    KC::S => self.gb.lock().release(B::Down),
                    KC::D => self.gb.lock().release(B::Right),
                    KC::K => self.gb.lock().release(B::A),
                    KC::L => self.gb.lock().release(B::B),
                    KC::Return => self.gb.lock().release(B::Start),
                    KC::Back => self.gb.lock().release(B::Select),
                    _ => (),
                },
            }
        }
    }

    fn save_data(&self) {
        let mut gb = self.gb.lock();

        if let Some(save_data) = gb.cartridge().save_data() {
            if let Some(path) = &self.rom_path {
                let sav_path = path.with_extension("sav");
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

    #[cfg(feature = "screenshot")]
    fn screenshot(&self) {
        use core::str::FromStr;

        let time = chrono::Local::now();

        let mut stem = self.sav_path.file_stem().unwrap().to_owned();
        stem.push(" - ");
        stem.push(time.to_string());

        let img_path = PathBuf::from_str(stem.to_str().unwrap())
            .unwrap()
            .with_extension("png");

        // println!("{img_path:?}");

        let gb = self.gb.lock();

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
