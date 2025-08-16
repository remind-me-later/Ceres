mod app;
mod screen;

use app::App;
use ceres_std::{CERES_STYLIZED, ORGANIZATION, QUALIFIER, clap::Parser};
use eframe::egui;

fn main() -> anyhow::Result<()> {
    let args = ceres_std::Cli::parse();
    let project_dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, CERES_STYLIZED)
        .ok_or_else(|| anyhow::anyhow!("couldn't get project directories"))?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([
                f32::from(ceres_std::PX_WIDTH),
                f32::from(ceres_std::PX_HEIGHT) + 22.0,
            ])
            .with_min_inner_size([
                f32::from(ceres_std::PX_WIDTH),
                f32::from(ceres_std::PX_HEIGHT) + 22.0,
            ]),
        renderer: eframe::Renderer::Wgpu,
        vsync: true,
        depth_buffer: 0,
        stencil_buffer: 0,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        CERES_STYLIZED,
        options,
        Box::new(move |cc| {
            Ok(Box::new(App::new(
                cc,
                args.model(),
                project_dirs,
                args.file(),
                args.shader_option(),
                args.pixel_perfect()
            )?))
        }),
    )
    .map_err(Into::into)
}
