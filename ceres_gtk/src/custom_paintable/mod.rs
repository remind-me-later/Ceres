mod imp;

use std::sync::Arc;

use ceres_core::Gb;
use gtk::{glib, subclass::prelude::ObjectSubclassIsExt};
use parking_lot::Mutex;

glib::wrapper! {
    pub struct CustomPaintable(ObjectSubclass<imp::CustomPaintable>)
        @extends gtk::Widget, gtk::GLArea;
}

impl Default for CustomPaintable {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomPaintable {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn gb(&self) -> &Arc<Mutex<Gb>> {
        &self.imp().gb
    }
}
