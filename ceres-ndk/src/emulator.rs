use crate::video::State;
use ceres_std::{GbThread, Model, ShaderOption, Button};
use jni::sys::{JNIEnv, jobject};
use log::debug;
use std::path::Path;

pub struct Emulator {
    pixel_data_rgba: Box<[u8]>,
    thread: GbThread,
    state: Option<State>,
}

impl Emulator {
    pub fn new() -> Self {
        Self {
            pixel_data_rgba: vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
            thread: GbThread::new(Model::default(), None, None).unwrap(),
            state: None,
        }
    }

    pub fn render(&mut self) {
        if let Some(state) = &mut self.state {
            self.thread
                .copy_pixel_data_rgba(&mut self.pixel_data_rgba)
                .expect("Failed to copy pixel data");

            debug!("Pixel data copied");

            // let _ = state.window().lock(None); FIXME: maybe unnecessary
            state.update_texture(&self.pixel_data_rgba);
            state.render().expect("Failed to render frame");
        }
    }

    pub fn drop_state(&mut self) {
        self.state = None;
    }

    pub fn recreate_state(&mut self, env: *mut JNIEnv, surface: jobject) {
        if self.state.is_none() {
            match pollster::block_on(State::new(env, surface, ShaderOption::default(), false)) {
                Ok(state) => {
                    self.state = Some(state);
                    debug!("Successfully created wgpu state");
                }
                Err(e) => {
                    log::error!("Failed to create wgpu state: {}", e);
                    // Don't panic, just leave state as None
                }
            }
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(state) = &mut self.state {
            state.resize(width, height);
        }
    }

    pub fn on_lost(&mut self) {
        if let Some(state) = &mut self.state {
            state.on_lost();
        }
    }

    pub fn load_rom(&mut self, rom_path: &Path, sav_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Loading ROM: {:?}", rom_path);
        self.thread.change_rom(sav_path, rom_path)?;
        debug!("ROM loaded successfully");
        Ok(())
    }

    pub fn press_button(&mut self, button: Button) {
        self.thread.press_release(|p| {
            p.press(button);
            true
        });
        debug!("Button pressed");
    }

    pub fn release_button(&mut self, button: Button) {
        self.thread.press_release(|p| {
            p.release(button);
            true
        });
        debug!("Button released");
    }

    pub fn pause(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.thread.pause()?;
        debug!("Emulator paused");
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.thread.resume()?;
        debug!("Emulator resumed");
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        self.thread.is_paused()
    }

    pub fn set_speed_multiplier(&mut self, multiplier: u32) {
        self.thread.set_speed_multiplier(multiplier);
        debug!("Speed multiplier set to: {}x", multiplier);
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.thread.set_volume(volume);
        debug!("Volume set to: {}", volume);
    }

    pub fn toggle_mute(&mut self) {
        self.thread.toggle_mute();
        debug!("Mute toggled");
    }
}
