use {super::Frame, crate::T_CYCLES_PER_SECOND};

pub trait AudioCallbacks {
    fn sample_rate(&self) -> u32;
    fn push_frame(&mut self, frame: Frame);

    #[allow(clippy::cast_precision_loss)]
    fn cycles_to_render(&self) -> f32 {
        T_CYCLES_PER_SECOND as f32 / self.sample_rate() as f32
    }
}
