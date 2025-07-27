mod cli_actions;
mod cli_handler;
mod imp;

pub use cli_handler::CliOptions;
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
            .property("flags", gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .build()
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
