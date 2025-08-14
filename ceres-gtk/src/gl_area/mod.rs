mod imp;
mod renderer;

use gtk::{glib, prelude::*, subclass::prelude::ObjectSubclassIsExt};
use std::{cell::RefCell, rc::Rc};

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
    pub fn gb_thread(&self) -> &Rc<RefCell<ceres_std::GbThread>> {
        self.imp().gb_thread()
    }

    fn make_current(&self) {
        GLAreaExt::make_current(self);
    }

    pub fn new() -> Self {
        glib::Object::new()
    }
}
