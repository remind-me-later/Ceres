use {
    ceres_glarea::CeresArea,
    gtk::{glib, Box, Button, FileChooserAction, FileChooserDialog, ResponseType},
    libadwaita::{
        gdk::Key,
        gtk::{self, EventControllerKey, FileFilter, Label, Orientation},
        prelude::*,
        Application, ApplicationWindow, HeaderBar,
    },
    std::rc::Rc,
};

mod audio;
mod ceres_glarea;

fn main() {
    let app = Application::builder()
        .application_id("com.example.ceres_adwaita")
        .build();

    app.connect_startup(|_| {
        libadwaita::init();
    });

    app.connect_activate(build_ui);

    app.run();
}

fn build_ui(app: &Application) {
    let window = Rc::new(
        ApplicationWindow::builder()
            .default_height(400)
            .default_width(400)
            .application(app)
            .build(),
    );

    let open_button = Button::with_label("Open");
    let header_bar = Rc::new(
        HeaderBar::builder()
            .title_widget(&libadwaita::WindowTitle::new("Ceres", ""))
            .build(),
    );

    header_bar.pack_start(&open_button);

    let content = Rc::new(
        Box::builder()
            .orientation(Orientation::Vertical)
            .homogeneous(false)
            .build(),
    );

    let default = Rc::new(Label::new(Some("Ceres")));

    content.append(header_bar.as_ref());
    content.append(default.as_ref());

    window.set_content(Some(content.as_ref()));

    {
        let window = Rc::clone(&window);

        open_button
            .connect_clicked(move |_| rom_chooser_dialog(&window, &content, &header_bar, &default));
    }

    window.show();
}

fn rom_chooser_dialog(
    window: &Rc<ApplicationWindow>,
    content: &Rc<Box>,
    header_bar: &Rc<HeaderBar>,
    default: &Rc<Label>,
) {
    let file_chooser = FileChooserDialog::new(
        Some("Open File"),
        Some(window.as_ref()),
        FileChooserAction::Open,
        &[("Open", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
    );

    let filter = FileFilter::new();
    filter.set_name(Some("GameBoy roms"));
    filter.add_pattern("*.gb");
    filter.add_pattern("*.gbc");

    file_chooser.add_filter(&filter);

    let content = Rc::clone(content);
    let header_bar = Rc::clone(header_bar);
    let window = Rc::clone(&window);
    let default = Rc::clone(&default);

    file_chooser.connect_response(move |dialog, response| {
        if response == ResponseType::Ok {
            on_file_chosen(&window, &header_bar, &content, &default, dialog);
        }

        dialog.close();
    });

    file_chooser.show();
}

fn on_file_chosen(
    window: &Rc<ApplicationWindow>,
    header_bar: &Rc<HeaderBar>,
    content: &Rc<Box>,
    default: &Rc<Label>,
    dialog: &FileChooserDialog,
) {
    let filename = dialog
        .file()
        .expect("Couldn't get file")
        .path()
        .expect("Couldn't get file path");

    header_bar.set_title_widget(Some(&libadwaita::WindowTitle::new(
        "Ceres",
        filename.as_path().file_name().unwrap().to_str().unwrap(),
    )));

    let picture = gtk::Picture::new();
    picture.set_can_shrink(false);
    picture.set_halign(gtk::Align::Center);

    let ceres_area = Rc::new(CeresArea::new());
    picture.set_paintable(Some(ceres_area.as_ref()));

    content.remove(default.as_ref());
    content.append(&picture);

    ceres_area.set_rom_path(&filename);

    picture.add_tick_callback(move |gb, _| {
        gb.queue_draw();
        glib::Continue(true)
    });

    let keys = EventControllerKey::new();
    window.add_controller(&keys);

    let rc_pic = Rc::clone(&ceres_area);
    keys.connect_key_pressed(move |_, key, _keycode, _state| {
        match key {
            Key::k => rc_pic.press(ceres_core::Button::A),
            Key::l => rc_pic.press(ceres_core::Button::B),
            Key::p => rc_pic.press(ceres_core::Button::Start),
            Key::o => rc_pic.press(ceres_core::Button::Select),
            Key::w => rc_pic.press(ceres_core::Button::Up),
            Key::a => rc_pic.press(ceres_core::Button::Left),
            Key::s => rc_pic.press(ceres_core::Button::Down),
            Key::d => rc_pic.press(ceres_core::Button::Right),
            _ => (),
        };

        gtk::Inhibit(true)
    });

    let rc_pic = Rc::clone(&ceres_area);
    keys.connect_key_released(move |_, key, _keycode, _state| {
        match key {
            Key::k => rc_pic.release(ceres_core::Button::A),
            Key::l => rc_pic.release(ceres_core::Button::B),
            Key::p => rc_pic.release(ceres_core::Button::Start),
            Key::o => rc_pic.release(ceres_core::Button::Select),
            Key::w => rc_pic.release(ceres_core::Button::Up),
            Key::a => rc_pic.release(ceres_core::Button::Left),
            Key::s => rc_pic.release(ceres_core::Button::Down),
            Key::d => rc_pic.release(ceres_core::Button::Right),
            _ => (),
        };
    });
}
