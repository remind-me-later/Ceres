use crate::{AppOption, PixelMode, ShaderOption, screen};
use anyhow::Context;
use ceres_std::GbThread;
use eframe::egui::{self, Key};
use rfd::FileDialog;
use std::{
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex},
};

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
    _audio: ceres_std::AudioState,
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

        let pixel_data_rgba = Arc::new(Mutex::new(
            vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
        ));

        let mut gb_ctx = GbThread::new(
            model,
            sav_path.as_deref(),
            rom_path,
            &audio,
            PainterCallbackImpl::new(&cc.egui_ctx, Arc::clone(&pixel_data_rgba)),
        )?;

        let screen = screen::GBScreen::new(cc, pixel_data_rgba, shader_option);

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
