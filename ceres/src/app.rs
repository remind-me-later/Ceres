use crate::{AppOption, PixelMode, ShaderOption, screen};
use anyhow::Context;
use ceres_std::GbThread;
use eframe::egui::{self, Key};
use rfd::FileDialog;
use std::{fs::File, path::PathBuf};

pub struct PainterCallbackImpl(egui::Context);

impl PainterCallbackImpl {
    pub fn new(ctx: &egui::Context) -> Self {
        Self(ctx.clone())
    }
}

impl ceres_std::PainterCallback for PainterCallbackImpl {
    fn repaint(&self) {
        self.0.request_repaint();
    }
}

pub struct App {
    project_dirs: directories::ProjectDirs,
    thread: GbThread,
    _audio: ceres_std::AudioState,
    screen: screen::GBScreen<{ ceres_core::PX_WIDTH as u32 }, { ceres_core::PX_HEIGHT as u32 }>,
    _rom_path: Option<PathBuf>,
    sav_path: Option<PathBuf>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model: ceres_core::Model,
        project_dirs: directories::ProjectDirs,
        rom_path: Option<&std::path::Path>,
        shader_option: ShaderOption,
    ) -> anyhow::Result<Self> {
        let audio = ceres_std::AudioState::new()?;
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

        let mut gb_ctx = GbThread::new(
            model,
            sav_path.as_deref(),
            rom_path,
            &audio,
            PainterCallbackImpl::new(&cc.egui_ctx),
        )?;
        let gb_clone = gb_ctx.gb_clone();
        let screen = screen::GBScreen::new(cc, gb_clone, shader_option);

        gb_ctx.resume()?;

        Ok(Self {
            project_dirs,
            thread: gb_ctx,
            _audio: audio,
            screen,
            _rom_path: rom_path.map(|path| path.to_path_buf()),
            sav_path,
        })
    }

    fn save_data(&self) -> anyhow::Result<()> {
        if let Ok(gb) = self.thread.gb_lock() {
            std::fs::create_dir_all(self.project_dirs.data_dir())?;
            if let Some(sav_path) = &self.sav_path {
                let sav_file = File::create(sav_path);
                gb.save_data(&mut sav_file?)?;
            }
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
                    if menu_button_ui.button("Open ROM").clicked() {
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

                    if menu_button_ui.button("Export save").clicked() {
                        println!("Export save file");
                    }
                });

                menu_bar_ui.menu_button("view", |menu_button_ui| {
                    menu_button_ui.add(egui::Label::new("Volume"));

                    menu_button_ui.horizontal(|horizontal_ui| {
                        let paused = self.thread.is_paused();
                        if horizontal_ui
                            .button(if paused { "\u{25b6}" } else { "\u{23f8}" })
                            .on_hover_text("Pause the game")
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

                        horizontal_ui.style_mut().spacing.slider_width = 50.0;

                        let volume_slider = egui::Slider::from_get_set(0.0..=1.0, |volume| {
                            if let Some(volume) = volume {
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

                        let mute_button = egui::Button::new(if self.thread.is_muted() {
                            "\u{1f507}"
                        } else {
                            "\u{1f50a}"
                        });

                        if horizontal_ui
                            .add(mute_button)
                            .on_hover_text("Mute the emulator")
                            .clicked()
                        {
                            self.thread.toggle_mute();
                        }
                    });

                    menu_button_ui.separator();

                    menu_button_ui.add(egui::Label::new("Shader"));

                    for shader_option in ShaderOption::iter() {
                        let shader_button = egui::SelectableLabel::new(
                            self.screen.shader_option() == shader_option,
                            shader_option.str(),
                        );

                        if menu_button_ui.add(shader_button).clicked() {
                            *self.screen.shader_option_mut() = shader_option;
                        }
                    }

                    menu_button_ui.separator();

                    menu_button_ui.add(egui::Label::new("Pixel mode"));

                    for pixel_mode in PixelMode::iter() {
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
            if let Ok(mut gb) = self.thread.mut_gb() {
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
            }
        });
    }

    fn on_exit(&mut self) {
        if let Err(e) = self.save_data() {
            eprintln!("couldn't save data: {e}");
        }
    }
}
