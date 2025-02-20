use ceres_audio as audio;
use ceres_core::{Cart, GbBuilder};
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;
use eframe::egui;
use std::{
    fs::File,
    io::Read,
    sync::{LockResult, Mutex, MutexGuard},
    time::Duration,
};
use thread_priority::ThreadBuilderExt;
use {anyhow::Context, ceres_core::Gb, std::path::Path, std::sync::Arc};

pub struct GbThread {
    gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
    model: ceres_core::Model,
    rom_ident: String,
    exiting: Arc<AtomicBool>,
    pause_thread: Arc<AtomicBool>,
    audio_stream: audio::Stream,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl GbThread {
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
            const DURATION: Duration =
                // FIXME: use 16 millis on mac
                if cfg!(target_os = "macos") {
                    Duration::from_millis(1000 / 60)
                } else {
                    ceres_core::FRAME_DURATION
                };

            let mut last_loop = std::time::Instant::now();

            while !exiting.load(Relaxed) {
                if !pause_thread.load(Relaxed) {
                    if let Ok(mut gb) = gb.lock() {
                        gb.run_frame();
                    }
                    ctx.request_repaint();
                }

                let elapsed = last_loop.elapsed();

                if elapsed < DURATION {
                    spin_sleep::sleep(DURATION - elapsed);
                }

                last_loop = std::time::Instant::now();
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

            let cart = Cart::new(rom)?;

            let ident = {
                let mut ident = String::new();
                cart.ascii_title().read_to_string(&mut ident)?;
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
            match File::open(&save_file) { Ok(mut save_data) => {
                let gb = gb_builder.load_save_data(&mut save_data)?.build();
                Ok((gb, ident))
            } _ => {
                Ok((gb_builder.build(), ident))
            }}
        } else {
            Ok((
                GbBuilder::new(model, sample_rate, Cart::default(), ring_buffer).build(),
                String::from("bootrom"),
            ))
        }
    }

    pub fn mut_gb(&mut self) -> LockResult<MutexGuard<Gb<audio::RingBuffer>>> {
        self.gb.lock()
    }

    pub fn is_paused(&self) -> bool {
        self.pause_thread.load(Relaxed)
    }

    pub fn pause(&mut self) -> anyhow::Result<()> {
        self.audio_stream.pause()?;
        self.pause_thread.store(true, Relaxed);
        Ok(())
    }

    pub fn resume(&mut self) -> anyhow::Result<()> {
        self.pause_thread.store(false, Relaxed);
        self.audio_stream.resume()?;
        Ok(())
    }

    pub fn rom_ident(&self) -> &str {
        &self.rom_ident
    }

    pub fn exit(&mut self) -> anyhow::Result<()> {
        self.exiting.store(true, Relaxed);
        self.thread_handle
            .take()
            .ok_or(anyhow::anyhow!("thread_handle is None"))?
            .join()
            .map_err(|e| anyhow::anyhow!("error joining thread: {e:?}"))?;
        Ok(())
    }

    pub fn gb_lock(&self) -> LockResult<MutexGuard<Gb<audio::RingBuffer>>> {
        self.gb.lock()
    }

    pub fn gb_clone(&self) -> Arc<Mutex<Gb<audio::RingBuffer>>> {
        Arc::clone(&self.gb)
    }

    pub fn volume(&self) -> &Arc<Mutex<f32>> {
        self.audio_stream.volume()
    }
}

impl Drop for GbThread {
    fn drop(&mut self) {
        if let Err(e) = self.audio_stream.pause() {
            eprintln!("error pausing audio stream: {e}");
        }

        if let Err(e) = self.exit() {
            eprintln!("error exiting gb_loop: {e}");
        }
    }
}
