use adw::{glib, prelude::*, subclass::prelude::*};

#[derive(Debug)]
pub struct PreferencesDialog {
    preferences_page: adw::PreferencesPage,
}

impl Default for PreferencesDialog {
    fn default() -> Self {
        // Create preferences page
        let preferences_page = adw::PreferencesPage::builder()
            .name("general")
            .title("General")
            .icon_name("preferences-system-symbolic")
            .build();

        // Create emulation group
        let emulation_group = adw::PreferencesGroup::builder()
            .title("Emulation")
            .description("Configure emulation settings")
            .build();

        // Create GB model row with combo
        // Is destructuve action so set css
        let gb_model_row = adw::ComboRow::builder()
            .title("Game Boy Model")
            .subtitle("This will immediately reset the emulator")
            .build();

        // Create string list for GB models
        let gb_models = gtk::StringList::new(&["DMG", "MGB", "CGB"]);
        gb_model_row.set_model(Some(&gb_models));
        gb_model_row.set_selected(2); // Default to GCB

        // Connect the combo row to an action
        gb_model_row.connect_selected_notify(|row| {
            let model_name = match row.selected() {
                0 => "DMG",
                1 => "MGB",
                2 => "CGB",
                _ => "CGB",
            };

            let variant = glib::Variant::from(model_name);

            if let Some(window) = row.root().and_downcast::<gtk::Window>() {
                window
                    .application()
                    .expect("Application should be set")
                    .activate_action("set_gb_model", Some(&variant));
            }
        });

        let shader_row = adw::ComboRow::builder()
            .title("Shader")
            .subtitle("Select a shader to use for rendering")
            .build();

        let shaders = gtk::StringList::new(&["Nearest", "Scale2x", "Scale3x", "LCD", "CRT"]);
        shader_row.set_model(Some(&shaders));
        shader_row.set_selected(0); // Default to Nearest
        shader_row.connect_selected_notify(|row| {
            let shader_name = match row.selected() {
                0 => "Nearest",
                1 => "Scale2x",
                2 => "Scale3x",
                3 => "LCD",
                4 => "CRT",
                _ => "Nearest",
            };

            let variant = glib::Variant::from(shader_name);

            if let Some(window) = row.root().and_downcast::<gtk::Window>() {
                window
                    .application()
                    .expect("Application should be set")
                    .activate_action("set_shader", Some(&variant));
            }
        });

        emulation_group.add(&gb_model_row);
        emulation_group.add(&shader_row);
        preferences_page.add(&emulation_group);

        Self { preferences_page }
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
}

impl WidgetImpl for PreferencesDialog {}
impl WindowImpl for PreferencesDialog {}
impl AdwDialogImpl for PreferencesDialog {}
impl PreferencesDialogImpl for PreferencesDialog {}
