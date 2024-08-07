mod imp;
mod renderer;

use ceres_core::Gb;
use gtk::{glib, prelude::*, subclass::prelude::ObjectSubclassIsExt};
use std::sync::{Arc, Mutex};

pub use renderer::PxScaleMode;

use crate::audio;

glib::wrapper! {
    pub struct GlArea(ObjectSubclass<imp::GlArea>)
        @extends gtk::Widget, gtk::GLArea;
}

impl Default for GlArea {
    fn default() -> Self {
        Self::new()
    }
}

impl GlArea {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn gb(&self) -> &Arc<Mutex<Gb<audio::RingBuffer>>> {
        &self.imp().gb
    }

    fn make_current(&self) {
        GLAreaExt::make_current(self);
    }

    pub fn pause(&self) {
        self.imp().pause();
    }

    pub fn play(&self) {
        self.imp().play();
    }

    pub fn clone_volume(&self) -> Arc<Mutex<f32>> {
        Arc::clone(self.imp().audio.borrow().volume())
    }

    pub fn set_scale_mode(&self, mode: PxScaleMode) {
        let imp = self.imp();
        *imp.scale_mode.borrow_mut() = mode;
        *imp.scale_changed.borrow_mut() = true;
    }
}
