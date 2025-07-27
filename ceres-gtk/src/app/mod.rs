mod cli_actions;
mod cli_handler;
mod imp;

pub use cli_handler::CliOptions;
use gtk::gio;
use gtk::glib;
use gtk::subclass::prelude::*;

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

    pub fn cli_options(&self) -> CliOptions {
        let imp = self.imp();
        imp.cli_options.borrow().clone()
    }

    pub fn preferences_dialog(&self) -> &crate::preferences_dialog::PreferencesDialog {
        self.imp()
            .preferences_dialog
            .get()
            .expect("Preferences dialog should be initialized during startup")
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
