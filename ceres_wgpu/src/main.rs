use video::Scaling;
use winit::event_loop::EventLoop;
use {
    anyhow::Context,
    clap::{builder::PossibleValuesParser, Arg, Command},
    std::path::PathBuf,
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

    let pathbuf = args
        .get_one::<String>("file")
        .map(PathBuf::from)
        .context("no path provided")?;

    let event_loop = EventLoop::new()?;
    let mut emu = emu::Emu::new(model, pathbuf, scaling)?;
    event_loop.run_app(&mut emu)?;

    Ok(())
}

mod emu {
    use crate::{
        audio,
        video::{self, Scaling},
        CERES_STYLIZED, SCREEN_MUL,
    };
    use parking_lot::Mutex;
    use winit::{
        dpi::PhysicalSize,
        event::{KeyEvent, WindowEvent},
        event_loop::ControlFlow,
        keyboard::NamedKey,
        window::Fullscreen,
    };
    use {
        alloc::sync::Arc,
        anyhow::Context,
        ceres_core::Gb,
        core::time::Duration,
        std::{
            fs::{self, File},
            io::Write,
            path::{Path, PathBuf},
        },
        winit::window,
    };

    const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
    const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
    const INIT_WIDTH: u32 = PX_WIDTH * SCREEN_MUL;
    const INIT_HEIGHT: u32 = PX_HEIGHT * SCREEN_MUL;

    #[derive(Default)]
    pub struct Emu<'a> {
        video: Option<video::Renderer<'a>>,
        audio: Option<audio::Renderer>,
        gb: Option<Arc<Mutex<Gb>>>,
        scaling: Scaling,
        rom_path: PathBuf,
    }

    impl<'a> Emu<'a> {
        pub fn new(
            model: ceres_core::Model,
            rom_path: PathBuf,
            scaling: Scaling,
        ) -> anyhow::Result<Self> {
            // Try to create GB before creating window
            let gb = Arc::new(Mutex::new(init_gb(model, &rom_path)?));
            let audio = init_audio(&gb)?;

            Ok(Self {
                gb: Some(gb),
                audio: Some(audio),
                scaling,
                rom_path,
                ..Default::default()
            })
        }
    }

    fn init_audio(gb: &Arc<Mutex<Gb>>) -> anyhow::Result<audio::Renderer> {
        audio::Renderer::new(Arc::clone(gb))
    }

    fn init_gb(model: ceres_core::Model, rom_path: &Path) -> anyhow::Result<Gb> {
        let rom = {
            fs::read(rom_path)
                .map(Vec::into_boxed_slice)
                .context("no such file")?
        };

        // TODO: similar names is allowed, maybe a bug?
        let ram = fs::read(rom_path.with_extension("sav"))
            .map(Vec::into_boxed_slice)
            .ok();

        // TODO: core error
        let cart = ceres_core::Cart::new(rom, ram).unwrap();

        let sample_rate = audio::Renderer::sample_rate();

        Ok(Gb::new(model, sample_rate, cart))
    }

    fn handle_key(
        event: &KeyEvent,
        video: &mut video::Renderer,
        audio: &mut audio::Renderer,
        gb: &Arc<Mutex<Gb>>,
    ) {
        use {ceres_core::Button as B, winit::event::ElementState, winit::keyboard::Key};

        if !video.window().has_focus() {
            return;
        }

        let key = &event.logical_key;

        match event.state {
            ElementState::Pressed => match key.as_ref() {
                Key::Character("w") => gb.lock().press(B::Up),
                Key::Character("a") => gb.lock().press(B::Left),
                Key::Character("s") => gb.lock().press(B::Down),
                Key::Character("d") => gb.lock().press(B::Right),
                Key::Character("k") => gb.lock().press(B::A),
                Key::Character("l") => gb.lock().press(B::B),
                Key::Character("n") => gb.lock().press(B::Start),
                Key::Character("m") => gb.lock().press(B::Select),
                // System
                Key::Character("f") => match video.window().fullscreen() {
                    Some(_) => video.window().set_fullscreen(None),
                    None => video
                        .window()
                        .set_fullscreen(Some(Fullscreen::Borderless(None))),
                },
                Key::Character("z") => video.cycle_scale_mode(),
                Key::Named(NamedKey::Space) => {
                    audio.toggle().unwrap();
                }
                _ => (),
            },
            ElementState::Released => match key.as_ref() {
                Key::Character("w") => gb.lock().release(B::Up),
                Key::Character("a") => gb.lock().release(B::Left),
                Key::Character("s") => gb.lock().release(B::Down),
                Key::Character("d") => gb.lock().release(B::Right),
                Key::Character("k") => gb.lock().release(B::A),
                Key::Character("l") => gb.lock().release(B::B),
                Key::Character("n") => gb.lock().release(B::Start),
                Key::Character("m") => gb.lock().release(B::Select),
                _ => (),
            },
        }
    }

    fn save_data(gb: &Arc<Mutex<Gb>>, rom_path: &Path) {
        let mut gb = gb.lock();

        if let Some(save_data) = gb.cartridge().save_data() {
            let sav_file = File::create(rom_path.with_extension("sav"));
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

    impl winit::application::ApplicationHandler for Emu<'_> {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            let window_attributes = winit::window::Window::default_attributes()
                .with_title(CERES_STYLIZED)
                .with_inner_size(PhysicalSize {
                    width: INIT_WIDTH,
                    height: INIT_HEIGHT,
                })
                .with_min_inner_size(PhysicalSize {
                    width: PX_WIDTH,
                    height: PX_HEIGHT,
                });

            let window = event_loop.create_window(window_attributes).unwrap();

            let mut video = pollster::block_on(video::Renderer::new(window, self.scaling)).unwrap();

            video.resize(PhysicalSize {
                width: INIT_WIDTH,
                height: INIT_HEIGHT,
            });

            self.video = Some(video);

            event_loop.set_control_flow(ControlFlow::Poll);
        }

        fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            _: window::WindowId,
            event: WindowEvent,
        ) {
            if let Some(video) = self.video.as_mut() {
                if let Some(gb) = self.gb.as_ref() {
                    match event {
                        WindowEvent::Resized(size) => video.resize(size),
                        WindowEvent::CloseRequested => {
                            save_data(gb, &self.rom_path);
                            event_loop.exit();
                        }
                        WindowEvent::KeyboardInput {
                            event: key_event, ..
                        } => {
                            if let Some(audio) = self.audio.as_mut() {
                                handle_key(&key_event, video, audio, gb);
                            }
                        }
                        WindowEvent::RedrawRequested => {
                            use wgpu::SurfaceError::{Lost, OutOfMemory, Outdated, Timeout};
                            match video.render() {
                                Ok(()) => {}
                                Err(Lost | Outdated) => video.on_lost(),
                                Err(OutOfMemory) => event_loop.exit(),
                                Err(Timeout) => eprintln!("Surface timeout"),
                            }
                        }
                        _ => (),
                    }
                }
            }
        }

        fn about_to_wait(&mut self, _: &winit::event_loop::ActiveEventLoop) {
            if let Some(video) = self.video.as_mut() {
                if let Some(gb) = self.gb.as_ref() {
                    video.update(gb.lock().pixel_data_rgba());
                    video.window().request_redraw();
                }
                std::thread::sleep(Duration::from_millis(1000 / 60));
            }
        }
    }
}
