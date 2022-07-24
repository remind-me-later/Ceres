mod imp;

use {
    gtk::{gdk, glib, subclass::prelude::ObjectSubclassIsExt},
    libadwaita::gtk,
    std::path::Path,
};

glib::wrapper! {
    pub struct CeresArea(ObjectSubclass<imp::CeresArea>)
        @extends gtk::Widget,
        @implements gdk::Paintable;
}

impl Default for CeresArea {
    fn default() -> Self {
        Self::new()
    }
}

impl CeresArea {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create CeresArea")
    }

    pub fn set_rom_path(&self, path: &Path) {
        self.imp().set_rom_path(path);
    }
}
