use adw::{glib, prelude::*};

#[derive(Debug)]
pub struct AboutDialog {
    dialog: adw::AboutDialog,
}

impl AboutDialog {
    pub fn new() -> Self {
        let dialog = adw::AboutDialog::builder()
            .application_name("Ceres")
            .license_type(gtk::License::MitX11)
            .version("0.1.0")
            .comments("A GTK+ 4.0 frontend for the Ceres GameBoy emulator")
            .website("github.com/remind-me-later/ceres")
            .build();

        Self { dialog }
    }

    pub fn present(&self, parent: Option<&impl glib::object::IsA<gtk::Widget>>) {
        self.dialog.present(parent);
    }
}

impl Default for AboutDialog {
    fn default() -> Self {
        Self::new()
    }
}
