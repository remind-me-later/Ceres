use adw::{glib, prelude::*, subclass::prelude::*};
use gtk::gio;
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct PreferencesDialog {
    preferences_page: adw::PreferencesPage,
    shader_row: adw::ComboRow,
    gb_model_row: adw::ComboRow,
    model_action_handler: RefCell<Option<glib::SignalHandlerId>>,
    shader_action_handler: RefCell<Option<glib::SignalHandlerId>>,
    initializing: Rc<RefCell<bool>>,
}

impl Default for PreferencesDialog {
    fn default() -> Self {
        let preferences_page = adw::PreferencesPage::builder()
            .name("general")
            .title("General")
            .icon_name("preferences-system-symbolic")
            .build();

        let emulation_group = adw::PreferencesGroup::builder()
            .title("Emulation")
            .description("Configure emulation settings")
            .build();

        let gb_model_row = adw::ComboRow::builder()
            .title("Game Boy Model")
            .subtitle("This will immediately reset the emulator")
            .build();

        let gb_models = gtk::StringList::new(&["GameBoy", "GameBoy Pocket", "GameBoy Color"]);
        gb_model_row.set_model(Some(&gb_models));

        let shader_row = adw::ComboRow::builder()
            .title("Shader")
            .subtitle("Select a shader to use for rendering")
            .build();

        let shaders = gtk::StringList::new(&["Nearest", "Scale2x", "Scale3x", "LCD", "CRT"]);
        shader_row.set_model(Some(&shaders));

        emulation_group.add(&gb_model_row);
        emulation_group.add(&shader_row);
        preferences_page.add(&emulation_group);

        Self {
            preferences_page,
            shader_row,
            gb_model_row,
            model_action_handler: RefCell::new(None),
            shader_action_handler: RefCell::new(None),
            initializing: Rc::new(RefCell::new(false)),
        }
    }
}

impl PreferencesDialog {
    pub fn connect_to_actions(&self, app: &gtk::Application) {
        let gb_model_row = &self.gb_model_row;
        let app_weak = app.downgrade();
        let initializing = Rc::clone(&self.initializing);
        gb_model_row.connect_selected_notify(move |row| {
            let app = match app_weak.upgrade() {
                Some(app) => app,
                None => return,
            };

            let model_name = match row.selected() {
                0 => "dmg",
                1 => "mgb",
                2 => "cgb",
                _ => "cgb",
            };
            let variant = glib::Variant::from(model_name);

            if *initializing.borrow() {
                app.activate_action("set-model", Some(&variant));
                return;
            }

            // For model changes, show confirmation dialog
            let dialog = adw::AlertDialog::builder()
                .heading("Changing GameBoy model")
                .body(format!(
                    "Changing the Model to {} will reset the emulator. Are you sure?",
                    model_name.to_uppercase()
                ))
                .default_response("cancel")
                .close_response("cancel")
                .build();

            dialog.add_responses(&[("cancel", "_Cancel"), ("ok", "_Ok")]);
            dialog.present(Some(row));

            let app_weak = app.downgrade();
            dialog.connect_response(None, move |dialog, response| {
                let app = match app_weak.upgrade() {
                    Some(app) => app,
                    None => return,
                };

                if response == "ok" {
                    app.activate_action("set-model", Some(&variant));
                }
                dialog.close();
            });
        });

        // Connect shader selection changes to the CLI action
        let shader_row = &self.shader_row;
        let app_weak = app.downgrade();
        shader_row.connect_selected_notify(move |row| {
            let app = match app_weak.upgrade() {
                Some(app) => app,
                None => return,
            };

            let shader_name = match row.selected() {
                0 => "nearest",
                1 => "scale2x",
                2 => "scale3x",
                3 => "lcd",
                4 => "crt",
                _ => "nearest",
            };

            let variant = glib::Variant::from(shader_name);
            app.activate_action("set-shader", Some(&variant));
        });

        // Listen to action state changes to update UI automatically
        if let Some(model_action) = app.lookup_action("set-model") {
            if let Some(stateful_action) = model_action.downcast_ref::<gio::SimpleAction>() {
                let gb_model_row_weak = self.gb_model_row.downgrade();
                let handler_id = stateful_action.connect_state_notify(move |action| {
                    let row = match gb_model_row_weak.upgrade() {
                        Some(row) => row,
                        None => return,
                    };

                    if let Some(state) = action.state() {
                        if let Some(model_str) = state.get::<String>() {
                            let index = match model_str.as_str() {
                                "dmg" => 0,
                                "mgb" => 1,
                                "cgb" => 2,
                                _ => 2,
                            };

                            row.set_selected(index);
                        }
                    }
                });
                *self.model_action_handler.borrow_mut() = Some(handler_id);

                // Set initial state
                if let Some(state) = stateful_action.state() {
                    if let Some(model_str) = state.get::<String>() {
                        let index = match model_str.as_str() {
                            "dmg" => 0,
                            "mgb" => 1,
                            "cgb" => 2,
                            _ => 2,
                        };
                        *self.initializing.borrow_mut() = true;
                        self.gb_model_row.set_selected(index);
                        *self.initializing.borrow_mut() = false;
                    }
                }
            }
        }

        if let Some(shader_action) = app.lookup_action("set-shader") {
            if let Some(stateful_action) = shader_action.downcast_ref::<gio::SimpleAction>() {
                let shader_row_weak = self.shader_row.downgrade();
                let handler_id = stateful_action.connect_state_notify(move |action| {
                    let row = match shader_row_weak.upgrade() {
                        Some(row) => row,
                        None => return,
                    };

                    if let Some(state) = action.state() {
                        if let Some(shader_str) = state.get::<String>() {
                            let index = match shader_str.as_str() {
                                "nearest" => 0,
                                "scale2x" => 1,
                                "scale3x" => 2,
                                "lcd" => 3,
                                "crt" => 4,
                                _ => 0,
                            };
                            row.set_selected(index);
                        }
                    }
                });
                *self.shader_action_handler.borrow_mut() = Some(handler_id);

                if let Some(state) = stateful_action.state() {
                    if let Some(shader_str) = state.get::<String>() {
                        let index = match shader_str.as_str() {
                            "nearest" => 0,
                            "scale2x" => 1,
                            "scale3x" => 2,
                            "lcd" => 3,
                            "crt" => 4,
                            _ => 0,
                        };
                        self.shader_row.set_selected(index);
                    }
                }
            }
        }
    }

    pub fn disconnect_from_actions(&self, app: &gtk::Application) {
        if let Some(handler_id) = self.model_action_handler.borrow_mut().take() {
            if let Some(action) = app.lookup_action("set-model") {
                action.disconnect(handler_id);
            }
        }

        if let Some(handler_id) = self.shader_action_handler.borrow_mut().take() {
            if let Some(action) = app.lookup_action("set-shader") {
                action.disconnect(handler_id);
            }
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PreferencesDialog {
    const NAME: &'static str = "CeresPreferencesWindow";
    type Type = crate::preferences_dialog::PreferencesDialog;
    type ParentType = adw::PreferencesDialog;
}

impl ObjectImpl for PreferencesDialog {
    fn constructed(&self) {
        self.parent_constructed();
        self.obj().add(&self.preferences_page);
    }

    fn dispose(&self) {
        if let Some(app) = self
            .obj()
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok())
            .and_then(|window| window.application())
        {
            self.disconnect_from_actions(&app);
        }
    }
}

impl WidgetImpl for PreferencesDialog {}
impl AdwDialogImpl for PreferencesDialog {}
impl PreferencesDialogImpl for PreferencesDialog {}
