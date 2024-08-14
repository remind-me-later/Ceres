use std::cell::RefCell;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use adw::gdk::Key;
use adw::prelude::AdwDialogExt;
use adw::prelude::AlertDialogExtManual;
use adw::subclass::prelude::*;
use adw::{glib, prelude::*};
use gtk::CompositeTemplate;

use crate::audio;
use crate::gl_area::{GlArea, PxScaleMode};

#[derive(Debug, CompositeTemplate)]
#[template(resource = "/org/remind-me-later/ceres-gtk/window.ui")]
pub struct Window {
    #[template_child(id = "gb_area")]
    pub gb_area: TemplateChild<GlArea>,
    #[template_child(id = "pause_button")]
    pub pause_button: TemplateChild<gtk::ToggleButton>,
    #[template_child(id = "volume_button")]
    pub volume_button: TemplateChild<gtk::ScaleButton>,
    pub dialog: gtk::FileDialog,
    pub rom_path: RefCell<Option<PathBuf>>,
}

impl Window {
    fn save_data(&self) {
        let gb = self.gb_area.gb().lock();

        if let Some(save_data) = gb.unwrap().cartridge().save_data() {
            // TODO: if rom can be saved rom_path should be Some
            if let Some(sav_path) = self
                .rom_path
                .borrow()
                .as_ref()
                .map(|p| p.with_extension("sav"))
            {
                let sav_file = File::create(sav_path);
                match sav_file {
                    // TODO: pretty errors
                    Ok(mut f) => {
                        if let Err(e) = f.write_all(save_data) {
                            eprintln!("couldn't save data in save file: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("couldn't open save file: {e}");
                    }
                }
            }
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for Window {
    const NAME: &'static str = "CeresWindow";
    type Type = super::Window;
    type ParentType = gtk::ApplicationWindow;

    fn new() -> Self {
        let file_dialog = {
            let gb_filter = gtk::FileFilter::new();
            gb_filter.set_name(Some("GameBoy roms"));
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
            rom_path: Default::default(),
            volume_button: Default::default(),
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
                    let audio = win.imp().gb_area.imp().audio.borrow().get_ring_buffer();

                    // TODO: gracefully handle invalid files
                    match init_gb(ceres_core::Model::Cgb, Some(&pathbuf), audio) {
                        Ok(mut new_gb) => {
                            *win.imp().rom_path.borrow_mut() = Some(pathbuf);
                            // Swap the GB instances
                            let mut lock = win.imp().gb_area.gb().lock().unwrap();
                            core::mem::swap(&mut *lock, &mut new_gb);
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

        klass.install_action_async(
            "win.pause",
            None,
            |win, _action_name, _action_target| async move {
                let imp = win.imp();
                let button = &imp.pause_button;

                if button.is_active() {
                    imp.gb_area.play();
                    button.set_active(false);
                } else {
                    imp.gb_area.pause();
                    button.set_active(true);
                }
            },
        );

        let about_dialog = gtk::Builder::from_resource("/org/remind-me-later/ceres-gtk/about.ui");
        let about_dialog: adw::AboutDialog = about_dialog
            .object("about_dialog")
            .expect("Failed to find about_dialog in UI file");

        klass.install_action("app.about", None, move |win, _, _| {
            about_dialog.present(Some(win));
        });
    }

    fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for Window {
    fn constructed(&self) {
        self.parent_constructed();

        // KeyBindings
        let gl_area = &self.gb_area;

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);

        {
            let gb = Arc::clone(gl_area.gb());
            keys.connect_key_pressed(move |_, key, _keycode, _state| {
                if let Ok(mut lock) = gb.lock() {
                    match key {
                        Key::k => lock.press(ceres_core::Button::A),
                        Key::l => lock.press(ceres_core::Button::B),
                        Key::m => lock.press(ceres_core::Button::Start),
                        Key::n => lock.press(ceres_core::Button::Select),
                        Key::w => lock.press(ceres_core::Button::Up),
                        Key::a => lock.press(ceres_core::Button::Left),
                        Key::s => lock.press(ceres_core::Button::Down),
                        Key::d => lock.press(ceres_core::Button::Right),
                        _ => (),
                    };
                }

                glib::signal::Propagation::Stop
            });
        }

        {
            let gb = Arc::clone(gl_area.gb());
            keys.connect_key_released(move |_, key, _keycode, _state| {
                if let Ok(mut lock) = gb.lock() {
                    match key {
                        Key::k => lock.release(ceres_core::Button::A),
                        Key::l => lock.release(ceres_core::Button::B),
                        Key::m => lock.release(ceres_core::Button::Start),
                        Key::n => lock.release(ceres_core::Button::Select),
                        Key::w => lock.release(ceres_core::Button::Up),
                        Key::a => lock.release(ceres_core::Button::Left),
                        Key::s => lock.release(ceres_core::Button::Down),
                        Key::d => lock.release(ceres_core::Button::Right),
                        _ => (),
                    };
                }
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
                    _ => unreachable!(),
                };

                // Set orientation and save state
                rend.obj().set_scale_mode(px_scale_mode);
                action.set_state(&parameter.to_variant());
            }
        ));

        self.obj().add_action(&action_px_scale);

        let volume = self.gb_area.volume();

        self.volume_button
            .connect_value_changed(move |_, new_volume| {
                *volume.lock().unwrap() = new_volume as f32;
            });

        // TODO: do this in XML
        self.volume_button.set_icons(&[
            "audio-volume-muted-symbolic",
            "audio-volume-high-symbolic",
            "audio-volume-low-symbolic",
            "audio-volume-medium-symbolic",
        ]);
    }

    fn dispose(&self) {
        self.save_data();
    }
}
impl WidgetImpl for Window {}
impl WindowImpl for Window {}
impl ApplicationWindowImpl for Window {}

fn init_gb(
    model: ceres_core::Model,
    rom_path: Option<&Path>,
    audio: audio::RingBuffer,
) -> Result<ceres_core::Gb<audio::RingBuffer>, ceres_core::Error> {
    let rom = rom_path.map(|p| fs::read(p).map(Vec::into_boxed_slice).unwrap());

    let ram = rom_path
        .map(|p| p.with_extension("sav"))
        .and_then(|p| fs::read(p).map(Vec::into_boxed_slice).ok());

    let cart = if let Some(rom) = rom {
        ceres_core::Cart::new(rom, ram)?
    } else {
        ceres_core::Cart::default()
    };

    let sample_rate = audio::Renderer::sample_rate();

    Ok(ceres_core::Gb::new(model, sample_rate, cart, audio))
}
