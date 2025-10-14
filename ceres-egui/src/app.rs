use crate::screen;
use ceres_std::GbThread;
use ceres_std::{ShaderOption, cli::AppOption as _};
use eframe::egui::{self, CornerRadius, Key, style::HandleShape};
use rfd::FileDialog;
use std::{
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct App {
    project_dirs: directories::ProjectDirs,
    rom_path: Option<PathBuf>,
    sav_path: Option<PathBuf>,
    screen: screen::GBScreen<{ ceres_std::PX_WIDTH as u32 }, { ceres_std::PX_HEIGHT as u32 }>,
    thread: GbThread,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model: ceres_std::Model,
        project_dirs: directories::ProjectDirs,
        rom_path: Option<&std::path::Path>,
        shader_option: ShaderOption,
        pixel_perfect: bool,
    ) -> anyhow::Result<Self> {
        // Apply our minimal black and white theme
        setup_theme(&cc.egui_ctx);

        let sav_path = rom_path.and_then(|path| Self::sav_path_from_rom_path(&project_dirs, path));

        let pixel_data_rgba = Arc::new(Mutex::new(
            vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
        ));

        let mut thread = GbThread::new(model, sav_path.as_deref(), rom_path)?;

        let mut screen = screen::GBScreen::new(cc, pixel_data_rgba, shader_option);

        *screen.mut_pixel_perfect() = pixel_perfect;

        thread.resume()?;

        Ok(Self {
            project_dirs,
            thread,
            screen,
            rom_path: rom_path.map(std::path::Path::to_path_buf),
            sav_path,
        })
    }

    fn sav_path_from_rom_path(
        project_dirs: &directories::ProjectDirs,
        rom_path: &std::path::Path,
    ) -> Option<PathBuf> {
        let file_stem = rom_path.file_stem()?;
        Some(
            project_dirs
                .data_dir()
                .join(file_stem)
                .with_extension("sav"),
        )
    }

    fn save_data(&self) -> anyhow::Result<()> {
        if !self.thread.has_save_data() {
            return Ok(());
        }

        std::fs::create_dir_all(self.project_dirs.data_dir())?;
        if let Some(sav_path) = &self.sav_path {
            let mut sav_file = File::create(sav_path)?;
            self.thread.write_save_data(&mut sav_file)?;
        }

        Ok(())
    }
}

impl eframe::App for App {
    fn on_exit(&mut self) {
        if let Err(e) = self.save_data() {
            eprintln!("couldn't save data: {e}");
        }
    }

    #[expect(clippy::too_many_lines)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |top_panel_ui| {
            egui::MenuBar::new().ui(top_panel_ui, |menu_bar_ui| {
                menu_bar_ui.menu_button("File", |menu_button_ui| {
                    if menu_button_ui.button("Open").clicked() {
                        let file = FileDialog::new()
                            .add_filter("gb", &["gb", "gbc"])
                            .pick_file();

                        if let Some(file) = file {
                            let sav_path = Self::sav_path_from_rom_path(&self.project_dirs, &file);

                            if let Err(e) = self.thread.change_rom(sav_path.as_deref(), &file) {
                                eprintln!("couldn't open ROM: {e}");
                            } else {
                                self.sav_path = sav_path;
                                self.rom_path = Some(file);
                            }
                        }
                    }

                    if menu_button_ui.button("Export").clicked() {
                        println!("Export save file");
                    }
                });

                menu_bar_ui.menu_button("View", |menu_button_ui| {
                    menu_button_ui.horizontal(|horizontal_ui| {
                        let paused = self.thread.is_paused();
                        if horizontal_ui
                            .selectable_label(paused, if paused { "\u{25b6}" } else { "\u{23f8}" })
                            .on_hover_text("Pause")
                            .clicked()
                            && let Err(e) = if paused {
                                self.thread.resume()
                            } else {
                                self.thread.pause()
                            }
                        {
                            eprintln!("couldn't pause/resume: {e}");
                        }

                        let selected_multiplier = self.thread.multiplier();

                        for multiplier in [1, 2, 4] {
                            if horizontal_ui
                                .selectable_label(
                                    selected_multiplier == multiplier,
                                    format!("{multiplier}x"),
                                )
                                .on_hover_text(format!("Speed {multiplier}x"))
                                .clicked()
                            {
                                self.thread.set_speed_multiplier(multiplier);
                            }
                        }
                    });

                    menu_button_ui.horizontal(|horizontal_ui| {
                        let muted = self.thread.is_muted();

                        if horizontal_ui
                            .selectable_label(muted, if muted { "\u{1f507}" } else { "\u{1f50a}" })
                            .on_hover_text("Mute")
                            .clicked()
                        {
                            self.thread.toggle_mute();
                        }

                        horizontal_ui.style_mut().spacing.slider_width = 50.0;

                        let volume_slider = egui::Slider::from_get_set(0.0..=1.0, |volume| {
                            if let Some(volume) = volume {
                                #[expect(clippy::cast_possible_truncation)]
                                self.thread.set_volume(volume as f32);
                            }

                            self.thread.volume().into()
                        })
                        .custom_formatter(
                            // percentage
                            |value, _| format!("{:.0}%", value * 100.0),
                        )
                        .trailing_fill(true);

                        horizontal_ui.add(volume_slider);
                    });

                    menu_button_ui.menu_button("Shader", |menu_button_ui| {
                        for shader_option in ShaderOption::iter() {
                            let shader_button = egui::Button::selectable(
                                self.screen.shader_option() == shader_option,
                                shader_option.str(),
                            );

                            if menu_button_ui.add(shader_button).clicked() {
                                *self.screen.shader_option_mut() = shader_option;
                            }
                        }
                    });

                    menu_button_ui.menu_button("Pixel-perfect", |menu_button_ui| {
                        for pixel_perfect in [true, false] {
                            let pixel_button = egui::Button::selectable(
                                self.screen.pixel_perfect() == pixel_perfect,
                                pixel_perfect.to_string(),
                            );

                            if menu_button_ui.add(pixel_button).clicked() {
                                *self.screen.mut_pixel_perfect() = pixel_perfect;
                            }
                        }
                    });
                });
            });
        });

        if let Ok(mut buffer) = self.screen.mut_buffer().lock()
            && self.thread.copy_pixel_data_rgba(&mut buffer).is_err()
        {
            eprintln!("couldn't copy pixel data");
        }

        egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: egui::Margin::default(),
                outer_margin: egui::Margin::default(),
                corner_radius: egui::CornerRadius::default(),
                shadow: egui::Shadow::default(),
                fill: egui::Color32::BLACK,
                stroke: egui::Stroke::NONE,
            })
            .show(ctx, |central_panel_ui| {
                self.screen.custom_painting(central_panel_ui);
            });

        self.thread.press_release(|p| {
            ctx.input(|i| {
                const KEY_DICT: [(Key, ceres_std::Button); 8] = [
                    (Key::W, ceres_std::Button::Up),
                    (Key::S, ceres_std::Button::Down),
                    (Key::A, ceres_std::Button::Left),
                    (Key::D, ceres_std::Button::Right),
                    (Key::L, ceres_std::Button::A),
                    (Key::K, ceres_std::Button::B),
                    (Key::N, ceres_std::Button::Select),
                    (Key::M, ceres_std::Button::Start),
                ];

                for (key, button) in KEY_DICT {
                    if i.key_pressed(key) {
                        p.press(button);
                    }

                    if i.key_released(key) {
                        p.release(button);
                    }
                }
            });

            true
        });

        ctx.request_repaint();
    }
}

