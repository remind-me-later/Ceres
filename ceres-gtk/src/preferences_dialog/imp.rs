use adw::{glib, prelude::*, subclass::prelude::*};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct PreferencesDialog {
    add_code_row: adw::EntryRow,
    cheats_group: adw::PreferencesGroup,
    code_rows: RefCell<Vec<adw::ActionRow>>,
    color_correction_row: adw::ComboRow,
    gb_model_row: adw::ComboRow,
    gl_area_bindings: RefCell<Vec<glib::Binding>>,
    initializing: Rc<RefCell<bool>>,
    pixel_perfect_row: adw::SwitchRow,
    preferences_page: adw::PreferencesPage,
    shader_row: adw::ComboRow,
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
            .description("Settings")
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

        let pixel_perfect_row = adw::SwitchRow::builder()
            .title("Pixel Perfect")
            .subtitle("Don't stretch the image")
            .build();

        let color_correction_row = adw::ComboRow::builder()
            .title("Color Correction")
            .subtitle("Select color correction mode")
            .build();

        let color_corrections = gtk::StringList::new(&[
            "Modern Balanced",
            "Modern Boost Contrast",
            "Reduce Contrast",
            "Low Contrast",
            "Correct Curves",
            "Disabled",
        ]);
        color_correction_row.set_model(Some(&color_corrections));

        emulation_group.add(&gb_model_row);
        emulation_group.add(&shader_row);
        emulation_group.add(&pixel_perfect_row);
        emulation_group.add(&color_correction_row);
        preferences_page.add(&emulation_group);

        // Cheats section
        let cheats_group = adw::PreferencesGroup::builder()
            .title("Cheats")
            .description("Game Genie")
            .build();

        let add_code_row = adw::EntryRow::builder()
            .title("Add Game Genie Code")
            .show_apply_button(true)
            .build();

        cheats_group.add(&add_code_row);
        preferences_page.add(&cheats_group);

        Self {
            preferences_page,
            shader_row,
            gb_model_row,
            color_correction_row,
            gl_area_bindings: RefCell::new(Vec::new()),
            initializing: Rc::new(RefCell::new(true)),
            pixel_perfect_row,
            cheats_group,
            add_code_row,
            code_rows: RefCell::new(Vec::new()),
        }
    }
}

