use crate::video::State;
use ceres_std::{Button, GbThread, Model, ShaderOption};
use jni::{JNIEnv, objects::JObject};
use log::debug;
use std::path::Path;

pub struct Emulator {
    pixel_data_rgba: Box<[u8]>,
    state: Option<State>,
    thread: GbThread,
}

impl Emulator {
    pub fn drop_state(&mut self) {
        self.state = None;
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

    pub fn new() -> Self {
        Self {
            pixel_data_rgba: vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
            thread: GbThread::new(Model::default(), None, None).unwrap(),
            state: None,
        }
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

    pub fn render(&mut self) {
        if let Some(state) = &mut self.state
            && matches!(
                self.thread.copy_pixel_data_rgba(&mut self.pixel_data_rgba),
                Ok(())
            )
        {
            debug!("Pixel data copied");

            // let _ = state.window().lock(None); //FIXME: why is this unnecessary?
            state.update_texture(&self.pixel_data_rgba);
            state.render().expect("Failed to render frame");
        }
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
}
