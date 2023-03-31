mod imp;
mod renderer;

use std::sync::Arc;

use ceres_core::Gb;
use gtk::{glib, subclass::prelude::ObjectSubclassIsExt};
use parking_lot::Mutex;

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
}
