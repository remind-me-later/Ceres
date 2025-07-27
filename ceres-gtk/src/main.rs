#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod application_window;
#[cfg(target_os = "linux")]
mod gl_area;
#[cfg(target_os = "linux")]
mod preferences_dialog;

#[cfg(target_os = "linux")]
use adw::{glib, prelude::*};

pub const APP_ID: &str = "com.github.remind-me-later.ceres";

#[cfg(target_os = "linux")]
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

    let app = app::Application::new();
    app.run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Ceres GTK is only supported on Linux.");
}
