mod imp;

use adw::glib;

glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog,
        @implements gtk::Buildable, gtk::Accessible, gtk::ConstraintTarget;
}

impl PreferencesDialog {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }
}

impl Default for PreferencesDialog {
    fn default() -> Self {
        Self::new()
    }
}
