use adw::{prelude::*, subclass::prelude::*};
use gtk::{
    gio, glib,
    subclass::prelude::{
        ApplicationImpl, ApplicationImplExt, GtkApplicationImpl, ObjectImpl, ObjectSubclass,
        ObjectSubclassExt,
    },
};

use super::cli_handler::CliOptions;

#[derive(Default)]
pub struct Application {
    pub cli_options: std::cell::RefCell<CliOptions>,
    pub preferences_dialog: std::cell::OnceCell<crate::preferences_dialog::PreferencesDialog>,
    pub about_dialog: std::cell::OnceCell<crate::about_dialog::AboutDialog>,
}

#[glib::object_subclass]
impl ObjectSubclass for Application {
    const NAME: &'static str = "Application";
    type ParentType = adw::Application;
    type Type = super::Application;
}

impl ObjectImpl for Application {}

impl ApplicationImpl for Application {
    fn command_line(&self, command_line: &gio::ApplicationCommandLine) -> glib::ExitCode {
        let app = self.obj();
        let args: Vec<String> = command_line
            .arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        let cli_options = CliOptions::parse_from_args(&args);
        *self.cli_options.borrow_mut() = cli_options;

        app.activate();

        glib::ExitCode::SUCCESS
    }

    fn startup(&self) {
        self.parent_startup();
        let app = self.obj();

        super::cli_actions::setup_cli_actions(&app);

        let preferences = crate::preferences_dialog::PreferencesDialog::new();
        let gtk_app = app.upcast_ref::<gtk::Application>();
        preferences.imp().connect_to_actions(gtk_app);

        self.preferences_dialog
            .set(preferences)
            .expect("Preferences dialog should only be set once");

        let about = crate::about_dialog::AboutDialog::new();
        self.about_dialog
            .set(about)
            .expect("About dialog should only be set once");

        #[allow(clippy::shadow_unrelated)]
        let about_action = gio::ActionEntry::builder("about")
            .activate(move |app: &Self::Type, _, _| {
                let about = app
                    .imp()
                    .about_dialog
                    .get()
                    .expect("About dialog should be initialized");

                about.present(app.active_window().as_ref());
            })
            .build();

        #[allow(clippy::shadow_unrelated)]
        let preferences_action = gio::ActionEntry::builder("preferences")
            .activate(move |app: &Self::Type, _, _| {
                let preferences = app
                    .imp()
                    .preferences_dialog
                    .get()
                    .expect("Preferences dialog should be initialized");

                preferences.present(app.active_window().as_ref());
            })
            .build();

        app.add_action_entries([about_action, preferences_action]);

        app.set_accels_for_action("win.open", &["<Primary>o"]);
        app.set_accels_for_action("win.pause", &["space"]);
        app.set_accels_for_action("app.preferences", &["<Primary>comma"]);
    }

    fn activate(&self) {
        let app = self.obj();
        let cli_options = self.cli_options.borrow().clone();

        let window = crate::application_window::ApplicationWindow::new(app.as_ref());

        window.setup_cli_listeners();

        super::cli_actions::apply_cli_options(&app, &cli_options);

        window.present();
    }
}

impl GtkApplicationImpl for Application {}

impl AdwApplicationImpl for Application {}
