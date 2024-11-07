use crate::{scene, Scaling};
use ceres_core::{Cart, Gb};
use std::{
    io::Read,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc, Mutex,
    },
};
use thread_priority::ThreadBuilderExt;

pub struct GbWidget {
    scene: scene::Scene,
    rom_ident: String,
    exiting: Arc<AtomicBool>,
    pause_thread: Arc<AtomicBool>,
    audio_stream: ceres_audio::Stream,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl GbWidget {
    pub fn new(
        model: ceres_core::Model,
        project_dirs: &directories::ProjectDirs,
        rom_path: Option<&Path>,
        audio_state: &ceres_audio::State,
    ) -> Self {
        fn gb_loop(
            gb: Arc<Mutex<Gb<ceres_audio::RingBuffer>>>,
            exiting: Arc<AtomicBool>,
            pause_thread: Arc<AtomicBool>,
        ) {
            loop {
                let begin = std::time::Instant::now();

                if exiting.load(Relaxed) {
                    break;
                }

                let duration = std::time::Duration::from_millis(1000 / 60);

                if !pause_thread.load(Relaxed) {
                    if let Ok(mut gb) = gb.lock() {
                        gb.run_frame();
                    }
                }

                let elapsed = begin.elapsed();

                if elapsed < duration {
                    spin_sleep::sleep(duration - elapsed);
                }
                // TODO: we're always running late
                // else {
                //     eprintln!("running late: {elapsed:?}");
                // }
            }

            // FIXME: clippy says we have to drop
            drop(gb);
            drop(exiting);
            drop(pause_thread);
        }

        let (cart, ident) = if let Some(rom_path) = rom_path {
            let rom = {
                std::fs::read(rom_path)
                    .map(Vec::into_boxed_slice)
                    .expect("no such file")
            };

            // TODO: core error
            let mut cart = Cart::new(rom).unwrap();
            let ident = {
                let mut ident = String::new();
                cart.ascii_title().read_to_string(&mut ident).unwrap();
                ident.push('-');
                ident.push_str(cart.version().to_string().as_str());
                ident.push('-');
                ident.push_str(cart.header_checksum().to_string().as_str());
                ident.push('-');
                ident.push_str(cart.global_checksum().to_string().as_str());

                ident
            };

            if let Ok(ram) =
                std::fs::read(project_dirs.data_dir().join(&ident).with_extension("sav"))
                    .map(Vec::into_boxed_slice)
            {
                cart.set_ram(ram).unwrap();
            }

            (cart, ident)
        } else {
            (Cart::default(), String::new())
        };

        let sample_rate = ceres_audio::Stream::sample_rate();
        let mut audio_stream = ceres_audio::Stream::new(audio_state).unwrap();
        let ring_buffer = audio_stream.get_ring_buffer();

        let gb = Arc::new(Mutex::new(Gb::new(model, sample_rate, cart, ring_buffer)));
        audio_stream.resume();

        let pause_thread = Arc::new(AtomicBool::new(false));

        let exiting = Arc::new(AtomicBool::new(false));

        let thread_builder = std::thread::Builder::new().name("gb_loop".to_owned());
        let thread_handle = {
            let gb = Arc::clone(&gb);
            let exit = Arc::clone(&exiting);
            let pause_thread = Arc::clone(&pause_thread);

            // std::thread::spawn(move || gb_loop(gb, exit, pause_thread))
            thread_builder
                .spawn_with_priority(thread_priority::ThreadPriority::Max, move |_| {
                    gb_loop(gb, exit, pause_thread);
                })
                .expect("failed to spawn thread")
        };

        let scene = scene::Scene::new(gb, Scaling::Scale2x);

        Self {
            scene,
            rom_ident: ident,
            exiting,
            pause_thread,
            thread_handle: Some(thread_handle),
            audio_stream,
        }
    }

    // pub fn is_paused(&self) -> bool {
    //     self.pause_thread.load(Relaxed)
    // }

    // pub fn pause(&mut self) {
    //     self.audio_stream.pause();
    //     self.pause_thread.store(true, Relaxed);
    // }

    // pub fn resume(&mut self) {
    //     self.pause_thread.store(false, Relaxed);
    //     self.audio_stream.resume();
    // }

    // pub fn rom_ident(&self) -> &str {
    //     &self.rom_ident
    // }

    pub fn scaling(&self) -> Scaling {
        self.scene.scaling()
    }

    pub fn set_scaling(&mut self, scaling: Scaling) {
        self.scene.set_scaling(scaling);
    }

    pub fn scene(&self) -> &scene::Scene {
        &self.scene
    }
}

impl Drop for GbWidget {
    fn drop(&mut self) {
        self.audio_stream.pause();
        self.exiting.store(true, Relaxed);
        self.thread_handle.take().unwrap().join().unwrap();
    }
}
