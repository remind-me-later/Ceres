use crate::audio;
#[cfg(feature = "game_genie")]
use ceres_core::GameGenieCode;
use ceres_core::GbBuilder;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;
use std::{
    fs::OpenOptions,
    sync::{Condvar, Mutex, atomic::AtomicU32},
};
use thread_priority::ThreadBuilderExt;
use {ceres_core::Gb, std::path::Path, std::sync::Arc};

pub struct GbThread {
    _audio_state: audio::AudioState,
    audio_stream: audio::Stream,
    exiting: Arc<AtomicBool>,
    gb: Arc<Mutex<Gb<audio::AudioCallbackImpl>>>,
    model: ceres_core::Model,
    multiplier: Arc<AtomicU32>,
    pause_condvar: Arc<(Mutex<bool>, Condvar)>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl GbThread {
    /// Activates a Game Genie code.
    ///
    /// # Errors
    ///
    /// Returns an error if too many codes are activated.
    #[cfg(feature = "game_genie")]
    pub fn activate_game_genie(&mut self, code: GameGenieCode) -> Result<(), ceres_core::Error> {
        self.gb
            .lock()
            .map_or(Ok(()), |mut gb| gb.activate_game_genie(code))
    }

    #[must_use]
    #[cfg(feature = "game_genie")]
    pub fn active_game_genie_codes(&self) -> Option<Vec<GameGenieCode>> {
        self.gb
            .lock()
            .map_or(None, |gb| Some(gb.active_game_genie_codes().to_vec()))
    }

    // Resets the GB state and loads the same ROM
    pub fn change_model(&mut self, model: ceres_core::Model) {
        if let Ok(mut gb) = self.gb.lock() {
            self.model = model;
            gb.change_model_and_soft_reset(model);
        }
    }

    /// Changes the ROM loaded by the Game Boy thread.
    ///
    /// # Errors
    ///
    /// Returns an error if creating a new Game Boy instance fails.
    pub fn change_rom(&mut self, sav_path: Option<&Path>, rom_path: &Path) -> Result<(), Error> {
        let ring_buffer = self.audio_stream.ring_buffer();

        let gb_new = Self::create_new_gb(
            &self.audio_stream,
            ring_buffer,
            self.model,
            Some(rom_path),
            sav_path,
        )?;

        if let Ok(mut gb) = self.gb.lock() {
            *gb = gb_new;
        }

        Ok(())
    }

    /// Copies the pixel data in RGBA format to the provided buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the Game Boy thread is not running.
    pub fn copy_pixel_data_rgba(&self, buffer: &mut [u8]) -> Result<(), Error> {
        self.gb.lock().map_or(Err(Error::NoThreadRunning), |gb| {
            debug_assert_eq!(buffer.len(), gb.pixel_data_rgba().len());
            buffer.copy_from_slice(gb.pixel_data_rgba());
            Ok(())
        })
    }

    fn create_new_gb(
        audio_stream: &audio::Stream,
        ring_buffer: audio::AudioCallbackImpl,
        model: ceres_core::Model,
        rom_path: Option<&Path>,
        sav_path: Option<&Path>,
    ) -> Result<Gb<audio::AudioCallbackImpl>, Error> {
        let gb_builder = GbBuilder::new(audio_stream.sample_rate(), ring_buffer).with_model(model);

        if let Some(rom_path) = rom_path {
            let gb_builder = {
                let rom = std::fs::read(rom_path)
                    .map(Vec::into_boxed_slice)
                    .map_err(Error::Io)?;

                gb_builder.with_rom(rom)?
            };

            if gb_builder.can_load_save_data()
                && let Some(sav_path) = sav_path
            {
                let mut save_data = OpenOptions::new()
                    .read(true)
                    .write(false)
                    .create(false)
                    .truncate(false)
                    .open(sav_path)
                    .map_err(Error::Io)?;
                let mut gb = gb_builder.build();
                gb.load_data(&mut save_data)?;
                Ok(gb)
            } else {
                Ok(gb_builder.build())
            }
        } else {
            Ok(gb_builder.build())
        }
    }

    #[cfg(feature = "game_genie")]
    pub fn deactivate_game_genie(&mut self, code: &GameGenieCode) {
        if let Ok(mut gb) = self.gb.lock() {
            gb.deactivate_game_genie(code);
        }
    }

    /// Exits the Game Boy thread and waits for it to finish.
    ///
    /// # Errors
    ///
    /// Returns an error if no thread is running or if joining the thread fails.
    pub fn exit(&mut self) -> Result<(), Error> {
        self.exiting.store(true, Relaxed);

        // Wake up the thread if it's paused so it can exit
        let (pause_lock, pause_cvar) = &*self.pause_condvar;
        if let Ok(mut paused) = pause_lock.lock() {
            *paused = false;
            pause_cvar.notify_one();
        }

        self.thread_handle
            .take()
            .ok_or(Error::NoThreadRunning)?
            .join()
            .map_err(|_e| Error::ThreadJoin)?;
        Ok(())
    }

    #[must_use]
    pub fn has_save_data(&self) -> bool {
        self.gb.lock().is_ok_and(|gb| gb.cart_has_battery())
    }

    #[must_use]
    pub const fn is_muted(&self) -> bool {
        self.audio_stream.is_muted()
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        let (pause_lock, _) = &*self.pause_condvar;
        pause_lock.lock().is_ok_and(|paused| *paused)
    }

    #[must_use]
    pub const fn model(&self) -> ceres_core::Model {
        self.model
    }

    #[must_use]
    pub fn multiplier(&self) -> u32 {
        self.multiplier.load(Relaxed)
    }

    /// Creates a new `GbThread` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if audio initialization, audio stream creation, or Game Boy creation fails.
    pub fn new(
        model: ceres_core::Model,
        sav_path: Option<&Path>,
        rom_path: Option<&Path>,
    ) -> Result<Self, Error> {
        fn gb_loop(
            gb: &Arc<Mutex<Gb<audio::AudioCallbackImpl>>>,
            exiting: &Arc<AtomicBool>,
            pause_condvar: &Arc<(Mutex<bool>, Condvar)>,
            multiplier: &Arc<AtomicU32>,
        ) {
            let mut last_loop = std::time::Instant::now();

            loop {
                // Check if we need to pause using the condition variable
                let (pause_lock, pause_cvar) = &**pause_condvar;
                if let Ok(mut paused) = pause_lock.lock() {
                    while *paused && !exiting.load(Relaxed) {
                        if let Ok(new_paused) = pause_cvar.wait(paused) {
                            paused = new_paused;
                        } else {
                            return; // Exit if the Condvar is poisoned
                        }
                    }
                }

                // Exit if we were signaled to exit while paused
                if exiting.load(Relaxed) {
                    break;
                }

                if let Ok(mut gb) = gb.lock() {
                    gb.run_frame();
                    // ctx.paint(gb.pixel_data_rgba());
                }
                // ctx.request_repaint();

                let duration = ceres_core::FRAME_DURATION / multiplier.load(Relaxed);
                let elapsed = last_loop.elapsed();

                if elapsed < duration {
                    spin_sleep::sleep(duration - elapsed);
                }

                last_loop = std::time::Instant::now();
            }
        }

        let audio_state = audio::AudioState::new().map_err(Error::Audio)?;

        let audio_stream = audio::Stream::new(&audio_state).map_err(Error::Audio)?;
        let ring_buffer = audio_stream.ring_buffer();

        let gb = Self::create_new_gb(&audio_stream, ring_buffer, model, rom_path, sav_path)?;
        let gb = Arc::new(Mutex::new(gb));

        #[expect(
            clippy::mutex_atomic,
            reason = "Using a Mutex to protect the pause state and a Condvar to signal when to pause/resume the thread"
        )]
        let pause_condvar = Arc::new((Mutex::new(false), Condvar::new()));

