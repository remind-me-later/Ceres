use {
    ceres_glarea::CeresArea,
    gtk::{glib, Box, Button, FileChooserAction, FileChooserDialog, ResponseType},
    libadwaita::{
        gdk::Key,
        gtk::{self, EventControllerKey, Label, Orientation},
        prelude::*,
        subclass::prelude::ObjectSubclassIsExt,
        Application, ApplicationWindow, HeaderBar,
    },
    std::rc::Rc,
};

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

        open_button.connect_clicked(move |_| {
            on_open_button_click(&window, &content, &header_bar, &default)
        });
    }

    window.show();
}

fn on_open_button_click(
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

    let content = Rc::clone(content);
    let header_bar = Rc::clone(header_bar);

    let window = Rc::clone(&window);
    let default = Rc::clone(&default);

    file_chooser.connect_response(move |d, response| {
        if response == ResponseType::Ok {
            let filename = d
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

            {
                let rc_clone1 = Rc::clone(&ceres_area);
                let rc_clone2 = Rc::clone(&ceres_area);

                let keys = EventControllerKey::new();
                window.add_controller(&keys);

                keys.connect_key_pressed(move |_, key, _keycode, _state| {
                    match key {
                        Key::k => rc_clone1
                            .imp()
                            .data
                            .borrow_mut()
                            .as_mut()
                            .unwrap()
                            .gb
                            .press(ceres_core::Button::A),
                        Key::l => rc_clone1
                            .imp()
                            .data
                            .borrow_mut()
                            .as_mut()
                            .unwrap()
                            .gb
                            .press(ceres_core::Button::B),
                        Key::p => rc_clone1
                            .imp()
                            .data
                            .borrow_mut()
                            .as_mut()
                            .unwrap()
                            .gb
                            .press(ceres_core::Button::Start),
                        _ => (),
                    };

                    gtk::Inhibit(true)
                });

                keys.connect_key_released(move |_, key, _keycode, _state| {
                    match key {
                        Key::k => rc_clone2
                            .imp()
                            .data
                            .borrow_mut()
                            .as_mut()
                            .unwrap()
                            .gb
                            .release(ceres_core::Button::A),
                        Key::l => rc_clone2
                            .imp()
                            .data
                            .borrow_mut()
                            .as_mut()
                            .unwrap()
                            .gb
                            .release(ceres_core::Button::B),
                        Key::p => rc_clone2
                            .imp()
                            .data
                            .borrow_mut()
                            .as_mut()
                            .unwrap()
                            .gb
                            .release(ceres_core::Button::Start),
                        _ => (),
                    };
                });
            }
        }

        d.close();
    });

    file_chooser.show();
}
