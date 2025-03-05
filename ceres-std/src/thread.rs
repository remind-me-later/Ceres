use crate::audio;
use ceres_core::{Cart, GbBuilder};
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;
use std::{fs::File, sync::Mutex, time::Duration};
use thread_priority::ThreadBuilderExt;

use {ceres_core::Gb, std::path::Path, std::sync::Arc};

pub trait PainterCallback: Send {
    fn paint(&self, pixel_data_rgba: &[u8]);
    fn request_repaint(&self);
}

pub struct GbThread {
    gb: Arc<Mutex<Gb<audio::AudioCallbackImpl>>>,
    model: ceres_core::Model,
    exiting: Arc<AtomicBool>,
    pause_thread: Arc<AtomicBool>,
    audio_stream: audio::Stream,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl GbThread {
    pub fn new<P: PainterCallback + 'static>(
        model: ceres_core::Model,
        sav_path: Option<&Path>,
        rom_path: Option<&Path>,
        audio_state: &audio::AudioState,
        ctx: P,
    ) -> Result<Self, Error> {
        fn gb_loop<P: PainterCallback>(
            gb: &Arc<Mutex<Gb<audio::AudioCallbackImpl>>>,
            exiting: &Arc<AtomicBool>,
            pause_thread: &Arc<AtomicBool>,
            ctx: &P,
        ) {
            const DURATION: Duration = ceres_core::FRAME_DURATION;

            let mut last_loop = std::time::Instant::now();

            while !exiting.load(Relaxed) {
                if !pause_thread.load(Relaxed) {
                    if let Ok(mut gb) = gb.lock() {
                        gb.run_frame();
                        ctx.paint(gb.pixel_data_rgba());
                    }
                    ctx.request_repaint();
                }

                let elapsed = last_loop.elapsed();

                if elapsed < DURATION {
                    spin_sleep::sleep(DURATION - elapsed);
                }

                last_loop = std::time::Instant::now();
            }
        }

        let audio_stream = audio::Stream::new(audio_state).map_err(Error::Audio)?;
        let ring_buffer = audio_stream.get_ring_buffer();

        let gb = Self::create_new_gb(ring_buffer, model, rom_path, sav_path)?;
        let gb = Arc::new(Mutex::new(gb));

        let pause_thread = Arc::new(AtomicBool::new(false));

        let exiting = Arc::new(AtomicBool::new(false));

        let thread_builder = std::thread::Builder::new().name("gb_loop".to_owned());
        let thread_handle = {
            let gb = Arc::clone(&gb);
            let exit = Arc::clone(&exiting);
            let pause_thread = Arc::clone(&pause_thread);

            // std::thread::spawn(move || gb_loop(gb, exit, pause_thread))
            thread_builder.spawn_with_priority(thread_priority::ThreadPriority::Max, move |_| {
                gb_loop(&gb, &exit, &pause_thread, &ctx);
            })?
        };

        Ok(Self {
            gb,
            exiting,
            pause_thread,
            thread_handle: Some(thread_handle),
            audio_stream,
            model,
        })
    }

    pub fn change_rom(&mut self, sav_path: Option<&Path>, rom_path: &Path) -> Result<(), Error> {
        let ring_buffer = self.audio_stream.get_ring_buffer();

        let gb_new = Self::create_new_gb(ring_buffer, self.model, Some(rom_path), sav_path)?;

        if let Ok(mut gb) = self.gb.lock() {
            *gb = gb_new;
        }

        Ok(())
    }

    fn create_new_gb(
        ring_buffer: audio::AudioCallbackImpl,
        model: ceres_core::Model,
        rom_path: Option<&Path>,
        sav_path: Option<&Path>,
    ) -> Result<Gb<audio::AudioCallbackImpl>, Error> {
        if let Some(rom_path) = rom_path {
            let rom = {
                std::fs::read(rom_path)
                    .map(Vec::into_boxed_slice)
                    .map_err(Error::Io)?
            };

            let cart = Cart::new(rom)?;

            let has_save = cart.has_battery();

            let gb_builder = GbBuilder::new(model, audio::Stream::sample_rate(), cart, ring_buffer);

            if !has_save {
                return Ok(gb_builder.build());
            }

            if let Some(sav_path) = sav_path {
                match File::open(sav_path) {
                    Ok(mut save_data) => {
                        let gb = gb_builder.load_save_data(&mut save_data)?.build();
                        Ok(gb)
                    }
                    Err(_) => Ok(gb_builder.build()),
                }
            } else {
                Ok(gb_builder.build())
            }
        } else {
            Ok(GbBuilder::new(
                model,
                audio::Stream::sample_rate(),
                Cart::default(),
                ring_buffer,
            )
            .build())
        }
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.pause_thread.load(Relaxed)
    }

    pub fn pause(&mut self) -> Result<(), audio::Error> {
        self.audio_stream.pause()?;
        self.pause_thread.store(true, Relaxed);
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), audio::Error> {
        self.pause_thread.store(false, Relaxed);
        self.audio_stream.resume()?;
        Ok(())
    }

    pub fn exit(&mut self) -> Result<(), Error> {
        self.exiting.store(true, Relaxed);
        self.thread_handle
            .take()
            .ok_or(Error::NoThreadRunning)?
            .join()
            .map_err(|_e| Error::ThreadJoin)?;
        Ok(())
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.audio_stream.volume()
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.audio_stream.set_volume(volume);
    }

    #[must_use]
    pub const fn is_muted(&self) -> bool {
        self.audio_stream.is_muted()
    }

    pub fn toggle_mute(&mut self) {
        if self.audio_stream.is_muted() {
            self.audio_stream.unmute();
        } else {
            self.audio_stream.mute();
        }
    }

    pub fn press(&mut self, button: ceres_core::Button) {
        if let Ok(mut gb) = self.gb.lock() {
            gb.press(button);
        }
    }

    pub fn release(&mut self, button: ceres_core::Button) {
        if let Ok(mut gb) = self.gb.lock() {
            gb.release(button);
        }
    }

    pub fn save_data<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
    ) -> Result<(), Error> {
        self.gb.lock().map_or(Err(Error::NoThreadRunning), |gb| {
            gb.save_data(writer).map_err(Error::Io)
        })
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

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Gb(ceres_core::Error),
    Audio(audio::Error),
    ThreadJoin,
    NoThreadRunning,
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "os error: {err}"),
            Self::Gb(err) => write!(f, "gb error: {err}"),
            Self::ThreadJoin => write!(f, "thread join error"),
            Self::NoThreadRunning => write!(f, "no thread running"),
            Self::Audio(err) => write!(f, "audio error: {err}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<ceres_core::Error> for Error {
    fn from(err: ceres_core::Error) -> Self {
        Self::Gb(err)
    }
}