        let exiting = Arc::new(AtomicBool::new(false));

        let multiplier = Arc::new(AtomicU32::new(1));

        let thread_builder = std::thread::Builder::new().name("gb_loop".to_owned());
        let thread_handle = {
            let gb = Arc::clone(&gb);
            let exit = Arc::clone(&exiting);
            let pause_condvar = Arc::clone(&pause_condvar);
            let multiplier = Arc::clone(&multiplier);

            // std::thread::spawn(move || gb_loop(gb, exit, pause_thread))
            thread_builder.spawn_with_priority(thread_priority::ThreadPriority::Max, move |_| {
                gb_loop(&gb, &exit, &pause_condvar, &multiplier);
            })?
        };

        Ok(Self {
            gb,
            exiting,
            pause_condvar,
            thread_handle: Some(thread_handle),
            _audio_state: audio_state,
            audio_stream,
            model,
            multiplier,
        })
    }

    /// Pauses the Game Boy thread and audio stream.
    ///
    /// # Errors
    ///
    /// Returns an error if pausing the audio stream fails.
    pub fn pause(&mut self) -> Result<(), audio::Error> {
        self.audio_stream.pause()?;

        // Signal the condition variable
        let (pause_lock, _pause_cvar) = &*self.pause_condvar;
        if let Ok(mut paused) = pause_lock.lock() {
            *paused = true;
        }

        Ok(())
    }

    pub fn press_release<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(&mut dyn Pressable) -> bool,
    {
        self.gb.lock().is_ok_and(|mut gb| f(&mut *gb))
    }

    /// Resumes the Game Boy thread and audio stream.
    ///
    /// # Errors
    ///
    /// Returns an error if resuming the audio stream fails.
    pub fn resume(&mut self) -> Result<(), audio::Error> {
        // Signal the condition variable to wake up the thread
        let (pause_lock, pause_cvar) = &*self.pause_condvar;
        if let Ok(mut paused) = pause_lock.lock() {
            *paused = false;
            pause_cvar.notify_one();
        }

        self.audio_stream.resume()?;
        Ok(())
    }

    /// Saves the current save data to the provided writer.
    ///
    /// # Errors
    ///
    /// Returns an error if the Game Boy thread is not running or if writing the save data fails.
    pub fn save_data<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
    ) -> Result<(), Error> {
        self.gb.lock().map_or(Err(Error::NoThreadRunning), |gb| {
            gb.save_data(writer).map_err(Error::Io)
        })
    }

    /// Saves a WebP screenshot to the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the Game Boy thread is not running, if creating the image fails,
    /// or if writing the file fails.
    pub fn save_screenshot<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Error> {
        let pixel_data = {
            let gb = self.gb.lock().map_err(|_err| Error::NoThreadRunning)?;
            // save into a vector so we can release the lock early
            gb.pixel_data_rgba().to_vec()
        };

        let img = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
            u32::from(ceres_core::PX_WIDTH),
            u32::from(ceres_core::PX_HEIGHT),
            pixel_data,
        )
        .ok_or(Error::ImageCreate)?;

        img.save_with_format(path, image::ImageFormat::WebP)
            .map_err(Error::Image)?;

        Ok(())
    }

    pub fn set_color_correction_mode(&self, mode: ceres_core::ColorCorrectionMode) {
        if let Ok(mut gb) = self.gb.lock() {
            gb.set_color_correction_mode(mode);
        }
    }

    fn set_sample_rate(&self, sample_rate: i32) {
        if let Ok(mut gb) = self.gb.lock() {
            gb.set_sample_rate(sample_rate);
        }
    }

    pub fn set_speed_multiplier(&mut self, multiplier: u32) {
        self.multiplier.store(multiplier, Relaxed);
        #[expect(clippy::cast_possible_wrap)]
        self.set_sample_rate(self.audio_stream.sample_rate() / multiplier as i32);
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.audio_stream.set_volume(volume);
    }

    pub fn toggle_mute(&mut self) {
        if self.audio_stream.is_muted() {
            self.audio_stream.unmute();
        } else {
            self.audio_stream.mute();
        }
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
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

#[derive(Debug)]
pub enum Error {
    Audio(audio::Error),
    Gb(ceres_core::Error),
    Image(image::ImageError),
    ImageCreate,
    Io(std::io::Error),
    NoThreadRunning,
    ThreadJoin,
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
            Self::Image(err) => write!(f, "image error: {err}"),
            Self::ImageCreate => write!(f, "failed to create image"),
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

pub trait Pressable {
    fn press(&mut self, button: ceres_core::Button);
    fn release(&mut self, button: ceres_core::Button);
}

impl Pressable for Gb<audio::AudioCallbackImpl> {
    fn press(&mut self, button: ceres_core::Button) {
        self.press(button);
    }

    fn release(&mut self, button: ceres_core::Button) {
        self.release(button);
    }
}
