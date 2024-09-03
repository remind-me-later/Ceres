use eframe::egui::{self, Event, Key};
use egui::mutex::Mutex;
use std::{fs::File, io::Write, sync::Arc};

use crate::{audio, gb_context::GbContext, screen, Scaling};

pub struct App {
    // Config parameters
    project_dirs: directories::ProjectDirs,

    // Contexts
    gb_ctx: Option<GbContext>,

    // Rendering
    _audio: audio::State,
    screen: screen::GBScreen,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model: ceres_core::Model,
        project_dirs: directories::ProjectDirs,
        rom_path: &std::path::Path,
        scaling: Scaling,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        let audio = audio::State::new().unwrap();
        let gb_ctx = GbContext::new(model, &project_dirs, rom_path, &audio, ctx).unwrap();
        let gb_clone = gb_ctx.gb_clone();
        let screen = screen::GBScreen::new(cc, gb_clone);

        Self {
            project_dirs,
            gb_ctx: Some(gb_ctx),
            _audio: audio,
            screen,
        }
    }

    fn save_data(&self) {
        if let Some(gb_ctx) = &self.gb_ctx {
            let gb = gb_ctx.gb_lock();
            if let Some(save_data) = gb.cartridge().save_data() {
                std::fs::create_dir_all(self.project_dirs.data_dir())
                    .expect("couldn't create data directory");
                let sav_file = File::create(
                    self.project_dirs
                        .data_dir()
                        .join(gb_ctx.rom_ident())
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
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.horizontal_top(|ui| {
                    if ui.button("Open ROM file").clicked() {
                        println!("Open ROM file");
                    }
                });

                ui.separator();

                egui::Frame::canvas(ui.style()).show(ui, |ui| {
                    self.screen.custom_painting(ui);
                });
            });

            // if ctx.input(|i| i.key_pressed(Key::Escape)) {
            //     self.using_gui = true;
            // }
        });
    }
}
