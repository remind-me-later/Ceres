use crate::{AppOption, ScalingOption, ShaderOption, screen};
use anyhow::Context;
use ceres_std::GbThread;
use eframe::egui::{self, CornerRadius, Key, style::HandleShape};
use rfd::FileDialog;
use std::{
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex},
};

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
        width: 1.0,
        color: fg1,
    };
    style.visuals.selection.bg_fill = bg2;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, yellow);

    style.visuals.hyperlink_color = blue;

    style.visuals.override_text_color = Some(fg0);

    ctx.set_style(style);
}

pub struct PainterCallbackImpl {
    ctx: egui::Context,
    buffer: Arc<Mutex<Box<[u8]>>>,
}

impl PainterCallbackImpl {
    pub fn new(ctx: &egui::Context, buffer: Arc<Mutex<Box<[u8]>>>) -> Self {
        Self {
            ctx: ctx.clone(),
            buffer,
        }
    }
}

impl ceres_std::PainterCallback for PainterCallbackImpl {
    fn paint(&self, pixel_data_rgba: &[u8]) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.copy_from_slice(pixel_data_rgba);
        }
    }

    fn request_repaint(&self) {
        self.ctx.request_repaint();
    }
}

pub struct App {
    project_dirs: directories::ProjectDirs,
    thread: GbThread,
    screen: screen::GBScreen<{ ceres_std::PX_WIDTH as u32 }, { ceres_std::PX_HEIGHT as u32 }>,
    _rom_path: Option<PathBuf>,
    sav_path: Option<PathBuf>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model: ceres_std::Model,
        project_dirs: directories::ProjectDirs,
        rom_path: Option<&std::path::Path>,
        shader_option: ShaderOption,
    ) -> anyhow::Result<Self> {
        // Apply our minimal black and white theme
        setup_theme(&cc.egui_ctx);

        let sav_path = if let Some(rom_path) = rom_path {
            let file_stem = rom_path.file_stem().context("couldn't get file stem")?;

            Some(
                project_dirs
                    .data_dir()
                    .join(file_stem)
                    .with_extension("sav"),
            )
        } else {
            None
        };

        let pixel_data_rgba = Arc::new(Mutex::new(
            vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
        ));

        let mut thread = GbThread::new(
            model,
            sav_path.as_deref(),
            rom_path,
            PainterCallbackImpl::new(&cc.egui_ctx, Arc::clone(&pixel_data_rgba)),
        )?;

        let screen = screen::GBScreen::new(cc, pixel_data_rgba, shader_option);

        thread.resume()?;

        Ok(Self {
            project_dirs,
            thread,
            screen,
            _rom_path: rom_path.map(std::path::Path::to_path_buf),
            sav_path,
        })
    }

    fn save_data(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.project_dirs.data_dir())?;
        if let Some(sav_path) = &self.sav_path {
            let sav_file = File::create(sav_path);
            self.thread.save_data(&mut sav_file?)?;
        }

        Ok(())
    }
}

impl eframe::App for App {
    #[expect(clippy::too_many_lines)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |top_panel_ui| {
            egui::menu::bar(top_panel_ui, |menu_bar_ui| {
                menu_bar_ui.menu_button("File", |menu_button_ui| {
                    if menu_button_ui.button("Open").clicked() {
                        let file = FileDialog::new()
                            .add_filter("gb", &["gb", "gbc"])
                            .pick_file();

                        if let Some(file) = file {
                            if let Err(e) = self.thread.change_rom(self.sav_path.as_deref(), &file)
                            {
                                eprintln!("couldn't open ROM: {e}");
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
                        {
                            if let Err(e) = if paused {
                                self.thread.resume()
                            } else {
                                self.thread.pause()
                            } {
                                eprintln!("couldn't pause/resume: {e}");
                            }
                        }

                        let multiplier = self.thread.multiplier();

                        if horizontal_ui
                            .selectable_label(multiplier == 1, "1x")
                            .on_hover_text("Speed 1x")
                            .clicked()
                        {
                            self.thread.set_multiplier(1);
                        }

                        if horizontal_ui
                            .selectable_label(multiplier == 2, "2x")
                            .on_hover_text("Speed 2x")
                            .clicked()
                        {
                            self.thread.set_multiplier(2);
                        }

                        if horizontal_ui
                            .selectable_label(multiplier == 4, "4x")
                            .on_hover_text("Speed 4x")
                            .clicked()
                        {
                            self.thread.set_multiplier(4);
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
                            let shader_button = egui::SelectableLabel::new(
                                self.screen.shader_option() == shader_option,
                                shader_option.str(),
                            );

                            if menu_button_ui.add(shader_button).clicked() {
                                *self.screen.shader_option_mut() = shader_option;
                            }
                        }
                    });

                    menu_button_ui.menu_button("Scaling", |menu_button_ui| {
                        for pixel_mode in ScalingOption::iter() {
                            let pixel_button = egui::SelectableLabel::new(
                                self.screen.pixel_mode() == pixel_mode,
                                pixel_mode.str(),
                            );

                            if menu_button_ui.add(pixel_button).clicked() {
                                *self.screen.mut_pixel_mode() = pixel_mode;
                            }
                        }
                    });
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
            .show(ctx, |central_panel_ui| {
                self.screen.custom_painting(central_panel_ui);
            });

        ctx.input(|i| {
            if i.key_pressed(Key::W) {
                self.thread.press(ceres_std::Button::Up);
            }

            if i.key_released(Key::W) {
                self.thread.release(ceres_std::Button::Up);
            }

            if i.key_pressed(Key::A) {
                self.thread.press(ceres_std::Button::Left);
            }

            if i.key_released(Key::A) {
                self.thread.release(ceres_std::Button::Left);
            }

            if i.key_pressed(Key::S) {
                self.thread.press(ceres_std::Button::Down);
            }

            if i.key_released(Key::S) {
                self.thread.release(ceres_std::Button::Down);
            }

            if i.key_pressed(Key::D) {
                self.thread.press(ceres_std::Button::Right);
            }

            if i.key_released(Key::D) {
                self.thread.release(ceres_std::Button::Right);
            }

            if i.key_pressed(Key::L) {
                self.thread.press(ceres_std::Button::A);
            }

            if i.key_released(Key::L) {
                self.thread.release(ceres_std::Button::A);
            }

            if i.key_pressed(Key::K) {
                self.thread.press(ceres_std::Button::B);
            }

            if i.key_released(Key::K) {
                self.thread.release(ceres_std::Button::B);
            }

            if i.key_pressed(Key::M) {
                self.thread.press(ceres_std::Button::Start);
            }

            if i.key_released(Key::M) {
                self.thread.release(ceres_std::Button::Start);
            }

            if i.key_pressed(Key::N) {
                self.thread.press(ceres_std::Button::Select);
            }

            if i.key_released(Key::N) {
                self.thread.release(ceres_std::Button::Select);
            }
        });
    }

    fn on_exit(&mut self) {
        if let Err(e) = self.save_data() {
            eprintln!("couldn't save data: {e}");
        }
    }
}
