use crate::{gb_context::GbContext, screen, Scaling};
use ceres_audio as audio;
use eframe::egui::{self, Key};
use rfd::FileDialog;
use std::fs::File;

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

        #[expect(clippy::unwrap_used)]
        let audio = audio::State::new().unwrap();
        #[expect(clippy::unwrap_used)]
        let mut gb_ctx = GbContext::new(model, &project_dirs, rom_path, &audio, ctx).unwrap();
        let gb_clone = gb_ctx.gb_clone();
        let screen = screen::GBScreen::new(cc, gb_clone, scaling);

        gb_ctx.resume();

        Self {
            project_dirs,
            gb_ctx,
            _audio: audio,
            screen,
        }
    }

    fn save_data(&self) {
        let gb = self.gb_ctx.gb_lock();
        #[expect(clippy::expect_used)]
        {
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
                    gb.save_data(&mut f).expect("couldn't save data");
                }
                Err(e) => {
                    eprintln!("couldn't open save file: {e}");
                }
            }
        }
    }
}

impl eframe::App for App {
    #[expect(clippy::too_many_lines, clippy::shadow_unrelated)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open ROM").clicked() {
                        let file = FileDialog::new()
                            .add_filter("gb", &["gb", "gbc"])
                            .pick_file();

                        if let Some(file) = file {
                            if let Err(e) = self.gb_ctx.change_rom(&self.project_dirs, &file) {
                                eprintln!("couldn't open ROM: {e}");
                            }
                        }
                    }

                    if ui.button("Export save").clicked() {
                        println!("Export save file");
                    }
                });

                ui.menu_button("view", |ui| {
                    ui.add(egui::Label::new("Volume"));

                    ui.horizontal(|ui| {
                        let paused = self.gb_ctx.is_paused();
                        if ui
                            .button(if paused { "\u{25b6}" } else { "\u{23f8}" })
                            .on_hover_text("Pause the game")
                            .clicked()
                        {
                            if paused {
                                self.gb_ctx.resume();
                            } else {
                                self.gb_ctx.pause();
                            }
                        }

                        ui.style_mut().spacing.slider_width = 50.0;

                        let volume_slider = egui::Slider::from_get_set(0.0..=1.0, |volume| {
                            let mut volume_ret = 0.0;

                            if let Ok(mut volume_mutex) = self.gb_ctx.volume().lock() {
                                #[expect(clippy::cast_possible_truncation)]
                                if let Some(volume) = volume {
                                    *volume_mutex = volume as f32;
                                    volume_ret = *volume_mutex;
                                } else {
                                    volume_ret = *volume_mutex;
                                }
                            }

                            f64::from(volume_ret)
                        })
                        .custom_formatter(
                            // percentage
                            |value, _| format!("{:.0}%", value * 100.0),
                        )
                        .trailing_fill(true);

                        ui.add(volume_slider);
                    });

                    ui.separator();

                    ui.add(egui::Label::new("Scaling algorithm"));

                    let nearest_button = egui::SelectableLabel::new(
                        self.screen.scaling() == Scaling::Nearest,
                        "Nearest",
                    );

                    if ui.add(nearest_button).clicked() {
                        *self.screen.mut_scaling() = Scaling::Nearest;
                    }

                    let scale2x_button = egui::SelectableLabel::new(
                        self.screen.scaling() == Scaling::Scale2x,
                        "Scale2x",
                    );

                    if ui.add(scale2x_button).clicked() {
                        *self.screen.mut_scaling() = Scaling::Scale2x;
                    }

                    let scale3x_button = egui::SelectableLabel::new(
                        self.screen.scaling() == Scaling::Scale3x,
                        "Scale3x",
                    );

                    if ui.add(scale3x_button).clicked() {
                        *self.screen.mut_scaling() = Scaling::Scale3x;
                    }

                    ui.separator();

                    ui.add(egui::Label::new("Pixel mode"));

                    let pixel_perfect_button = egui::SelectableLabel::new(
                        self.screen.pixel_mode() == screen::PixelMode::PixelPerfect,
                        "Pixel perfect",
                    );

                    if ui.add(pixel_perfect_button).clicked() {
                        *self.screen.mut_pixel_mode() = screen::PixelMode::PixelPerfect;
                    }

                    let fit_window_button = egui::SelectableLabel::new(
                        self.screen.pixel_mode() == screen::PixelMode::FitWindow,
                        "Fit window",
                    );

                    if ui.add(fit_window_button).clicked() {
                        *self.screen.mut_pixel_mode() = screen::PixelMode::FitWindow;
                    }
                });
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: egui::Margin::default(),
                outer_margin: egui::Margin::default(),
                corner_radius: egui::CornerRadius::default(),
                shadow: egui::Shadow::default(),
                fill: egui::Color32::BLACK,
                stroke: egui::Stroke::NONE,
            })
            .show(ctx, |ui| {
                self.screen.custom_painting(ui);
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
    }

    fn on_exit(&mut self) {
        self.save_data();
    }
}
