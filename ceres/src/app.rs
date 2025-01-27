use crate::{gb_context::GbContext, screen, Scaling};
use ceres_audio as audio;
use eframe::egui::{self, Key, Vec2};
use rfd::FileDialog;
use std::{fs::File, io::Write};

pub struct App {
    // Config parameters
    project_dirs: directories::ProjectDirs,

    // Contexts
    gb_ctx: GbContext,

    // Rendering
    _audio: audio::State,
    screen: screen::GBScreen<{ ceres_core::PX_WIDTH as u32 }, { ceres_core::PX_HEIGHT as u32 }>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model: ceres_core::Model,
        project_dirs: directories::ProjectDirs,
        rom_path: Option<&std::path::Path>,
        scaling: Scaling,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        let audio = audio::State::new().unwrap();
        let gb_ctx = GbContext::new(model, &project_dirs, rom_path, &audio, ctx).unwrap();
        let gb_clone = gb_ctx.gb_clone();
        let screen = screen::GBScreen::new(cc, gb_clone, scaling);

        Self {
            project_dirs,
            gb_ctx,
            _audio: audio,
            screen,
        }
    }

    fn save_data(&self) {
        let gb = self.gb_ctx.gb_lock();
        if let Some(save_data) = gb.cartridge().save_data() {
            std::fs::create_dir_all(self.project_dirs.data_dir())
                .expect("couldn't create data directory");
            let sav_file = File::create(
                self.project_dirs
                    .data_dir()
                    .join(self.gb_ctx.rom_ident())
                    .with_extension("sav"),
            );
            match sav_file {
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

impl eframe::App for App {
    #[allow(clippy::too_many_lines, clippy::shadow_unrelated)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open ROM").clicked() {
                            let file = FileDialog::new()
                                .add_filter("gb", &["gb", "gbc"])
                                .pick_file();

                            if let Some(file) = file {
                                self.gb_ctx.change_rom(&self.project_dirs, &file).unwrap();
                            }
                        }

                        if ui.button("Export save").clicked() {
                            println!("Export save file");
                        }
                    });

                    egui::ComboBox::from_label("Scaling")
                        .selected_text(format!("{}", self.screen.scaling()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                self.screen.mut_scaling(),
                                Scaling::Nearest,
                                "Nearest",
                            );
                            ui.selectable_value(
                                self.screen.mut_scaling(),
                                Scaling::Scale2x,
                                "Scale2x",
                            );
                            ui.selectable_value(
                                self.screen.mut_scaling(),
                                Scaling::Scale3x,
                                "Scale3x",
                            );
                        });

                    let paused = self.gb_ctx.is_paused();
                    if ui
                        .button(if paused { "Play" } else { "Pause" })
                        .on_hover_text("Pause the game")
                        .clicked()
                    {
                        if paused {
                            self.gb_ctx.resume();
                        } else {
                            self.gb_ctx.pause();
                        }
                    }
                });

                egui::Window::new("GameBoy")
                    .resizable(true)
                    .default_size(Vec2::new(
                        ceres_core::PX_WIDTH as f32,
                        ceres_core::PX_HEIGHT as f32,
                    ))
                    .show(ctx, |ui| {
                        self.screen.custom_painting(ui);
                    });
            });

            ctx.input(|i| {
                let mut gb = self.gb_ctx.mut_gb();

                if i.key_pressed(Key::W) {
                    gb.press(ceres_core::Button::Up);
                }

                if i.key_released(Key::W) {
                    gb.release(ceres_core::Button::Up);
                }

                if i.key_pressed(Key::A) {
                    gb.press(ceres_core::Button::Left);
                }

                if i.key_released(Key::A) {
                    gb.release(ceres_core::Button::Left);
                }

                if i.key_pressed(Key::S) {
                    gb.press(ceres_core::Button::Down);
                }

                if i.key_released(Key::S) {
                    gb.release(ceres_core::Button::Down);
                }

                if i.key_pressed(Key::D) {
                    gb.press(ceres_core::Button::Right);
                }

                if i.key_released(Key::D) {
                    gb.release(ceres_core::Button::Right);
                }

                if i.key_pressed(Key::L) {
                    gb.press(ceres_core::Button::A);
                }

                if i.key_released(Key::L) {
                    gb.release(ceres_core::Button::A);
                }

                if i.key_pressed(Key::K) {
                    gb.press(ceres_core::Button::B);
                }

                if i.key_released(Key::K) {
                    gb.release(ceres_core::Button::B);
                }

                if i.key_pressed(Key::M) {
                    gb.press(ceres_core::Button::Start);
                }

                if i.key_released(Key::M) {
                    gb.release(ceres_core::Button::Start);
                }

                if i.key_pressed(Key::N) {
                    gb.press(ceres_core::Button::Select);
                }

                if i.key_released(Key::N) {
                    gb.release(ceres_core::Button::Select);
                }
            });
        });
    }

    fn on_exit(&mut self) {
        self.save_data();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // NOTE: a bright gray makes the shadows of the windows look weird.
        // We use a bit of transparency so that if the user switches on the
        // `transparent()` option they get immediate results.
        egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).to_normalized_gamma_f32()

        // _visuals.window_fill() would also be a natural choice
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw_input: &mut egui::RawInput) {}
}
