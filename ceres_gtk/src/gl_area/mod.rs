mod imp;
mod renderer;

use ceres_core::Gb;
use gtk::{glib, subclass::prelude::ObjectSubclassIsExt, traits::GLAreaExt};
use parking_lot::Mutex;
use std::sync::Arc;

pub use renderer::PxScaleMode;

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

    pub fn gb(&self) -> &Arc<Mutex<Gb>> {
        &self.imp().gb
    }

    fn make_current(&self) {
        GLAreaExt::make_current(self);
    }

    // pub fn choose_scale_mode(&self, scale_mode: ScaleMode) {
    //     if let Some(rend) = self.imp().renderer.borrow_mut().as_mut() {
    //         // self.make_current();
    //         rend.choose_scale_mode(scale_mode);
    //     }
    // }

    pub fn set_scale_mode(&self, mode: PxScaleMode) {
        let imp = self.imp();
        *imp.scale_mode.borrow_mut() = mode;
        *imp.scale_changed.borrow_mut() = true;

        // rend.choose_scale_mode(new_scale);
    }
}
