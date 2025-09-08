use crate::video::State;
use ceres_std::{GbThread, Model, ShaderOption};
use jni::sys::{JNIEnv, jobject};

pub struct Emulator {
    pixel_data_rgba: Box<[u8]>,
    thread: GbThread,
    state: Option<State>,
}

impl Emulator {
    pub fn new(env: *mut JNIEnv, surface: jobject) -> Self {
        Self {
            pixel_data_rgba: vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
            thread: GbThread::new(Model::default(), None, None).unwrap(),
            state: Some(
                pollster::block_on(State::new(env, surface, ShaderOption::default(), false))
                    .unwrap(),
            ),
        }
    }

    pub fn render(&mut self) {
        if let Some(state) = &mut self.state {
            let _ = self.thread.copy_pixel_data_rgba(&mut self.pixel_data_rgba);

            // let _ = state.window().lock(None); FIXME: maybe unnecessary
            state.update_texture(&self.pixel_data_rgba);
            state.render();
        }
    }

    pub fn drop_state(&mut self) {
        self.state = None;
    }

    pub fn recreate_state(&mut self, env: *mut JNIEnv, surface: jobject) {
        if self.state.is_none() {
            self.state = Some(
                pollster::block_on(State::new(env, surface, ShaderOption::default(), false))
                    .unwrap(),
            );
        }
    }
}
