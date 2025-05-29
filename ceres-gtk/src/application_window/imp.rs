use adw::{gdk, glib, prelude::*, subclass::prelude::*};
use std::{cell::RefCell, fs::File, path::PathBuf, rc::Rc};

use crate::gl_area::{GlArea, PxScaleMode};

#[derive(Debug, gtk::CompositeTemplate)]
#[template(resource = "/org/remind-me-later/ceres-gtk/window.ui")]
pub struct ApplicationWindow {
    #[template_child(id = "gb_area")]
    pub gb_area: TemplateChild<GlArea>,
    #[template_child(id = "pause_button")]
    pub pause_button: TemplateChild<adw::SplitButton>,
    #[template_child(id = "volume_button")]
    pub volume_button: TemplateChild<gtk::ScaleButton>,
    pub dialog: gtk::FileDialog,
    pub rom_path: RefCell<Option<PathBuf>>,
    pub is_paused: RefCell<bool>,
}

impl ApplicationWindow {
    fn save_data(&self) {
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
}

#[glib::object_subclass]
impl ObjectSubclass for ApplicationWindow {
    const NAME: &'static str = "CeresWindow";
    type Type = crate::application_window::ApplicationWindow;
    type ParentType = adw::ApplicationWindow;

    fn new() -> Self {
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

        Self {
            dialog: file_dialog,
            gb_area: TemplateChild::default(),
            pause_button: TemplateChild::default(),
            volume_button: Default::default(),
            rom_path: RefCell::new(None),
            is_paused: RefCell::new(false),
        }
    }

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
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
                            // set window title to path
                            win.set_title(
                                pathbuf.file_name().map(|s| s.to_string_lossy()).as_deref(),
                            );
                        }
                        Err(err) => {
                            let info_dialog = adw::AlertDialog::builder()
                                .heading("Unable to open ROM file")
                                .body(format!("{err}"))
                                .default_response("cancel")
                                .close_response("cancel")
                                .build();

                            info_dialog.add_responses(&[("cancel", "_Ok")]);

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

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
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

        let action_px_scale = gtk::gio::SimpleAction::new_stateful(
            "px_scale",
            Some(&String::static_variant_type()),
            &"Nearest".to_variant(),
        );

        action_px_scale.connect_activate(glib::clone!(
            #[weak]
            rend,
            move |action, parameter| {
                // Get parameter
                let parameter = parameter
                    .expect("Could not get parameter.")
                    .get::<String>()
                    .expect("The value needs to be of type `String`.");

                let px_scale_mode = match parameter.as_str() {
                    "Nearest" => PxScaleMode::Nearest,
                    "Scale2x" => PxScaleMode::Scale2x,
                    "Scale3x" => PxScaleMode::Scale3x,
                    "LCD" => PxScaleMode::Lcd,
                    "CRT" => PxScaleMode::Crt,
                    _ => unreachable!(),
                };

                // Set orientation and save state
                rend.obj().set_scale_mode(px_scale_mode);
                action.set_state(&parameter.to_variant());
            }
        ));

        self.obj().add_action(&action_px_scale);

        let action_speed_multiplier = gtk::gio::SimpleAction::new_stateful(
            "speed_multiplier",
            Some(&String::static_variant_type()),
            &"1".to_variant(),
        );

        action_speed_multiplier.connect_activate(glib::clone!(
            #[weak]
            rend,
            move |action, parameter| {
                // Get parameter
                let parameter = parameter
                    .expect("Could not get parameter.")
                    .get::<String>()
                    .expect("The value needs to be of type `String`.");

                // Set orientation and save state
                rend.obj()
                    .gb_thread()
                    .borrow_mut()
                    .set_speed_multiplier(parameter.parse::<u32>().unwrap());
                action.set_state(&parameter.to_variant());
            }
        ));

        self.obj().add_action(&action_speed_multiplier);

        let thread_clone = Rc::clone(self.gb_area.gb_thread());

        self.volume_button
            .connect_value_changed(move |_, new_volume| {
                thread_clone.borrow_mut().set_volume(new_volume as f32);
            });
    }

    fn dispose(&self) {
        self.save_data();
    }
}
impl WidgetImpl for ApplicationWindow {}
impl WindowImpl for ApplicationWindow {}
impl ApplicationWindowImpl for ApplicationWindow {}
impl AdwApplicationWindowImpl for ApplicationWindow {}
