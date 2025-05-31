use adw::{prelude::*, subclass::prelude::*};
use gtk::{
    gio, glib,
    subclass::prelude::{
        ApplicationImpl, ApplicationImplExt, GtkApplicationImpl, ObjectImpl, ObjectSubclass,
        ObjectSubclassExt,
    },
};

use crate::gl_area::PxScaleMode;

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

        #[allow(clippy::shadow_unrelated)]
        let preferences_action = gio::ActionEntry::builder("preferences")
            .activate(move |app: &Self::Type, _, _| {
                let window = app.active_window();
                let preferences = crate::preferences_dialog::PreferencesDialog::new();
                preferences.present(window.as_ref());
            })
            .build();

        #[allow(clippy::shadow_unrelated)]
        let change_gb_model_action = gio::ActionEntry::builder("set_gb_model")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(move |app: &Self::Type, _, param: Option<&glib::Variant>| {
                if let Some(parameter) = param {
                    if let Some(model_name) = parameter.get::<String>() {
                        let model = match model_name.as_str() {
                            "DMG" => ceres_std::Model::Dmg,
                            "MGB" => ceres_std::Model::Mgb,
                            "CGB" => ceres_std::Model::Cgb,
                            _ => ceres_std::Model::Cgb,
                        };

                        let win = app.active_window().expect("Active window should be set");
                        // downcast
                        let win = win
                            .downcast_ref::<crate::application_window::ApplicationWindow>()
                            .expect("Active window should be an ApplicationWindow");

                        win.imp().save_data();

                        let thread = &mut win.imp().gb_area.gb_thread().borrow_mut();

                        thread.change_model(model);
                    }
                }
            })
            .build();

        #[allow(clippy::shadow_unrelated)]
        let set_shader_action = gio::ActionEntry::builder("set_shader")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(move |app: &Self::Type, _, param: Option<&glib::Variant>| {
                if let Some(parameter) = param {
                    if let Some(shader_name) = parameter.get::<String>() {
                        let win = app.active_window().expect("Active window should be set");
                        // downcast
                        let win = win
                            .downcast_ref::<crate::application_window::ApplicationWindow>()
                            .expect("Active window should be an ApplicationWindow");

                        if let Some(rend) = win.imp().gb_area.imp().renderer.borrow_mut().as_mut() {
                            let px_scale_mode = match shader_name.as_str() {
                                "Nearest" => PxScaleMode::Nearest,
                                "Scale2x" => PxScaleMode::Scale2x,
                                "Scale3x" => PxScaleMode::Scale3x,
                                "LCD" => PxScaleMode::Lcd,
                                "CRT" => PxScaleMode::Crt,
                                _ => unreachable!(),
                            };

                            rend.choose_scale_mode(px_scale_mode);
                        }
                    }
                }
            })
            .build();

        app.add_action_entries([
            about_action,
            preferences_action,
            change_gb_model_action,
            set_shader_action,
        ]);

        app.set_accels_for_action("win.open", &["<Primary>o"]);
        app.set_accels_for_action("win.pause", &["space"]);
        app.set_accels_for_action("app.preferences", &["<Primary>comma"]);
    }

    fn activate(&self) {
        let app = self.obj();
        let window = crate::application_window::ApplicationWindow::new(app.as_ref());
        window.present();
    }
}

impl GtkApplicationImpl for Application {}

impl AdwApplicationImpl for Application {}
