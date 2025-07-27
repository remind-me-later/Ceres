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

    pub fn connect_to_actions(&self, app: &gtk::Application) {
        self.imp().connect_to_actions(app);
    }

    pub fn disconnect_from_actions(&self, app: &gtk::Application) {
        self.imp().disconnect_from_actions(app);
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
