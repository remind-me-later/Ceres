use adw::{gdk, glib, prelude::*, subclass::prelude::*};
use std::{cell::RefCell, fs::File, path::PathBuf, rc::Rc};

use crate::gl_area::GlArea;

#[derive(Debug)]
pub struct ApplicationWindow {
    toolbar_view: adw::ToolbarView,
    gb_area: GlArea,
    pause_button: adw::SplitButton,
    volume_button: gtk::ScaleButton,
    dialog: gtk::FileDialog,
    rom_path: RefCell<Option<PathBuf>>,
    is_paused: RefCell<bool>,
}

impl Default for ApplicationWindow {
    fn default() -> Self {
        let file_dialog = {
            let gb_filter = gtk::FileFilter::new();
            gb_filter.set_name(Some("GameBoy ROMs"));
            gb_filter.add_suffix("gb");
            gb_filter.add_suffix("gbc");

            gtk::FileDialog::builder()
                .modal(true)
                .default_filter(&gb_filter)
                .build()
        };

        let toolbar_view = adw::ToolbarView::new();
        let header_bar = adw::HeaderBar::new();

        let volume_button = gtk::ScaleButton::new(
            0.0,
            1.0,
            0.1,
            &[
                "audio-volume-muted-symbolic",
                "audio-volume-high-symbolic",
                "audio-volume-low-symbolic",
                "audio-volume-medium-symbolic",
            ],
        );
        volume_button.set_value(1.0);

        header_bar.pack_start(&volume_button);

        let pause_button = adw::SplitButton::new();
        pause_button.set_icon_name("media-playback-pause-symbolic");
        pause_button.set_action_name(Some("win.pause"));

        let speed_menu = gtk::gio::Menu::new();
        {
            let x1_speed_item = gtk::gio::MenuItem::new(Some("_1x"), Some("win.speed_multiplier"));
            x1_speed_item
                .set_action_and_target_value(Some("win.speed_multiplier"), Some(&"1".to_variant()));
            speed_menu.append_item(&x1_speed_item);

            let x2_speed_item = gtk::gio::MenuItem::new(Some("_2x"), Some("win.speed_multiplier"));
            x2_speed_item
                .set_action_and_target_value(Some("win.speed_multiplier"), Some(&"2".to_variant()));
            speed_menu.append_item(&x2_speed_item);

            let x4_speed_item = gtk::gio::MenuItem::new(Some("_4x"), Some("win.speed_multiplier"));
            x4_speed_item
                .set_action_and_target_value(Some("win.speed_multiplier"), Some(&"4".to_variant()));
            speed_menu.append_item(&x4_speed_item);

            let x8_speed_item = gtk::gio::MenuItem::new(Some("_8x"), Some("win.speed_multiplier"));
            x8_speed_item
                .set_action_and_target_value(Some("win.speed_multiplier"), Some(&"8".to_variant()));
            speed_menu.append_item(&x8_speed_item);
        }

        pause_button.set_menu_model(Some(&speed_menu));

        header_bar.pack_start(&pause_button);

        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_primary(true);

        let app_menu = gtk::gio::Menu::new();
        {
            let open_item = gtk::gio::MenuItem::new(Some("_Open"), Some("win.open"));
            app_menu.append_item(&open_item);

            let preferences_item =
                gtk::gio::MenuItem::new(Some("_Preferences"), Some("app.preferences"));
            app_menu.append_item(&preferences_item);

            let about_item = gtk::gio::MenuItem::new(Some("_About"), Some("app.about"));
            app_menu.append_item(&about_item);
        }

        menu_button.set_menu_model(Some(&app_menu));
        header_bar.pack_end(&menu_button);

        toolbar_view.add_top_bar(&header_bar);

        let gb_area = GlArea::new();
        toolbar_view.set_content(Some(&gb_area));

        Self {
            dialog: file_dialog,
            gb_area,
            toolbar_view,
            pause_button,
            volume_button,
            rom_path: RefCell::new(None),
            is_paused: RefCell::new(false),
        }
    }
}

impl ApplicationWindow {
    pub fn save_data(&self) {
        if let Some(path) = self.rom_path.borrow().as_ref() {
            if !self.gb_area.gb_thread().borrow().has_save_data() {
                return;
            }

            let file_stem = path.file_stem().unwrap();
            let sav_path = Self::data_path().join(file_stem).with_extension("sav");

            std::fs::create_dir_all(sav_path.parent().unwrap()).unwrap();
            let sav_file = File::create(sav_path);
            self.gb_area
                .gb_thread()
                .borrow_mut()
                .save_data(&mut sav_file.unwrap())
                .unwrap();
        }
    }

