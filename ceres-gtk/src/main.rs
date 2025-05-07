mod app;
mod gl_area;
mod window;

use adw::glib;
use adw::prelude::*;

pub const APP_ID: &str = "com.github.remind-me-later.ceres-gtk";

fn main() -> glib::ExitCode {
    {
        let library = unsafe { libloading::os::unix::Library::new("libepoxy.so.0") }.unwrap();

        epoxy::load_with(|name| {
            unsafe { library.get::<_>(name.as_bytes()) }
                .map(|symbol| *symbol)
                .unwrap_or(core::ptr::null())
        });
    }

    adw::gio::resources_register_include!("ceres_gtk.gresource")
        .expect("Failed to register resources.");

    app::Application::new().run()
}
