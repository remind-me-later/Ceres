use super::CliOptions;
use gtk::{gio, glib, prelude::*};

pub fn setup_cli_actions(app: &crate::app::Application) {
    let set_model_action = gio::SimpleAction::new_stateful(
        "set-model",
        Some(glib::VariantTy::STRING),
        &"cgb".to_variant(),
    );

    let app_weak = app.downgrade();
    set_model_action.connect_activate(move |action, parameter| {
        let app = match app_weak.upgrade() {
            Some(app) => app,
            None => return,
        };

        if let Some(model_str) = parameter.and_then(|p| p.get::<String>()) {
            let model = match model_str.as_str() {
                "dmg" => ceres_std::Model::Dmg,
                "mgb" => ceres_std::Model::Mgb,
                "cgb" => ceres_std::Model::Cgb,
                _ => return,
            };

            if let Some(window) = app.active_window() {
                if let Some(app_window) =
                    window.downcast_ref::<crate::application_window::ApplicationWindow>()
                {
                    app_window.set_model(model);
                    action.set_state(&model_str.to_variant());
                }
            }
        }
    });

    let set_shader_action = gio::SimpleAction::new_stateful(
        "set-shader",
        Some(glib::VariantTy::STRING),
        &"nearest".to_variant(),
    );

    let app_weak = app.downgrade();
    set_shader_action.connect_activate(move |action, parameter| {
        let app = match app_weak.upgrade() {
            Some(app) => app,
            None => return,
        };

        if let Some(shader_str) = parameter.and_then(|p| p.get::<String>()) {
            let shader_mode = match shader_str.as_str() {
                "nearest" => crate::gl_area::ShaderMode::Nearest,
                "scale2x" => crate::gl_area::ShaderMode::Scale2x,
                "scale3x" => crate::gl_area::ShaderMode::Scale3x,
                "lcd" => crate::gl_area::ShaderMode::Lcd,
                "crt" => crate::gl_area::ShaderMode::Crt,
                _ => return,
            };

            if let Some(window) = app.active_window() {
                if let Some(app_window) =
                    window.downcast_ref::<crate::application_window::ApplicationWindow>()
                {
                    app_window.set_shader(shader_mode);
                    action.set_state(&shader_str.to_variant());
                }
            }
        }
    });

    let open_file_action = gio::SimpleAction::new("open-file", None);
    let app_weak = app.downgrade();
    open_file_action.connect_activate(move |_action, _parameter| {
        let app = match app_weak.upgrade() {
            Some(app) => app,
            None => return,
        };

        if let Some(window) = app.active_window() {
            let dialog = gtk::FileDialog::builder().title("Open ROM File").build();

            let app_weak = app.downgrade();
            dialog.open(Some(&window), gio::Cancellable::NONE, move |result| {
                let app = match app_weak.upgrade() {
                    Some(app) => app,
                    None => return,
                };

                if let Ok(file) = result {
                    let path = file.path().unwrap();
                    if let Some(window) = app.active_window() {
                        if let Some(app_window) =
                            window.downcast_ref::<crate::application_window::ApplicationWindow>()
                        {
                            app_window.load_file(&path);
                        }
                    }
                }
            });
        }
    });

    app.add_action(&set_model_action);
    app.add_action(&set_shader_action);
    app.add_action(&open_file_action);

    app.set_accels_for_action("app.open-file", &["<Control>o"]);
}

pub fn apply_cli_options(app: &crate::app::Application, options: &CliOptions) {
    if let Some(action) = app.lookup_action("set-model") {
        if let Some(stateful_action) = action.downcast_ref::<gio::SimpleAction>() {
            let model_str = match options.model {
                ceres_std::Model::Dmg => "dmg",
                ceres_std::Model::Mgb => "mgb",
                ceres_std::Model::Cgb => "cgb",
            };
            stateful_action.set_state(&model_str.to_variant());
        }
    }

    if let Some(action) = app.lookup_action("set-shader") {
        if let Some(stateful_action) = action.downcast_ref::<gio::SimpleAction>() {
            let shader_str = match options.shader_option {
                ceres_std::ShaderOption::Nearest => "nearest",
                ceres_std::ShaderOption::Scale2x => "scale2x",
                ceres_std::ShaderOption::Scale3x => "scale3x",
                ceres_std::ShaderOption::Lcd => "lcd",
                ceres_std::ShaderOption::Crt => "crt",
            };
            stateful_action.set_state(&shader_str.to_variant());
        }
    }
}
