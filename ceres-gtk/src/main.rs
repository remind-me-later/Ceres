mod app;
mod audio;
mod gl_area;
mod window;

use adw::glib;
use adw::prelude::*;

pub const APP_ID: &str = "com.github.remind-me-later.ceres-gtk";

fn main() -> glib::ExitCode {
    {
        #[cfg(target_os = "macos")]
        let library =
            unsafe { libloading::os::unix::Library::new("/opt/homebrew/lib/libepoxy.0.dylib") }
                .unwrap();
        #[cfg(all(unix, not(target_os = "macos")))]
        let library = unsafe { libloading::os::unix::Library::new("libepoxy.so.0") }.unwrap();
        #[cfg(windows)]
        let library = libloading::os::windows::Library::open_already_loaded("libepoxy-0.dll")
            .or_else(|_| libloading::os::windows::Library::open_already_loaded("epoxy-0.dll"))
            .unwrap();

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
