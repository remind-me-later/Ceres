use adw::{prelude::*, subclass::prelude::AdwApplicationImpl};
use gtk::{
    gio, glib,
    subclass::prelude::{
        ApplicationImpl, ApplicationImplExt, GtkApplicationImpl, ObjectImpl, ObjectSubclass,
        ObjectSubclassExt,
    },
};

#[derive(Default)]
pub struct Application;

#[glib::object_subclass]
impl ObjectSubclass for Application {
    const NAME: &'static str = "Application";
    type ParentType = adw::Application;
    type Type = super::Application;
}

impl ObjectImpl for Application {}

impl ApplicationImpl for Application {
    fn startup(&self) {
        self.parent_startup();
        let app = self.obj();

        #[allow(clippy::shadow_unrelated)]
        let about_action = gio::ActionEntry::builder("about")
            .activate(move |app: &Self::Type, _, _| {
                let window = app.active_window();
                let about_dialog = adw::AboutDialog::builder()
                    .application_name("Ceres")
                    .license_type(gtk::License::MitX11)
                    .version("0.1.0")
                    .comments("A GTK+ 4.0 frontend for the Ceres GameBoy emulator")
                    .website("github.com/remind-me-later/ceres")
                    .build();

                about_dialog.present(window.as_ref());
            })
            .build();

        app.add_action_entries([about_action]);

        app.set_accels_for_action("win.open", &["<Primary>o"]);
        app.set_accels_for_action("win.pause", &["space"]);
    }

    fn activate(&self) {
        let app = self.obj();
        let window = crate::application_window::ApplicationWindow::new(app.as_ref());
        window.present();
    }
}

impl GtkApplicationImpl for Application {}

impl AdwApplicationImpl for Application {}
