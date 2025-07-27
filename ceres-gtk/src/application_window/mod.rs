mod imp;

use adw::{gio, glib, prelude::*, subclass::prelude::ObjectSubclassIsExt};

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

    pub fn setup_cli_listeners(&self) {
        self.imp().setup_cli_action_listeners();
    }
}
