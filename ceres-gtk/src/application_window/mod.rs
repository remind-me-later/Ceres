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
}
