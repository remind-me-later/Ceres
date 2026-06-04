//! Diagnostic driver: runs cgb-acid2 to completion, then dumps the
//! actual pixel buffer to disk for comparison with the reference.

use ceres_core::{ColorCorrectionMode, GbBuilder, Model};
use ceres_test_runner::{load_test_rom, test_runner::DummyAudioCallback};

const FRAMES: u32 = 300;

fn main() {
    let rom = load_test_rom("cgb-acid2/cgb-acid2.gbc").expect("load cgb-acid2");
    let mut gb = GbBuilder::new(48000, DummyAudioCallback::default())
        .with_model(Model::CgbE)
        .with_run_bootrom(true)
        .with_rom(rom.into_boxed_slice())
        .expect("build")
        .build();
    gb.set_color_correction_mode(ColorCorrectionMode::Disabled);

    let mut reached_breakpoint = false;
    let mut last_breakpoint_frame = 0;
    for frame in 0..FRAMES {
        gb.run_frame();
        if gb.check_and_reset_ld_b_b_breakpoint() {
            reached_breakpoint = true;
            last_breakpoint_frame = frame;
            eprintln!("ld b,b breakpoint hit at frame {frame}");
            // keep running — the test runner keeps going until timeout
        }
    }

    if !reached_breakpoint {
        eprintln!("did not reach breakpoint within {FRAMES} frames");
    } else {
        eprintln!("last breakpoint was at frame {last_breakpoint_frame}, current frame {FRAMES}");
    }

    let pixels = gb.pixel_data_rgba();
    let img = image::RgbaImage::from_raw(
        u32::from(ceres_core::PX_WIDTH),
        u32::from(ceres_core::PX_HEIGHT),
        pixels.to_vec(),
    )
    .expect("image dimensions");
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/cgb_acid2_actual.png".to_string());
    img.save(&out_path).expect("save image");
    eprintln!("wrote {out_path}");

    // Count diff pixels vs reference
    let expected_path = "external/test-roms/cgb-acid2/cgb-acid2.png";
    if let Ok(expected_img) = image::open(expected_path) {
        let expected_rgba = expected_img.to_rgba8();
        let mut diff_count: u64 = 0;
        let mut sum_dist: u64 = 0;
        let mut first_diffs: Vec<(u32, u32, [u8; 4], [u8; 4])> = Vec::new();
        for y in 0..u32::from(ceres_core::PX_HEIGHT) {
            for x in 0..u32::from(ceres_core::PX_WIDTH) {
                let idx = ((y * u32::from(ceres_core::PX_WIDTH) + x) * 4) as usize;
                let a = [
                    pixels[idx],
                    pixels[idx + 1],
                    pixels[idx + 2],
                    pixels[idx + 3],
                ];
                let b = [
                    expected_rgba.as_raw()[idx],
                    expected_rgba.as_raw()[idx + 1],
                    expected_rgba.as_raw()[idx + 2],
                    expected_rgba.as_raw()[idx + 3],
                ];
                if a != b {
                    diff_count += 1;
                    let d = a[0].abs_diff(b[0]) as u64
                        + a[1].abs_diff(b[1]) as u64
                        + a[2].abs_diff(b[2]) as u64;
                    sum_dist += d;
                    if first_diffs.len() < 20 {
                        first_diffs.push((x, y, a, b));
                    }
                }
            }
        }
        eprintln!(
            "diff: {diff_count} pixels differ, total channel distance {sum_dist} ({}% pixels match)",
            100 - (diff_count * 100 / (u32::from(ceres_core::PX_WIDTH) * u32::from(ceres_core::PX_HEIGHT)) as u64)
        );
        for (x, y, a, b) in first_diffs {
            eprintln!("  ({x:3},{y:3}) actual=#{:02X}{:02X}{:02X}{:02X} expected=#{:02X}{:02X}{:02X}{:02X}",
                a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]);
        }
    } else {
        eprintln!("no reference screenshot at {expected_path}");
    }
}
