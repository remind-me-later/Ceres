mod imp;

use adw::{gio, glib, prelude::*};

glib::wrapper! {
    pub struct ApplicationWindow(ObjectSubclass<imp::ApplicationWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements
            gio::ActionMap, gio::ActionGroup,
            gtk::Native, gtk::Root, gtk::Buildable, gtk::Accessible, gtk::ConstraintTarget,
            gtk::ShortcutManager;
}

impl ApplicationWindow {
    pub fn new<P: IsA<gtk::Application>>(app: &P) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
