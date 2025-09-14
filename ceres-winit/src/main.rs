mod app;
mod video;

use ceres_std::cli::{CERES_STYLIZED, ORGANIZATION, QUALIFIER, clap::Parser};
use winit::event_loop::EventLoop;

#[derive(Clone)]
enum CeresEvent {}

fn main() -> anyhow::Result<()> {
    let args = ceres_std::cli::Cli::parse();

    let main_event_loop = { EventLoop::<CeresEvent>::with_user_event().build()? };

    let project_dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, CERES_STYLIZED)
        .ok_or_else(|| {
            anyhow::anyhow!("Failed to get project directories for '{}'", CERES_STYLIZED)
        })?;

    let mut main_window = app::App::new(
        project_dirs,
        args.model(),
        args.file(),
        args.shader_option(),
        args.pixel_perfect(),
    )?;

    main_event_loop.run_app(&mut main_window)?;

    Ok(())
}