impl PreferencesDialog {
    pub(super) fn connect_to_gl_area(&self, gl_area: &crate::gl_area::GlArea) {
        let mut bindings = self.gl_area_bindings.borrow_mut();

        // Bind shader row to GlArea property
        let shader_binding = gl_area
            .bind_property("shader-mode", &self.shader_row, "selected")
            .transform_to(|_, shader_str: String| {
                Some(
                    match shader_str.as_str() {
                        "Nearest" => 0_u32,
                        "Scale2x" => 1_u32,
                        "Scale3x" => 2_u32,
                        "LCD" => 3_u32,
                        "CRT" => 4_u32,
                        _ => 0_u32,
                    }
                    .to_value(),
                )
            })
            .transform_from(|_, selected: u32| {
                Some(
                    match selected {
                        0 => "Nearest",
                        1 => "Scale2x",
                        2 => "Scale3x",
                        3 => "LCD",
                        4 => "CRT",
                        _ => "Nearest",
                    }
                    .to_value(),
                )
            })
            .bidirectional()
            .sync_create()
            .build();

        // Bind gb-model row to GlArea property with confirmation dialog for changes
        let gb_model_row = &self.gb_model_row;
        let gl_area_weak = gl_area.downgrade();
        let initializing = Rc::clone(&self.initializing);

        gb_model_row.connect_selected_notify(glib::clone!(
            #[weak]
            gb_model_row,
            move |row| {
                let gl_area = match gl_area_weak.upgrade() {
                    Some(gl_area) => gl_area,
                    None => return,
                };

                let model_str = match row.selected() {
                    0 => "dmg",
                    1 => "mgb",
                    2 => "cgb",
                    _ => "cgb",
                };

                if *initializing.borrow() {
                    gl_area.set_property("gb-model", model_str);
                    return;
                }

                // For model changes, show confirmation dialog
                let dialog = adw::AlertDialog::builder()
                    .heading("Changing GameBoy model")
                    .body(format!(
                        "Changing the Model to {} will reset the emulator. Are you sure?",
                        model_str.to_uppercase()
                    ))
                    .default_response("cancel")
                    .close_response("cancel")
                    .build();

                dialog.add_responses(&[("cancel", "_Cancel"), ("ok", "_Ok")]);
                dialog.present(Some(row));

                let gl_area_weak = gl_area.downgrade();
                let model_str = model_str.to_owned();
                let initializing = Rc::clone(&initializing);
                dialog.connect_response(
                    None,
                    glib::clone!(
                        #[weak]
                        gb_model_row,
                        move |dialog, response| {
                            let gl_area = match gl_area_weak.upgrade() {
                                Some(gl_area) => gl_area,
                                None => return,
                            };

                            if response == "ok" {
                                gl_area.set_property("gb-model", &model_str);
                            } else {
                                // Reset the selection to current value
                                let current_model = gl_area.property::<String>("gb-model");
                                let index = match current_model.as_str() {
                                    "dmg" => 0,
                                    "mgb" => 1,
                                    "cgb" => 2,
                                    _ => 2,
                                };
                                *initializing.borrow_mut() = true; // Prevent re-triggering
                                gb_model_row.set_selected(index);
                                *initializing.borrow_mut() = false; // Re-enable triggering
                            }
                            dialog.close();
                        }
                    ),
                );
            }
        ));

        // Create a one-way binding from GlArea to the row (for initialization and external changes)
        let gb_model_binding = gl_area
            .bind_property("gb-model", &self.gb_model_row, "selected")
            .transform_to(|_, model_str: String| {
                Some(
                    match model_str.as_str() {
                        "dmg" => 0_u32,
                        "mgb" => 1_u32,
                        "cgb" => 2_u32,
                        _ => 2_u32,
                    }
                    .to_value(),
                )
            })
            .sync_create()
            .build();

        // Bind pixel-perfect row
        let pixel_perfect_binding = gl_area
            .bind_property("pixel-perfect", &self.pixel_perfect_row, "active")
            .bidirectional()
            .sync_create()
            .build();

        // Bind color correction row to GlArea property
        let color_correction_binding = gl_area
            .bind_property("color-correction", &self.color_correction_row, "selected")
            .transform_to(|_, correction_str: String| {
                Some(
                    match correction_str.as_str() {
                        "ModernBalanced" => 0_u32,
                        "ModernBoostContrast" => 1_u32,
                        "ReduceContrast" => 2_u32,
                        "LowContrast" => 3_u32,
                        "CorrectCurves" => 4_u32,
                        "Disabled" => 5_u32,
                        _ => 0_u32,
                    }
                    .to_value(),
                )
            })
            .transform_from(|_, selected: u32| {
                Some(
                    match selected {
                        0 => "ModernBalanced",
                        1 => "ModernBoostContrast",
                        2 => "ReduceContrast",
                        3 => "LowContrast",
                        4 => "CorrectCurves",
                        5 => "Disabled",
                        _ => "ModernBalanced",
                    }
                    .to_value(),
                )
            })
            .bidirectional()
            .sync_create()
            .build();

        bindings.push(shader_binding);
        bindings.push(gb_model_binding);
        bindings.push(pixel_perfect_binding);
        bindings.push(color_correction_binding);

        // Cheats: wire add/remove logic and initial refresh
        self.refresh_game_genie_rows(gl_area);

        // Handle add via apply button
        let add_row = self.add_code_row.clone();
        let gl_area_weak = gl_area.downgrade();
        let prefs_weak = self.obj().downgrade();
        self.add_code_row.connect_apply(move |row| {
            let Some(gl_area) = gl_area_weak.upgrade() else {
                return;
            };
            let Some(prefs) = prefs_weak.upgrade() else {
                return;
            };

            let text = add_row.text();
            let code_str = text.trim().to_owned();
            if code_str.is_empty() {
                return;
            }

            match ceres_std::GameGenieCode::new(&code_str) {
                Ok(code) => {
                    // Ensure RefMut drops before we refresh UI
                    let activate_res = {
                        let mut thread = gl_area.gb_thread().borrow_mut();
                        thread.activate_game_genie(code)
                    };

                    match activate_res {
                        Ok(()) => {
                            add_row.set_text("");
                            prefs.imp().refresh_game_genie_rows(&gl_area);
                        }
                        Err(err) => {
                            let dialog = adw::AlertDialog::builder()
                                .heading("Couldn't add code")
                                .body(format!("{err}"))
                                .default_response("ok")
                                .close_response("ok")
                                .build();
                            dialog.add_responses(&[("ok", "_Ok")]);
                            dialog.present(Some(row));
                        }
                    }
                }
                Err(err) => {
                    let dialog = adw::AlertDialog::builder()
                        .heading("Invalid Game Genie code")
                        .body(format!("{err}"))
                        .default_response("ok")
                        .close_response("ok")
                        .build();
                    dialog.add_responses(&[("ok", "_Ok")]);
                    dialog.present(Some(row));
                }
            }
        });
    }

