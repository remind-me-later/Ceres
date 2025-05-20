mod app;
mod gl_area;
mod window;

use adw::glib;
use adw::prelude::*;

pub const APP_ID: &str = "com.github.remind-me-later.ceres-gtk";

fn main() -> glib::ExitCode {
    {
        epoxy::load_with(|name| {
            unsafe {
                #[cfg(target_os = "linux")]
                let library = libloading::os::unix::Library::new("libepoxy.so.0").unwrap();
                #[cfg(target_os = "windows")]
                let library = libloading::os::windows::Library::new("epoxy-0.dll").unwrap();
                library.get::<_>(name.as_bytes())
            }
            .map(|symbol| *symbol)
            .unwrap_or(core::ptr::null())
        });
    }

    adw::gio::resources_register_include!("ceres_gtk.gresource")
        .expect("Failed to register resources.");

    app::Application::new().run()
}
