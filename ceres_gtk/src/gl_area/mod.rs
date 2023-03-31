mod imp;
mod renderer;

use std::sync::Arc;

use ceres_core::Gb;
use gtk::{glib, subclass::prelude::ObjectSubclassIsExt, traits::GLAreaExt};
use parking_lot::Mutex;

use self::renderer::ScaleMode;

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

    pub fn toggle_scale_mode(&self) {
        let imp = self.imp();

        let cur_scale = *imp.scale_mode.borrow();
        let new_scale = match cur_scale {
            ScaleMode::Nearest => ScaleMode::Scale2x,
            ScaleMode::Scale2x => ScaleMode::Nearest,
        };

        *imp.scale_mode.borrow_mut() = new_scale;
        *imp.scale_changed.borrow_mut() = true;

        // rend.choose_scale_mode(new_scale);
    }
}