fn setup_theme(ctx: &egui::Context) {
    let bg0 = egui::Color32::from_rgb(40, 40, 40); // Background
    let bg1 = egui::Color32::from_rgb(60, 56, 54); // Lighter background
    let bg2 = egui::Color32::from_rgb(80, 73, 69); // Selection background
    let fg0 = egui::Color32::from_rgb(251, 241, 199); // Main text
    let fg1 = egui::Color32::from_rgb(235, 219, 178); // Secondary text
    // let red = egui::Color32::from_rgb(204, 36, 29); // Red accent
    // let green = egui::Color32::from_rgb(152, 151, 26); // Green accent
    let yellow = egui::Color32::from_rgb(215, 153, 33); // Yellow accent
    // let orange = egui::Color32::from_rgb(214, 93, 14); // Orange accent
    let blue = egui::Color32::from_rgb(69, 133, 136); // Blue accent
    // let aqua = egui::Color32::from_rgb(104, 157, 106); // Aqua accent

    let mut style = (*ctx.style()).clone();

    style.visuals.window_fill = bg0;
    style.visuals.panel_fill = bg0;

    style.visuals.widgets.inactive.bg_fill = bg0;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, fg1);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.inactive.weak_bg_fill = bg0;

    style.visuals.widgets.noninteractive.bg_fill = bg0;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.noninteractive.weak_bg_fill = bg0;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.5, fg1);

    style.visuals.widgets.hovered.bg_fill = bg1;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.hovered.weak_bg_fill = bg1;
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, fg0);

    style.visuals.widgets.active.bg_fill = bg2;
    style.visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.active.weak_bg_fill = bg2;
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0, yellow);

    style.visuals.widgets.open.bg_fill = bg1;
    style.visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
    style.visuals.widgets.open.weak_bg_fill = bg1;
    style.visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, fg0);

    let corner_radius = CornerRadius::same(2);
    style.visuals.window_corner_radius = corner_radius;
    style.visuals.menu_corner_radius = corner_radius;
    style.visuals.widgets.noninteractive.corner_radius = corner_radius;
    style.visuals.widgets.inactive.corner_radius = corner_radius;
    style.visuals.widgets.hovered.corner_radius = corner_radius;
    style.visuals.widgets.active.corner_radius = corner_radius;
    style.visuals.widgets.open.corner_radius = corner_radius;

    let shadow = egui::epaint::Shadow {
        offset: [1, 1],
        blur: 5,
        spread: 0,
        color: bg0,
    };
    style.visuals.popup_shadow = shadow;
    style.visuals.window_shadow = shadow;
    style.visuals.handle_shape = HandleShape::Rect { aspect_ratio: 0.5 };
    style.visuals.window_stroke = egui::Stroke {
        width: 0.0,
        color: fg1,
    };
    style.visuals.selection.bg_fill = bg2;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, yellow);

    style.visuals.hyperlink_color = blue;

    style.visuals.override_text_color = Some(fg0);

    ctx.set_style(style);
}