    pub fn data_path() -> PathBuf {
        let mut path = glib::user_data_dir();
        path.push(ceres_std::CERES_BIN);
        std::fs::create_dir_all(&path).expect("Could not create directory.");
        path
    }

    pub fn set_model(&self, model: ceres_std::Model) {
        self.gb_area.set_model(model);
    }

    pub fn model(&self) -> ceres_std::Model {
        self.gb_area.model()
    }

    pub fn set_shader(&self, mode: crate::gl_area::ShaderMode) {
        self.gb_area.set_shader(mode);
    }

    pub fn shader(&self) -> crate::gl_area::ShaderMode {
        self.gb_area.shader()
    }

    pub fn setup_cli_action_listeners(&self) {
        let obj = self.obj();

        if let Some(app) = obj
            .application()
            .and_then(|app| app.downcast::<crate::app::Application>().ok())
        {
            if let Some(action) = app.lookup_action("set-model") {
                if let Some(stateful_action) = action.downcast_ref::<gtk::gio::SimpleAction>() {
                    stateful_action.connect_state_notify(glib::clone!(
                        #[weak]
                        obj,
                        move |action| {
                            if let Some(state) = action.state() {
                                if let Some(model_str) = state.get::<String>() {
                                    let model = match model_str.as_str() {
                                        "dmg" => ceres_std::Model::Dmg,
                                        "mgb" => ceres_std::Model::Mgb,
                                        "cgb" => ceres_std::Model::Cgb,
                                        _ => return,
                                    };
                                    obj.set_model(model);
                                }
                            }
                        }
                    ));
                }
            }

            if let Some(action) = app.lookup_action("set-shader") {
                if let Some(stateful_action) = action.downcast_ref::<gtk::gio::SimpleAction>() {
                    stateful_action.connect_state_notify(glib::clone!(
                        #[weak]
                        obj,
                        move |action| {
                            if let Some(state) = action.state() {
                                if let Some(shader_str) = state.get::<String>() {
                                    let shader_mode = match shader_str.as_str() {
                                        "nearest" => crate::gl_area::ShaderMode::Nearest,
                                        "scale2x" => crate::gl_area::ShaderMode::Scale2x,
                                        "scale3x" => crate::gl_area::ShaderMode::Scale3x,
                                        "lcd" => crate::gl_area::ShaderMode::Lcd,
                                        "crt" => crate::gl_area::ShaderMode::Crt,
                                        _ => return,
                                    };
                                    obj.set_shader(shader_mode);
                                }
                            }
                        }
                    ));
                }
            }

            if let Some(action) = app.lookup_action("open-file") {
                if let Some(simple_action) = action.downcast_ref::<gtk::gio::SimpleAction>() {
                    simple_action.connect_activate(glib::clone!(
                        #[weak]
                        obj,
                        move |_action, _parameter| {
                            if let Some(open_action) = obj.lookup_action("open") {
                                open_action.activate(None);
                            }
                        }
                    ));
                }
            }
        }
    }

