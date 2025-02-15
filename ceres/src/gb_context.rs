use ceres_audio as audio;
use ceres_core::{Cart, GbBuilder};
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;
use eframe::egui;
use std::{
    fs::File,
    io::Read,
    sync::{Mutex, MutexGuard},
    time::Duration,
};
use thread_priority::ThreadBuilderExt;
use {anyhow::Context, ceres_core::Gb, std::path::Path, std::sync::Arc};

pub struct GbContext {
    gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
    model: ceres_core::Model,
    rom_ident: String,
    exiting: Arc<AtomicBool>,
    pause_thread: Arc<AtomicBool>,
    audio_stream: audio::Stream,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl GbContext {
    pub fn new(
        model: ceres_core::Model,
        project_dirs: &directories::ProjectDirs,
        rom_path: Option<&Path>,
        audio_state: &audio::State,
        ctx: &egui::Context,
    ) -> anyhow::Result<Self> {
        fn gb_loop(
            gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
            exiting: Arc<AtomicBool>,
            pause_thread: Arc<AtomicBool>,
            ctx: &egui::Context,
        ) {
            loop {
                let begin = std::time::Instant::now();

                if exiting.load(Relaxed) {
                    break;
                }

                let duration = if cfg!(target_os = "macos") {
                    Duration::from_millis(1000 / 60)
                } else {
                    ceres_core::FRAME_DURATION
                };

                if !pause_thread.load(Relaxed) {
                    if let Ok(mut gb) = gb.lock() {
                        gb.run_frame();
                    }
                    ctx.request_repaint();
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

        let audio_stream = audio::Stream::new(audio_state)?;
        let ring_buffer = audio_stream.get_ring_buffer();

        let (gb, ident) = Self::create_new_gb(ring_buffer, model, rom_path, project_dirs)?;
        let gb = Arc::new(Mutex::new(gb));

        let pause_thread = Arc::new(AtomicBool::new(false));

        let exiting = Arc::new(AtomicBool::new(false));

        let thread_builder = std::thread::Builder::new().name("gb_loop".to_owned());
        let thread_handle = {
            let gb = Arc::clone(&gb);
            let exit = Arc::clone(&exiting);
            let pause_thread = Arc::clone(&pause_thread);
            let ctx = ctx.clone();

            // std::thread::spawn(move || gb_loop(gb, exit, pause_thread))
            thread_builder.spawn_with_priority(thread_priority::ThreadPriority::Max, move |_| {
                gb_loop(gb, exit, pause_thread, &ctx);
            })?
        };

        // Ok((Gb::new(model, sample_rate, cart, audio_callback), ident))
        Ok(Self {
            gb,
            rom_ident: ident,
            exiting,
            pause_thread,
            thread_handle: Some(thread_handle),
            audio_stream,
            model,
        })
    }

    pub fn change_rom(
        &mut self,
        project_dirs: &directories::ProjectDirs,
        rom_path: &Path,
    ) -> anyhow::Result<()> {
        let ring_buffer = self.audio_stream.get_ring_buffer();

        let (gb_new, ident) =
            Self::create_new_gb(ring_buffer, self.model, Some(rom_path), project_dirs)?;

        if let Ok(mut gb) = self.gb.lock() {
            self.rom_ident = ident;
            *gb = gb_new;
        }

        Ok(())
    }

    fn create_new_gb(
        ring_buffer: audio::RingBuffer,
        model: ceres_core::Model,
        rom_path: Option<&Path>,
        project_dirs: &directories::ProjectDirs,
    ) -> anyhow::Result<(Gb<audio::RingBuffer>, String)> {
        let sample_rate = audio::Stream::sample_rate();

        if let Some(rom_path) = rom_path {
            let rom = {
                std::fs::read(rom_path)
                    .map(Vec::into_boxed_slice)
                    .context("no such file")?
            };

            #[expect(clippy::unwrap_used)]
            let cart = Cart::new(rom).unwrap();

            #[expect(clippy::unwrap_used)]
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

            let gb_builder = GbBuilder::new(model, sample_rate, cart, ring_buffer);

            let save_file = project_dirs.data_dir().join(&ident).with_extension("sav");
            if let Ok(mut save_data) = File::open(&save_file) {
                let gb = gb_builder.load_save_data(&mut save_data)?.build();
                Ok((gb, ident))
            } else {
                Ok((gb_builder.build(), ident))
            }
        } else {
            Ok((
                GbBuilder::new(model, sample_rate, Cart::default(), ring_buffer).build(),
                String::from("bootrom"),
            ))
        }
    }

    #[expect(clippy::unwrap_used)]
    pub fn mut_gb(&mut self) -> MutexGuard<Gb<audio::RingBuffer>> {
        self.gb.lock().unwrap()
    }

    pub fn is_paused(&self) -> bool {
        self.pause_thread.load(Relaxed)
    }

    #[expect(clippy::unwrap_used)]
    pub fn pause(&mut self) {
        self.audio_stream.pause().unwrap();
        self.pause_thread.store(true, Relaxed);
    }

    #[expect(clippy::unwrap_used)]
    pub fn resume(&mut self) {
        self.pause_thread.store(false, Relaxed);
        self.audio_stream.resume().unwrap();
    }

    pub fn rom_ident(&self) -> &str {
        &self.rom_ident
    }

    #[expect(clippy::unwrap_used)]
    pub fn exit(&mut self) {
        self.exiting.store(true, Relaxed);
        self.thread_handle.take().unwrap().join().unwrap();
    }

    #[expect(clippy::unwrap_used)]
    pub fn gb_lock(&self) -> MutexGuard<Gb<audio::RingBuffer>> {
        self.gb.lock().unwrap()
    }

    pub fn gb_clone(&self) -> Arc<Mutex<Gb<audio::RingBuffer>>> {
        Arc::clone(&self.gb)
    }

    pub fn volume(&self) -> &Arc<Mutex<f32>> {
        self.audio_stream.volume()
    }
}

impl Drop for GbContext {
    #[expect(clippy::unwrap_used)]
    fn drop(&mut self) {
        self.audio_stream.pause().unwrap();
        self.exit();
    }
}
