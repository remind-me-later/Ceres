use std::fs;
use std::path::Path;
use std::sync::Arc;

use gtk::gdk::Key;
use gtk::subclass::prelude::*;
use gtk::{glib, prelude::*, CompositeTemplate};

use crate::audio;
use crate::gl_area::{GlArea, PxScaleMode};

#[derive(Debug, CompositeTemplate)]
#[template(file = "window.ui")]
pub struct Window {
    #[template_child(id = "gb_area")]
    pub gb_area: TemplateChild<GlArea>,
    pub dialog: gtk::FileDialog,
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
                    let filename = file.path().expect("Couldn't get file path");

                    // TODO: gracefully handle invalid files
                    match init_gb(ceres_core::Model::Cgb, Some(&filename)) {
                        Ok(mut new_gb) => {
                            // Swap the GB instances
                            let mut lock = win.imp().gb_area.gb().lock();
                            core::mem::swap(&mut *lock, &mut new_gb);
                        }
                        Err(err) => {
                            let info_dialog = gtk::MessageDialog::builder()
                                .transient_for(&win)
                                .modal(true)
                                .buttons(gtk::ButtonsType::Close)
                                .text("Unable to open ROM file")
                                .secondary_text(format!("{err}"))
                                .build();

                            info_dialog.run_future().await;
                            info_dialog.close();
                        }
                    }
                }
            },
        );
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

        let gb = Arc::clone(gl_area.gb());
        keys.connect_key_pressed(move |_, key, _keycode, _state| {
            let mut lock = gb.lock();
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

            gtk::Inhibit(true)
        });

        let gb = Arc::clone(gl_area.gb());
        keys.connect_key_released(move |_, key, _keycode, _state| {
            let mut lock = gb.lock();
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
        });

        self.obj().add_controller(keys);

        // Actions
        let rend = self.gb_area.imp();

        let action_px_scale = gtk::gio::SimpleAction::new_stateful(
            "px_scale",
            Some(&String::static_variant_type()),
            "Nearest".to_variant(),
        );

        action_px_scale.connect_activate(glib::clone!(@weak rend =>
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
                    _ => unreachable!()
                };

                // Set orientation and save state
                rend.obj().set_scale_mode(px_scale_mode);
                action.set_state(parameter.to_variant());
        }));

        self.obj().add_action(&action_px_scale);
    }
}
impl WidgetImpl for Window {}
impl WindowImpl for Window {}
impl ApplicationWindowImpl for Window {}

fn init_gb(
    model: ceres_core::Model,
    rom_path: Option<&Path>,
) -> Result<ceres_core::Gb, ceres_core::Error> {
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

    Ok(ceres_core::Gb::new(model, sample_rate, cart))
}
