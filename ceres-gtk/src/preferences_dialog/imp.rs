use adw::{glib, prelude::*, subclass::prelude::*};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct PreferencesDialog {
    preferences_page: adw::PreferencesPage,
    shader_row: adw::ComboRow,
    gb_model_row: adw::ComboRow,
    gl_area_bindings: RefCell<Vec<glib::Binding>>,
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
            gl_area_bindings: RefCell::new(Vec::new()),
            initializing: Rc::new(RefCell::new(true)),
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

        bindings.push(shader_binding);
        bindings.push(gb_model_binding);
    }

    pub(super) fn disconnect_from_gl_area(&self) {
        let mut bindings = self.gl_area_bindings.borrow_mut();
        for binding in bindings.drain(..) {
            binding.unbind();
        }
    }

    pub(super) fn set_initialization_complete(&self) {
        *self.initializing.borrow_mut() = false;
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
        self.disconnect_from_gl_area();
    }
}

impl WidgetImpl for PreferencesDialog {}
impl AdwDialogImpl for PreferencesDialog {}
impl PreferencesDialogImpl for PreferencesDialog {}