    pub fn load_file(&self, file_path: &std::path::Path) {
        let pathbuf = file_path.to_path_buf();

        let sav_path = {
            let file_stem = pathbuf.file_stem().unwrap();
            Some(Self::data_path().join(file_stem).with_extension("sav"))
        };

        let change_rom_res = self
            .gb_area
            .gb_thread()
            .borrow_mut()
            .change_rom(sav_path.as_deref(), &pathbuf);

        match change_rom_res {
            Ok(()) => {
                *self.rom_path.borrow_mut() = Some(pathbuf.clone());
                self.obj()
                    .set_title(pathbuf.file_name().map(|s| s.to_string_lossy()).as_deref());
            }
            Err(err) => {
                eprintln!("Unable to load ROM file: {err}");
            }
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for ApplicationWindow {
    const NAME: &'static str = "CeresWindow";
    type Type = crate::application_window::ApplicationWindow;
    type ParentType = adw::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.install_action_async(
            "win.open",
            None,
            |win, _action_name, _action_target| async move {
                let file_dialog = &win.imp().dialog;

                let res = file_dialog.open_future(Some(&win)).await;

                if let Ok(file) = res {
                    let pathbuf = file.path().expect("Couldn't get file path");

                    let sav_path = {
                        let file_stem = pathbuf.file_stem().unwrap();
                        Some(Self::data_path().join(file_stem).with_extension("sav"))
                    };

                    // TODO: gracefully handle invalid files
                    let change_rom_res = win
                        .imp()
                        .gb_area
                        .gb_thread()
                        .borrow_mut()
                        .change_rom(sav_path.as_deref(), &pathbuf);

                    match change_rom_res {
                        Ok(()) => {
                            *win.imp().rom_path.borrow_mut() = Some(pathbuf.clone());
                            win.set_title(
                                pathbuf.file_name().map(|s| s.to_string_lossy()).as_deref(),
                            );
                        }
                        Err(err) => {
                            let info_dialog = adw::AlertDialog::builder()
                                .heading("Unable to open ROM file")
                                .body(format!("{err}"))
                                .default_response("ok")
                                .close_response("ok")
                                .build();

                            info_dialog.add_responses(&[("ok", "_Ok")]);

                            info_dialog.choose_future(&win).await;
                        }
                    }
                }
            },
        );

        klass.install_action("win.pause", None, |win, _action_name, _action_target| {
            let imp = win.imp();
            let button = &imp.pause_button;

            if *imp.is_paused.borrow() {
                imp.gb_area.play();
                button.set_icon_name("media-playback-pause-symbolic");
                *imp.is_paused.borrow_mut() = false;
            } else {
                imp.gb_area.pause();
                button.set_icon_name("media-playback-start-symbolic");
                *imp.is_paused.borrow_mut() = true;
            }
        });
    }
}

impl ObjectImpl for ApplicationWindow {
    fn constructed(&self) {
        self.parent_constructed();

        // KeyBindings
        let gl_area = &self.gb_area;

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);

        {
            let thread_clone = Rc::clone(gl_area.gb_thread());
            keys.connect_key_pressed(move |_, key, _keycode, _state| {
                if thread_clone.borrow_mut().press_release(|k| {
                    match key {
                        gdk::Key::l => k.press(ceres_std::Button::A),
                        gdk::Key::k => k.press(ceres_std::Button::B),
                        gdk::Key::m => k.press(ceres_std::Button::Start),
                        gdk::Key::n => k.press(ceres_std::Button::Select),
                        gdk::Key::w => k.press(ceres_std::Button::Up),
                        gdk::Key::a => k.press(ceres_std::Button::Left),
                        gdk::Key::s => k.press(ceres_std::Button::Down),
                        gdk::Key::d => k.press(ceres_std::Button::Right),
                        _ => return false,
                    };

                    true
                }) {
                    glib::signal::Propagation::Stop
                } else {
                    // if the key is not handled, return Proceed to allow other handlers to run
                    glib::signal::Propagation::Proceed
                }
            });
        }

        {
            let thread_clone = Rc::clone(gl_area.gb_thread());
            keys.connect_key_released(move |_, key, _keycode, _state| {
                thread_clone.borrow_mut().press_release(|k| {
                    match key {
                        gdk::Key::l => k.release(ceres_std::Button::A),
                        gdk::Key::k => k.release(ceres_std::Button::B),
                        gdk::Key::m => k.release(ceres_std::Button::Start),
                        gdk::Key::n => k.release(ceres_std::Button::Select),
                        gdk::Key::w => k.release(ceres_std::Button::Up),
                        gdk::Key::a => k.release(ceres_std::Button::Left),
                        gdk::Key::s => k.release(ceres_std::Button::Down),
                        gdk::Key::d => k.release(ceres_std::Button::Right),
                        _ => return false,
                    };

                    true
                });
            });
        }

        self.obj().add_controller(keys);

        // Actions
        let rend = self.gb_area.imp();

        let action_speed_multiplier = gtk::gio::SimpleAction::new_stateful(
            "speed_multiplier",
            Some(&String::static_variant_type()),
            &"1".to_variant(),
        );

        action_speed_multiplier.connect_activate(glib::clone!(
            #[weak]
            rend,
            move |action, parameter| {
                let parameter = parameter
                    .expect("Could not get parameter.")
                    .get::<String>()
                    .expect("The value needs to be of type `String`.");

                rend.obj()
                    .gb_thread()
                    .borrow_mut()
                    .set_speed_multiplier(parameter.parse::<u32>().unwrap());
                action.set_state(&parameter.to_variant());
            }
        ));

        self.obj().add_action(&action_speed_multiplier);

        self.volume_button.connect_value_changed(glib::clone!(
            #[weak(rename_to = gb_area)]
            self.gb_area,
            move |_, new_volume| {
                gb_area
                    .gb_thread()
                    .borrow_mut()
                    .set_volume(new_volume as f32);
            }
        ));

        self.obj().set_title(Some("Ceres"));
        self.obj().set_content(Some(&self.toolbar_view));

        self.setup_cli_action_listeners();
    }

    fn dispose(&self) {
        self.save_data();
    }
}
impl WidgetImpl for ApplicationWindow {}
impl WindowImpl for ApplicationWindow {}
impl ApplicationWindowImpl for ApplicationWindow {}
impl AdwApplicationWindowImpl for ApplicationWindow {}
