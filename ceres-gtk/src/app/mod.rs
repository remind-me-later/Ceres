mod imp;

use gtk::gio;
use gtk::glib;

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Application {
    pub fn new() -> Self {
        glib::Object::builder::<Self>()
            .property("application-id", crate::APP_ID)
            .build()
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
