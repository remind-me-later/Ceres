use adw::{prelude::*, subclass::prelude::AdwApplicationImpl};
use gtk::{
    gio, glib,
    subclass::prelude::{
        ApplicationImpl, ApplicationImplExt, GtkApplicationImpl, ObjectImpl, ObjectSubclass,
        ObjectSubclassExt,
    },
};

#[derive(Default)]
// #[properties(wrapper_type = super::Application)]
pub struct Application;

#[glib::object_subclass]
impl ObjectSubclass for Application {
    const NAME: &'static str = "Application";
    type ParentType = adw::Application;
    type Type = super::Application;
}

// #[glib::derived_properties]
impl ObjectImpl for Application {}

impl ApplicationImpl for Application {
    fn startup(&self) {
        self.parent_startup();
        let app = self.obj();

        let about_dialog = gtk::Builder::from_resource("/org/remind-me-later/ceres-gtk/about.ui");
        let about_dialog: adw::AboutDialog = about_dialog
            .object("about_dialog")
            .expect("Failed to find about_dialog in UI file");

        // klass.install_action("app.about", None, move |win, _, _| {
        //     about_dialog.present(Some(win));
        // });

        #[allow(clippy::shadow_unrelated)]
        let about_action = gio::ActionEntry::builder("about")
            .activate(move |app: &Self::Type, _, _| {
                let window = app.active_window();
                about_dialog.present(window.as_ref());
            })
            .build();

        app.add_action_entries([about_action]);
    }

    fn activate(&self) {
        let app = self.obj();
        let window = crate::window::Window::new(app.as_ref());
        window.present();
        // app.add_window(&window);
    }
}

impl GtkApplicationImpl for Application {}

impl AdwApplicationImpl for Application {}
