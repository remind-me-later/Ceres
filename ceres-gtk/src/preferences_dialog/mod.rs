mod imp;

use adw::{glib, subclass::prelude::ObjectSubclassIsExt};

glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog,
        @implements gtk::Buildable, gtk::Accessible, gtk::ConstraintTarget;
}

impl PreferencesDialog {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn connect_to_gl_area(&self, gl_area: &crate::gl_area::GlArea) {
        self.imp().connect_to_gl_area(gl_area);
    }

    pub fn disconnect_from_gl_area(&self) {
        self.imp().disconnect_from_gl_area();
    }

    pub fn set_initialization_complete(&self) {
        self.imp().set_initialization_complete();
    }
}

impl Default for PreferencesDialog {
    fn default() -> Self {
        Self::new()
    }
}
