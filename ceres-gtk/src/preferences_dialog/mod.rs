mod imp;

use adw::{glib, subclass::prelude::ObjectSubclassIsExt};

glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog,
        @implements gtk::Native, gtk::Root,
            gtk::Buildable, gtk::Accessible, gtk::ConstraintTarget,
            gtk::ShortcutManager;
}

impl PreferencesDialog {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn set_shader(&self, mode: crate::gl_area::ShaderMode) {
        self.imp().set_shader(mode);
    }

    pub fn set_model(&self, model: ceres_std::Model) {
        self.imp().set_gb_model(model);
    }
}

impl Default for PreferencesDialog {
    fn default() -> Self {
        Self::new()
    }
}
