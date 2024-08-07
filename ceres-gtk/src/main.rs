mod audio;
mod gl_area;
mod window;

use gtk::glib;
use gtk::prelude::*;
use window::Window;

fn main() -> glib::ExitCode {
    {
        #[cfg(target_os = "macos")]
        let library = unsafe { libloading::os::unix::Library::new("libepoxy.0.dylib") }.unwrap();
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

    let application = adw::Application::builder()
        .application_id("com.github.remind-me-later.Ceres")
        .build();

    application.connect_activate(|app| {
        let win = Window::new(app);
        win.set_visible(true);
    });

    application.run()
}
