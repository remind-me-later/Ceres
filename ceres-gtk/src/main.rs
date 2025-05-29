mod app;
mod application_window;
mod gl_area;

use adw::glib;
use adw::prelude::*;

pub const APP_ID: &str = "com.github.remind-me-later.ceres";

fn main() -> glib::ExitCode {
    {
        epoxy::load_with(|name| {
            unsafe {
                let library = libloading::os::unix::Library::new("libepoxy.so.0").unwrap();
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
