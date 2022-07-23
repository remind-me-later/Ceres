use {
    ceres_glarea::CeresArea,
    gtk::{glib, Box, Button, FileChooserAction, FileChooserDialog, Orientation, ResponseType},
    libadwaita::{gtk, prelude::*, Application, ApplicationWindow, HeaderBar},
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
    let window = Rc::new(ApplicationWindow::builder().application(app).build());

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

    content.append(header_bar.as_ref());

    window.set_content(Some(content.as_ref()));

    {
        let window = Rc::clone(&window);
        let content = Rc::clone(&content);
        let header_bar = Rc::clone(&header_bar);

        open_button.connect_clicked(move |_| on_open_button_click(&window, &content, &header_bar));
    }

    window.show();
}

fn on_open_button_click(
    window: &Rc<ApplicationWindow>,
    content: &Rc<Box>,
    header_bar: &Rc<HeaderBar>,
) {
    let file_chooser = FileChooserDialog::new(
        Some("Open File"),
        Some(window.as_ref()),
        FileChooserAction::Open,
        &[("Open", ResponseType::Ok), ("Cancel", ResponseType::Cancel)],
    );

    {
        let content = Rc::clone(content);
        let header_bar = Rc::clone(header_bar);

        file_chooser.connect_response(move |d: &FileChooserDialog, response: ResponseType| {
            if response == ResponseType::Ok {
                let file = d.file().expect("Couldn't get file");
                let filename = file.path().expect("Couldn't get file path");

                header_bar.set_title_widget(Some(&libadwaita::WindowTitle::new(
                    "Ceres",
                    filename.as_path().file_name().unwrap().to_str().unwrap(),
                )));

                let picture = gtk::Picture::new();
                picture.set_can_shrink(false);
                picture.set_halign(gtk::Align::Center);

                let ceres_area = Rc::new(CeresArea::new());
                picture.set_paintable(Some(ceres_area.as_ref()));

                content.append(&picture);

                ceres_area.set_rom_path(&filename);

                picture.add_tick_callback(move |gb, _| {
                    gb.queue_draw();
                    glib::Continue(true)
                });

                {
                    let ceres_glarea = Rc::clone(&ceres_area);

                    glib::timeout_add_local(ceres_core::FRAME_DUR, move || {
                        ceres_glarea.get_frame();
                        glib::Continue(true)
                    });
                }
            }

            d.close();
        });
    }

    file_chooser.show();
}
