use crate::audio;
use ceres_core::{Button, Cart};
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;
use eframe::egui::{self, Key};
use std::{
    io::Read,
    sync::{Mutex, MutexGuard},
};
use thread_priority::ThreadBuilderExt;
use {alloc::sync::Arc, anyhow::Context, ceres_core::Gb, std::path::Path};

pub struct GbContext {
    gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
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
        rom_path: &Path,
        audio_state: &audio::State,
        ctx: &egui::Context,
    ) -> anyhow::Result<Self> {
        fn gb_loop(
            gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
            exiting: Arc<AtomicBool>,
            pause_thread: Arc<AtomicBool>,
            ctx: egui::Context,
        ) {
            loop {
                let begin = std::time::Instant::now();

                if exiting.load(Relaxed) {
                    break;
                }

                let mut duration = ceres_core::FRAME_DURATION;

                if !pause_thread.load(Relaxed) {
                    if let Ok(mut gb) = gb.lock() {
                        duration = gb.run_frame();
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

        let rom = {
            std::fs::read(rom_path)
                .map(Vec::into_boxed_slice)
                .context("no such file")?
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

        if let Ok(ram) = std::fs::read(project_dirs.data_dir().join(&ident).with_extension("sav"))
            .map(Vec::into_boxed_slice)
        {
            cart.set_ram(ram).unwrap();
        }

        let sample_rate = audio::Stream::sample_rate();
        let audio_stream = audio::Stream::new(audio_state)?;
        let ring_buffer = audio_stream.get_ring_buffer();

        let gb = Arc::new(Mutex::new(Gb::new(model, sample_rate, cart, ring_buffer)));

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
                gb_loop(gb, exit, pause_thread, ctx);
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
        })
    }

    pub fn on_key_press(&mut self, key: &Key) {
        if let Ok(mut gb) = self.gb.lock() {
            match key {
                Key::W => gb.press(Button::Up),
                Key::A => gb.press(Button::Left),
                Key::S => gb.press(Button::Down),
                Key::D => gb.press(Button::Right),
                Key::L => gb.press(Button::A),
                Key::K => gb.press(Button::B),
                Key::M => gb.press(Button::Start),
                Key::N => gb.press(Button::Select),
                _ => (),
            }
        }
    }

    pub fn on_key_release(&mut self, key: &Key) {
        if let Ok(mut gb) = self.gb.lock() {
            match key {
                Key::W => gb.release(Button::Up),
                Key::A => gb.release(Button::Left),
                Key::S => gb.release(Button::Down),
                Key::D => gb.release(Button::Right),
                Key::L => gb.release(Button::A),
                Key::K => gb.release(Button::B),
                Key::M => gb.release(Button::Start),
                Key::N => gb.release(Button::Select),
                _ => (),
            }
        }
    }

    pub fn is_paused(&self) -> bool {
        self.pause_thread.load(Relaxed)
    }

    pub fn pause(&mut self) {
        self.audio_stream.pause().unwrap();
        self.pause_thread.store(true, Relaxed);
    }

    pub fn resume(&mut self) {
        self.pause_thread.store(false, Relaxed);
        self.audio_stream.resume().unwrap();
    }

    pub fn rom_ident(&self) -> &str {
        &self.rom_ident
    }

    pub fn exit(&mut self) {
        self.exiting.store(true, Relaxed);
        self.thread_handle.take().unwrap().join().unwrap();
    }

    pub fn gb_lock(&self) -> MutexGuard<Gb<audio::RingBuffer>> {
        self.gb.lock().unwrap()
    }

    pub fn gb_clone(&self) -> Arc<Mutex<Gb<audio::RingBuffer>>> {
        Arc::clone(&self.gb)
    }
}

impl Drop for GbContext {
    fn drop(&mut self) {
        self.audio_stream.pause().unwrap();
        self.exit();
    }
}
