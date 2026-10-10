//! Debug helper: runs a test ROM from the boot ROM for up to N frames (or
//! until its `ld b, b` breakpoint) and saves the screen as a PNG.
//!
//! Usage: `cargo run -p ceres-test-runner --example dump_frame -- <rom> <dmg|cgb|cgbc> <frames> <out.png>`
//! (`<rom>` is under `external/test-roms`; `cgb` is a CGB-E.)

use ceres_core::{GbBuilder, Model, PX_HEIGHT, PX_WIDTH};
use ceres_test_runner::{DummyAudioCallback, load_test_rom};

const USAGE: &str = "usage: dump_frame <rom> <dmg|cgb|cgbc> <frames> <out.png>";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [rom, model, frames, out] = args.as_slice() else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let model = match model.as_str() {
        "dmg" => Model::DmgB,
        "cgb" => Model::CgbE,
        "cgbc" => Model::CgbC,
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    let Ok(frames) = frames.parse::<u32>() else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };

    let rom = load_test_rom(rom).expect("load ROM");
    let mut gb = GbBuilder::new(48000, DummyAudioCallback)
        .with_model(model)
        .with_rom(rom.into_boxed_slice())
        .expect("valid ROM")
        .build();
    gb.set_color_correction_mode(ceres_core::ColorCorrectionMode::Disabled);

    // Stop at the `ld b, b` breakpoint, like the screenshot tests.
    for _ in 0..frames {
        gb.run_frame();
        if gb.take_ld_b_b_breakpoint() {
            break;
        }
    }

    image::save_buffer(
        out,
        gb.pixel_data_rgba(),
        u32::from(PX_WIDTH),
        u32::from(PX_HEIGHT),
        image::ColorType::Rgba8,
    )
    .expect("save PNG");
    println!("wrote {out}");
}
