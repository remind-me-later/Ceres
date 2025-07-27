mod imp;

use adw::{gio, glib, prelude::*, subclass::prelude::ObjectSubclassIsExt};

use crate::gl_area::ShaderMode;

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

    pub fn set_model(&self, model: ceres_std::Model) {
        self.imp().set_model(model);
    }

    pub fn model(&self) -> ceres_std::Model {
        self.imp().model()
    }

    pub fn set_shader(&self, mode: ShaderMode) {
        self.imp().set_shader(mode);
    }

    pub fn shader(&self) -> ShaderMode {
        self.imp().shader()
    }

    pub fn save_data(&self) {
        self.imp().save_data()
    }

    pub fn setup_cli_listeners(&self) {
        self.imp().setup_cli_action_listeners();
    }

    pub fn apply_cli_options(&self, options: &crate::app::CliOptions) {
        self.set_model(options.model);

        let shader_mode = match options.shader_option {
            ceres_std::ShaderOption::Nearest => ShaderMode::Nearest,
            ceres_std::ShaderOption::Scale2x => ShaderMode::Scale2x,
            ceres_std::ShaderOption::Scale3x => ShaderMode::Scale3x,
            ceres_std::ShaderOption::Lcd => ShaderMode::Lcd,
            ceres_std::ShaderOption::Crt => ShaderMode::Crt,
        };
        self.set_shader(shader_mode);

        if let Some(file_path) = &options.file {
            self.load_file(file_path);
        }
    }

    pub fn load_file(&self, file_path: &std::path::Path) {
        self.imp().load_file(file_path);
    }
}
