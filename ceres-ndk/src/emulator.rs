use crate::video::State;
use anyhow::Context;
#[cfg(feature = "game_genie")]
use ceres_std::GameGenieCode;
use ceres_std::{Button, ColorCorrectionMode, GbThread, Model, ShaderOption};
use jni::{JNIEnv, objects::JObject};
use log::debug;
use std::path::Path;

pub struct Emulator {
    pixel_data_rgba: Box<[u8]>,
    state: Option<State>,
    thread: GbThread,
}

impl Emulator {
    #[cfg(feature = "game_genie")]
    pub fn activate_game_genie(&mut self, code: GameGenieCode) -> Result<(), ceres_core::Error> {
        self.thread.activate_game_genie(code)
    }

    #[cfg(feature = "game_genie")]
    pub fn active_game_genie_codes(&self) -> Option<Vec<GameGenieCode>> {
        self.thread.active_game_genie_codes()
    }

    pub fn change_model(&mut self, model: Model) {
        self.thread.change_model(model);
        debug!("Model changed");
    }

    #[cfg(feature = "game_genie")]
    pub fn deactivate_game_genie(&mut self, code: &GameGenieCode) {
        self.thread.deactivate_game_genie(code);
    }

    pub fn drop_state(&mut self) {
        self.state = None;
    }

    pub fn has_save_data(&self) -> bool {
        self.thread.has_save_data()
    }

    pub const fn is_muted(&self) -> bool {
        self.thread.is_muted()
    }

    pub fn is_paused(&self) -> bool {
        self.thread.is_paused()
    }

    pub fn load_rom(
        &mut self,
        rom_path: &Path,
        sav_path: Option<&Path>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Loading ROM: {}", rom_path.display());
        self.thread.change_rom(sav_path, rom_path)?;
        debug!("ROM loaded successfully");
        Ok(())
    }

    pub const fn model(&self) -> Model {
        self.thread.model()
    }

    pub fn multiplier(&self) -> u32 {
        self.thread.multiplier()
    }

    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            pixel_data_rgba: vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
            thread: GbThread::new(Model::default(), None, None)?,
            state: None,
        })
    }

    pub const fn on_lost(&mut self) {
        if let Some(state) = &mut self.state {
            state.on_lost();
        }
    }

    pub fn pause(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.thread.pause()?;
        debug!("Emulator paused");
        Ok(())
    }

    pub fn press_button(&mut self, button: Button) {
        self.thread.press_release(|p| {
            p.press(button);
            true
        });
        debug!("Button pressed");
    }

    pub fn recreate_state(&mut self, env: JNIEnv, surface: JObject) {
        if self.state.is_none() {
            // let env = env.get_java_vm().expect("Failed to get JavaVM");

            match pollster::block_on(State::new(env, surface, ShaderOption::default(), false)) {
                Ok(state) => {
                    self.state = Some(state);
                    debug!("Successfully created wgpu state");
                }
                Err(e) => {
                    log::error!("Failed to create wgpu state: {e}");
                    // Don't panic, just leave state as None
                }
            }
        }
    }

    pub fn release_button(&mut self, button: Button) {
        self.thread.press_release(|p| {
            p.release(button);
            true
        });
        debug!("Button released");
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        if let Some(state) = &mut self.state {
            self.thread
                .copy_pixel_data_rgba(&mut self.pixel_data_rgba)
                .context("Failed to copy pixel data")?;

            debug!("Pixel data copied");

            // let _ = state.window().lock(None); //FIXME: why is this unnecessary?
            state.update_texture(&self.pixel_data_rgba);
            state.render().context("Failed to render frame")?;
        }

        Ok(())
    }

    pub const fn resize(&mut self, width: u32, height: u32) {
        if let Some(state) = &mut self.state {
            state.resize(width, height);
        }
    }

    pub fn resume(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.thread.resume()?;
        debug!("Emulator resumed");
        Ok(())
    }

    pub fn save_data(&self, path: &str) -> anyhow::Result<()> {
        if self.thread.has_save_data() {
            let mut file = std::fs::File::create(path)?;
            self.thread
                .save_data(&mut file)
                .context("Failed to save data")
        } else {
            Ok(())
        }
    }

    // #[cfg(feature = "screenshot")]
    // pub fn save_screenshot<P: AsRef<std::path::Path>>(
    //     &self,
    //     path: P,
    // ) -> Result<(), ceres_std::Error> {
    //     self.thread.save_screenshot(path)
    // }

    pub fn set_color_correction_mode(&self, mode: ColorCorrectionMode) {
        self.thread.set_color_correction_mode(mode);
        debug!("Color correction mode set");
    }

    pub fn set_shader_option(&mut self, shader: ShaderOption) {
        if let Some(state) = &mut self.state {
            state.set_shader_option(shader);
            debug!("Shader option set");
        }
    }

    pub fn set_speed_multiplier(&mut self, multiplier: u32) {
        self.thread.set_speed_multiplier(multiplier);
        debug!("Speed multiplier set to: {multiplier}x");
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.thread.set_volume(volume);
        debug!("Volume set to: {volume}");
    }

    pub fn toggle_mute(&mut self) {
        self.thread.toggle_mute();
        debug!("Mute toggled");
    }

    pub fn volume(&self) -> f32 {
        self.thread.volume()
    }
}
