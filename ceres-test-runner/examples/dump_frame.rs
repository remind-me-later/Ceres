//! Debug helper: run a test ROM for N frames and dump the framebuffer as PNG.
//!
//! Usage: `cargo run -p ceres-test-runner --example dump_frame -- <rom> <model> <frames> <out.png>`

use ceres_core::{AudioCallback, Gb, GbBuilder, Model, Sample};
use ceres_test_runner::load_test_rom;

pub struct NoAudio;
impl AudioCallback for NoAudio {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rom_rel = &args[1];
    let model = match args[2].as_str() {
        "dmg" => Model::DmgB,
        "cgb" => Model::CgbE,
        "cgbc" => Model::CgbC,
        other => panic!("unknown model {other}"),
    };
    let frames: u32 = args[3].parse().unwrap();
    let out = &args[4];

    let rom = load_test_rom(rom_rel).expect("load rom");
    let mut gb: Gb<NoAudio> = GbBuilder::new(48000, NoAudio)
        .with_model(model)
        .with_run_bootrom(true)
        .with_rom(rom.into_boxed_slice())
        .expect("build gb")
        .build();
    gb.set_color_correction_mode(ceres_core::ColorCorrectionMode::Disabled);

    // Stop at the `ld b,b` breakpoint like the screenshot tests do.
    for _ in 0..frames {
        gb.run_frame();
        if gb.take_ld_b_b_breakpoint() {
            break;
        }
    }

    let rgba = gb.pixel_data_rgba();
    image::save_buffer(out, rgba, 160, 144, image::ColorType::Rgba8).expect("save png");
    println!("wrote {out}");
}