    pub(super) fn disconnect_from_gl_area(&self) {
        let mut bindings = self.gl_area_bindings.borrow_mut();
        for binding in bindings.drain(..) {
            binding.unbind();
        }
    }

    fn refresh_game_genie_rows(&self, gl_area: &crate::gl_area::GlArea) {
        // Clear previous rows
        for row in self.code_rows.borrow_mut().drain(..) {
            self.cheats_group.remove(&row);
        }

        // Fetch active codes
        let codes = gl_area
            .gb_thread()
            .borrow()
            .active_game_genie_codes()
            .unwrap_or_default();

        for code in codes {
            let row = adw::ActionRow::builder().title(code.to_string()).build();

            let remove_btn = gtk::Button::from_icon_name("list-remove-symbolic");
            remove_btn.add_css_class("destructive-action");
            remove_btn.add_css_class("flat");

            // Wire removal
            let gl_area_weak = gl_area.downgrade();
            let prefs_weak = self.obj().downgrade();
            remove_btn.connect_clicked(move |_| {
                if let Some(gl_area) = gl_area_weak.upgrade() {
                    {
                        // Drop mutable borrow before refreshing UI
                        let mut thread = gl_area.gb_thread().borrow_mut();
                        thread.deactivate_game_genie(&code);
                    }
                    if let Some(prefs) = prefs_weak.upgrade() {
                        prefs.imp().refresh_game_genie_rows(&gl_area);
                    }
                }
            });

            row.add_suffix(&remove_btn);
            self.cheats_group.add(&row);
            self.code_rows.borrow_mut().push(row);
        }
    }

    pub(super) fn set_initialization_complete(&self) {
        *self.initializing.borrow_mut() = false;
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PreferencesDialog {
    const NAME: &'static str = "CeresPreferencesWindow";
    type ParentType = adw::PreferencesDialog;
    type Type = crate::preferences_dialog::PreferencesDialog;
}

impl ObjectImpl for PreferencesDialog {
    fn constructed(&self) {
        self.parent_constructed();
        self.obj().add(&self.preferences_page);
    }

    fn dispose(&self) {
        self.disconnect_from_gl_area();
    }
}

impl WidgetImpl for PreferencesDialog {}
impl AdwDialogImpl for PreferencesDialog {}
impl PreferencesDialogImpl for PreferencesDialog {}
