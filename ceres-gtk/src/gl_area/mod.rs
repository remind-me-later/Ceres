mod imp;
mod renderer;

use gtk::{glib, prelude::*, subclass::prelude::ObjectSubclassIsExt};
use std::{cell::RefCell, rc::Rc};

pub use renderer::ShaderMode;

glib::wrapper! {
    pub struct GlArea(ObjectSubclass<imp::GlArea>)
        @extends gtk::Widget, gtk::GLArea,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
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
        self.imp().gb_thread()
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

    pub fn set_shader(&self, mode: ShaderMode) {
        self.imp().set_shader(mode);
    }

    pub fn shader(&self) -> ShaderMode {
        self.imp().shader()
    }

    pub fn set_model(&self, model: ceres_std::Model) {
        self.imp().set_model(model);
    }

    pub fn model(&self) -> ceres_std::Model {
        self.imp().model()
    }
}
