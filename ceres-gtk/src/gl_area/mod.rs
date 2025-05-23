mod imp;
mod renderer;

use gtk::{glib, prelude::*, subclass::prelude::ObjectSubclassIsExt};
use std::{cell::RefCell, rc::Rc};

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

    pub fn gb_thread(&self) -> &Rc<RefCell<ceres_std::GbThread>> {
        &self.imp().gb_thread
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

    pub fn set_scale_mode(&self, mode: PxScaleMode) {
        self.imp().scale_mode_changed.borrow_mut().replace(mode);
    }
}
