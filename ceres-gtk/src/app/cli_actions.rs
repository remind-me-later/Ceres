use super::CliOptions;
use gtk::{gio, glib, prelude::*};

pub fn setup_cli_actions(app: &crate::app::Application) {
    let set_model_action = gio::SimpleAction::new_stateful(
        "set-model",
        Some(glib::VariantTy::STRING),
        &"cgb".to_variant(),
    );

    set_model_action.connect_activate(glib::clone!(move |action, parameter| {
        if let Some(model_str) = parameter.and_then(|p| p.get::<String>()) {
            action.set_state(&model_str.to_variant());
        }
    }));

    let set_shader_action = gio::SimpleAction::new_stateful(
        "set-shader",
        Some(glib::VariantTy::STRING),
        &"nearest".to_variant(),
    );

    set_shader_action.connect_activate(glib::clone!(move |action, parameter| {
        if let Some(shader_str) = parameter.and_then(|p| p.get::<String>()) {
            action.set_state(&shader_str.to_variant());
        }
    }));

    let load_file_action = gio::SimpleAction::new("load-file", Some(glib::VariantTy::STRING));

    app.add_action(&set_model_action);
    app.add_action(&set_shader_action);
    app.add_action(&load_file_action);
}

pub fn apply_cli_options(app: &crate::app::Application, options: &CliOptions) {
    if let Some(action) = app.lookup_action("set-model") {
        if let Some(stateful_action) = action.downcast_ref::<gio::SimpleAction>() {
            let model_str = match options.model {
                ceres_std::Model::Dmg => "dmg",
                ceres_std::Model::Mgb => "mgb",
                ceres_std::Model::Cgb => "cgb",
            };
            stateful_action.activate(Some(&model_str.to_variant()));
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
            stateful_action.activate(Some(&shader_str.to_variant()));
        }
    }

    if let Some(file_path) = &options.file {
        if let Some(action) = app.lookup_action("load-file") {
            action.activate(Some(&file_path.to_string_lossy().to_string().to_variant()));
        }
    }
}
