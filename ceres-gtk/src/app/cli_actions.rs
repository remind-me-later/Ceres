use super::CliOptions;
use gtk::{prelude::*, subclass::prelude::ObjectSubclassIsExt};

pub fn apply_cli_options(app: &crate::app::Application, options: &CliOptions) {
    // For model and shader, we need to access the GlArea from the active window
    if let Some(window) = app.active_window() {
        if let Some(app_window) =
            window.downcast_ref::<crate::application_window::ApplicationWindow>()
        {
            let gl_area = app_window.imp().gl_area();

            // Set model property
            let model_str = match options.model {
                ceres_std::Model::Dmg => "dmg",
                ceres_std::Model::Mgb => "mgb",
                ceres_std::Model::Cgb => "cgb",
            };
            gl_area.set_property("gb-model", model_str);

            // Set shader property
            let shader_str = match options.shader_option {
                ceres_std::ShaderOption::Nearest => "Nearest",
                ceres_std::ShaderOption::Scale2x => "Scale2x",
                ceres_std::ShaderOption::Scale3x => "Scale3x",
                ceres_std::ShaderOption::Lcd => "LCD",
                ceres_std::ShaderOption::Crt => "CRT",
            };
            gl_area.set_property("shader-mode", shader_str);

            if let Some(file_path) = &options.file {
                if let Some(action) = app_window.lookup_action("win.load-file") {
                    action.activate(Some(&file_path.to_string_lossy().to_string().to_variant()));
                }
            }
        }
    }
}
