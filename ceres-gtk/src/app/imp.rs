use adw::{prelude::*, subclass::prelude::*};
use ceres_std::cli::{Cli, clap::Parser};
use gtk::{
    gio, glib,
    subclass::prelude::{
        ApplicationImpl, ApplicationImplExt, GtkApplicationImpl, ObjectImpl, ObjectSubclass,
        ObjectSubclassExt,
    },
};

#[derive(Default)]
pub struct Application {
    about_dialog: std::cell::OnceCell<crate::about_dialog::AboutDialog>,
    cli_options: std::cell::RefCell<Cli>,
    preferences_dialog: std::cell::OnceCell<crate::preferences_dialog::PreferencesDialog>,
}

impl Application {
    fn apply_cli_options(&self) {
        let app = self.obj();
        let options = self.cli_options.borrow();

        // For model and shader, we need to access the GlArea from the active window
        if let Some(window) = app.active_window()
            && let Some(app_window) =
                window.downcast_ref::<crate::application_window::ApplicationWindow>()
        {
            let gl_area = app_window.imp().gl_area();

            // Set model property
            let model_str = match options.model() {
                ceres_std::Model::Dmg => "dmg",
                ceres_std::Model::Mgb => "mgb",
                ceres_std::Model::Cgb => "cgb",
            };
            gl_area.set_property("gb-model", model_str);

            // Set shader property
            let shader_str = match options.shader_option() {
                ceres_std::cli::ShaderOption::Nearest => "Nearest",
                ceres_std::cli::ShaderOption::Scale2x => "Scale2x",
                ceres_std::cli::ShaderOption::Scale3x => "Scale3x",
                ceres_std::cli::ShaderOption::Lcd => "LCD",
                ceres_std::cli::ShaderOption::Crt => "CRT",
            };
            gl_area.set_property("shader-mode", shader_str);

            let pixel_perfect = options.pixel_perfect();
            gl_area.set_property("pixel-perfect", pixel_perfect);

            if let Some(file_path) = &options.file()
                && let Some(action) = app_window.lookup_action("win.load-file")
            {
                action.activate(Some(&file_path.to_string_lossy().to_string().to_variant()));
            }
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Application {
    const NAME: &'static str = "Application";
    type ParentType = adw::Application;
    type Type = super::Application;
}

impl ObjectImpl for Application {}

impl ApplicationImpl for Application {
    fn activate(&self) {
        let app = self.obj();

        let window = crate::application_window::ApplicationWindow::new(app.as_ref());

        let preferences = self
            .preferences_dialog
            .get()
            .expect("Preferences dialog should be initialized");

        // Connect preferences dialog to the GlArea using properties
        preferences.connect_to_gl_area(window.imp().gl_area());

        self.apply_cli_options();

        preferences.set_initialization_complete();

        window.present();
    }

    fn command_line(&self, command_line: &gio::ApplicationCommandLine) -> glib::ExitCode {
        let cli_options = Cli::parse_from(command_line.arguments());
        *self.cli_options.borrow_mut() = cli_options;

        let app = self.obj();
        app.activate();

        glib::ExitCode::SUCCESS
    }

    fn startup(&self) {
        self.parent_startup();
        let app = self.obj();

        let preferences = crate::preferences_dialog::PreferencesDialog::new();

        self.preferences_dialog
            .set(preferences)
            .expect("Preferences dialog should only be set once");

        let about = crate::about_dialog::AboutDialog::new();
        self.about_dialog
            .set(about)
            .expect("About dialog should only be set once");

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
        app.set_accels_for_action("win.save-data", &["<Primary>s"]);
        app.set_accels_for_action("win.screenshot", &["F12"]);
        app.set_accels_for_action("app.preferences", &["<Primary>comma"]);
    }
}

impl GtkApplicationImpl for Application {}

impl AdwApplicationImpl for Application {}
