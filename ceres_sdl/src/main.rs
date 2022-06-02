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
        fs::{self, File},
        io::{Read, Write},
        path::{Path, PathBuf},
        ptr::null_mut,
    },
};

mod audio;
mod video;

const CERES_STR: &str = "Ceres";
const HELP: &str = "TODO";

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
    let mut emulator = Emu::new(model, &rom_path);

    emulator.run();
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

static mut EMU: *mut Emu = null_mut();
static mut EVENTS: *mut EventPump = null_mut();

pub struct Emu {
    sdl: Sdl,
    gb: Gb,
    has_focus: bool,
    sav_path: PathBuf,
    quit: bool,
    video: video::Renderer,
    audio: audio::Renderer,
}

impl Emu {
    pub fn new(model: Model, rom_path: &Path) -> Box<Self> {
        let sdl = sdl2::init().unwrap();

        let sav_path = rom_path.with_extension("sav");
        let cart = {
            let rom = read_file(rom_path).unwrap();
            let ram = read_file(&sav_path).ok();
            Cartridge::new(rom, ram).unwrap()
        };

        let audio = audio::Renderer::new(&sdl);
        let video = video::Renderer::new(&sdl);
        unsafe {
            let mut boxed = Box::new(sdl.event_pump().unwrap());
            EVENTS = boxed.as_mut();
            Box::leak(boxed);
        }

        let mut gb = Gb::new(model, cart);
        gb.set_ppu_frame_callback(ppu_frame_callback);
        gb.set_sample_rate(audio.sample_rate());
        gb.set_apu_frame_callback(apu_frame_callback);
        gb.set_quit_callback(quit_callback);

        let res = Self {
            sdl,
            gb,
            has_focus: false,
            sav_path,
            video,
            audio,
            quit: false,
        };

        let _controller = res.init_controller();

        let mut boxed = Box::new(res);
        unsafe {
            EMU = boxed.as_mut();
        }

        boxed
    }

    pub fn run(&mut self) {
        self.gb.run_frame();
        unsafe {
            Box::from_raw(EVENTS);
        }
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

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Quit { .. } => self.quit(),
            Event::Window { win_event, .. } => match win_event {
                WindowEvent::Resized(width, height) => {
                    self.video.resize_viewport(width as u32, height as u32)
                }
                WindowEvent::Close => self.quit(),
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

    fn quit(&mut self) {
        self.quit = true;
        if let Some(cart_ram) = self.gb.save_data() {
            let mut f = File::create(self.sav_path.clone()).unwrap();
            f.write_all(cart_ram).unwrap();
        }
    }
}

fn read_file(path: &Path) -> Result<Box<[u8]>, ()> {
    let mut f = File::open(path).map_err(|_| ())?;
    let metadata = fs::metadata(&path).unwrap();
    let mut buffer = vec![0; metadata.len() as usize];
    f.read_exact(&mut buffer).unwrap();
    Ok(buffer.into_boxed_slice())
}

#[inline]
fn apu_frame_callback(l: Sample, r: Sample) {
    unsafe {
        (*EMU).audio.push_frame(l, r);
    }
}

#[inline]
fn ppu_frame_callback(rgba: &[u8]) {
    unsafe {
        for e in (*EVENTS).poll_iter() {
            (*EMU).handle_event(e);
        }

        (*EMU).video.draw_frame(rgba);
    }
}

#[inline]
fn quit_callback() -> bool {
    unsafe { (*EMU).quit }
}
