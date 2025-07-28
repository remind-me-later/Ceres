mod app;
mod video;

#[cfg(target_os = "macos")]
mod macos;

use ceres_std::{CERES_STYLIZED, ORGANIZATION, QUALIFIER, clap::Parser};
use winit::event_loop::EventLoop;

#[cfg(not(target_os = "macos"))]
#[derive(Clone)]
enum CeresEvent {}

fn main() -> anyhow::Result<()> {
    let args = ceres_std::Cli::parse();

    #[cfg(target_os = "macos")]
    let main_event_loop = {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        EventLoop::<CeresEvent>::with_user_event()
            .with_default_menu(false)
            .build()?
    };

    #[cfg(target_os = "macos")]
    {
        macos::set_event_proxy(main_event_loop.create_proxy());
        macos::create_menu_bar();
    }

    #[cfg(not(target_os = "macos"))]
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
        args.scaling_option().into(),
    )?;

    main_event_loop.run_app(&mut main_window)?;

    Ok(())
}
