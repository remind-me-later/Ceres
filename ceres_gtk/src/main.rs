mod audio;
mod gl_area;

use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Context;
use ceres_core::Gb;
use gl_area::GlArea;
use gtk::gdk::Key;
use gtk::glib;
use gtk::prelude::*;
use parking_lot::Mutex;

extern crate alloc;

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
    application.connect_activate(build_ui);
    application.run()
}

fn build_ui(application: &adw::Application) {
    let gl_area = Rc::new(GlArea::new());

    let open_button = gtk::Button::builder()
        .icon_name("document-open-symbolic")
        .build();

    // Menu
    let scale_button = gtk::ToggleButton::builder().label("2x scale").build();

    {
        let gl_area = Rc::clone(&gl_area);
        scale_button.connect_clicked(move |_| {
            gl_area.toggle_scale_mode();
        });
    }

    let popover = gtk::Popover::new();
    popover.set_child(Some(&scale_button));

    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .popover(&popover)
        .build();

    /////////////////////////////

    // Connect a gesture to handle clicks.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);

    let gb = Arc::clone(gl_area.gb());
    keys.connect_key_pressed(move |_, key, _keycode, _state| {
        let mut lock = gb.lock();
        match key {
            Key::k => lock.press(ceres_core::Button::A),
            Key::l => lock.press(ceres_core::Button::B),
            Key::p => lock.press(ceres_core::Button::Start),
            Key::o => lock.press(ceres_core::Button::Select),
            Key::w => lock.press(ceres_core::Button::Up),
            Key::a => lock.press(ceres_core::Button::Left),
            Key::s => lock.press(ceres_core::Button::Down),
            Key::d => lock.press(ceres_core::Button::Right),
            _ => (),
        };

        gtk::Inhibit(true)
    });

    let gb = Arc::clone(gl_area.gb());
    keys.connect_key_released(move |_, key, _keycode, _state| {
        let mut lock = gb.lock();
        match key {
            Key::k => lock.release(ceres_core::Button::A),
            Key::l => lock.release(ceres_core::Button::B),
            Key::p => lock.release(ceres_core::Button::Start),
            Key::o => lock.release(ceres_core::Button::Select),
            Key::w => lock.release(ceres_core::Button::Up),
            Key::a => lock.release(ceres_core::Button::Left),
            Key::s => lock.release(ceres_core::Button::Down),
            Key::d => lock.release(ceres_core::Button::Right),
            _ => (),
        };
    });

    /////////////////////////

    let header_bar = adw::HeaderBar::new();
    header_bar.pack_start(&open_button);
    header_bar.pack_end(&menu_button);

    // Combine the content in a box
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    // Adwaitas' ApplicationWindow does not include a HeaderBar
    content.append(&header_bar);
    content.append(gl_area.as_ref());

    let window = Rc::new(
        adw::ApplicationWindow::builder()
            .application(application)
            .title("Ceres")
            .content(&content)
            .visible(true)
            .build(),
    );

    window.add_controller(keys);

    let gb = Arc::clone(gl_area.gb());

    open_button.connect_clicked(move |_| {
        gtk::glib::MainContext::default().spawn_local(dialog(Rc::clone(&window), Arc::clone(&gb)));
    });
}

async fn dialog<W: IsA<gtk::Window>>(window: Rc<W>, gb: Arc<Mutex<Gb>>) {
    let default_filter = gtk::FileFilter::new();
    default_filter.add_suffix("gb");
    default_filter.add_suffix("gbc");

    let file_dialog = gtk::FileDialog::builder()
        .modal(true)
        .default_filter(&default_filter)
        .build();

    let res = file_dialog.open_future(Some(window.borrow())).await;

    if let Ok(file) = res {
        let filename = file.path().expect("Couldn't get file path");

        // TODO: gracefully handle invalid files
        let mut new_gb = init_gb(ceres_core::Model::Cgb, Some(&filename)).unwrap();

        let mut lock = gb.lock();
        core::mem::swap(&mut *lock, &mut new_gb);
    }
}

fn init_gb(model: ceres_core::Model, rom_path: Option<&Path>) -> anyhow::Result<ceres_core::Gb> {
    let rom = rom_path.map(|p| fs::read(p).map(Vec::into_boxed_slice).unwrap());

    let ram = rom_path
        .map(|p| p.with_extension("sav"))
        .and_then(|p| fs::read(p).map(Vec::into_boxed_slice).ok());

    let cart = if let Some(rom) = rom {
        ceres_core::Cart::new(rom, ram).context("invalid rom header")?
    } else {
        ceres_core::Cart::default()
    };

    let sample_rate = audio::Renderer::sample_rate();

    Ok(ceres_core::Gb::new(model, sample_rate, cart))
}
