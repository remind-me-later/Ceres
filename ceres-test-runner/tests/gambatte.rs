//! Integration tests using the Gambatte test ROM suite.
//!
//! These tests correspond to the Gambatte-derived unit tests in
//! `ceres-core/src/sm83/tests.rs`. The purpose is to verify that the same
//! hardware behaviors that pass at the unit-test level also pass end-to-end
//! with the full emulator stack (PPU, APU, memory, boot sequence, etc.).
//!
//! # Pass criteria
//!
//! These tests use the same pass criteria as the official Gambatte test runner
//! (`gambatte-core/test/testrunner.cpp`). The emulator is run for a fixed
//! number of frames (15 frames when skipping the boot ROM), and then the
//! screen output is checked against the expected result string encoded in
//! the ROM filename.

use ceres_core::Model;
use ceres_test_runner::{
    load_test_rom,
    test_runner::{CompletionCheck, DummyAudioCallback, TestConfig, TestResult, TestRunner},
};
use std::cell::Cell;

// ────────────────────────────────────────────────────────────────────────────
// Tiles for screen-based validation
// ────────────────────────────────────────────────────────────────────────────

#[rustfmt::skip]
const TILES: [[u8; 64]; 16] = [
    // 0
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,1,1,1,1,1,1,
    ],
    // 1
    [
        0,0,0,0,0,0,0,0,
        0,0,0,0,1,0,0,0,
        0,0,0,0,1,0,0,0,
        0,0,0,0,1,0,0,0,
        0,0,0,0,1,0,0,0,
        0,0,0,0,1,0,0,0,
        0,0,0,0,1,0,0,0,
        0,0,0,0,1,0,0,0,
    ],
    // 2
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,0,0,0,0,0,0,1,
        0,0,0,0,0,0,0,1,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
    ],
    // 3
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,0,0,0,0,0,0,1,
        0,0,0,0,0,0,0,1,
        0,0,1,1,1,1,1,1,
        0,0,0,0,0,0,0,1,
        0,0,0,0,0,0,0,1,
        0,1,1,1,1,1,1,1,
    ],
    // 4
    [
        0,0,0,0,0,0,0,0,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,1,1,1,1,1,1,
        0,0,0,0,0,0,0,1,
        0,0,0,0,0,0,0,1,
        0,0,0,0,0,0,0,1,
    ],
    // 5
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,1,1,1,1,1,0,
        0,0,0,0,0,0,0,1,
        0,0,0,0,0,0,0,1,
        0,1,1,1,1,1,1,0,
    ],
    // 6
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,1,1,1,1,1,1,
    ],
    // 7
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,0,0,0,0,0,0,1,
        0,0,0,0,0,0,1,0,
        0,0,0,0,0,1,0,0,
        0,0,0,0,1,0,0,0,
        0,0,0,1,0,0,0,0,
        0,0,0,1,0,0,0,0,
    ],
    // 8
    [
        0,0,0,0,0,0,0,0,
        0,0,1,1,1,1,1,0,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,0,1,1,1,1,1,0,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,0,1,1,1,1,1,0,
    ],
    // 9
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,1,1,1,1,1,1,
        0,0,0,0,0,0,0,1,
        0,0,0,0,0,0,0,1,
        0,1,1,1,1,1,1,1,
    ],
    // A
    [
        0,0,0,0,0,0,0,0,
        0,0,0,0,1,0,0,0,
        0,0,1,0,0,0,1,0,
        0,1,0,0,0,0,0,1,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
    ],
    // B
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,0,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,1,1,1,1,1,0,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,1,1,1,1,1,0,
    ],
    // C
    [
        0,0,0,0,0,0,0,0,
        0,0,1,1,1,1,1,0,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,1,
        0,0,1,1,1,1,1,0,
    ],
    // D
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,0,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,0,0,0,0,0,1,
        0,1,1,1,1,1,1,0,
    ],
    // E
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
    ],
    // F
    [
        0,0,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,1,1,1,1,1,1,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
        0,1,0,0,0,0,0,0,
    ],
];

fn tile_from_char(c: char) -> Option<&'static [u8; 64]> {
    let idx = match c {
        '0'..='9' => c as usize - '0' as usize,
        'a'..='f' => c as usize - 'a' as usize + 10,
        'A'..='F' => c as usize - 'A' as usize + 10,
        _ => return None,
    };
    Some(&TILES[idx])
}

// ────────────────────────────────────────────────────────────────────────────
// Completion check
// ────────────────────────────────────────────────────────────────────────────

/// Gambatte test completion check.
pub struct GambatteCheck {
    expected: String,
    frame: Cell<u32>,
}

impl GambatteCheck {
    #[must_use]
    pub fn new(expected: String) -> Self {
        Self {
            expected,
            frame: Cell::new(0),
        }
    }

    fn evaluate(&self, gb: &ceres_core::Gb<DummyAudioCallback>) -> TestResult {
        let actual_rgba = gb.pixel_data_rgba();

        for (i, c) in self.expected.chars().enumerate() {
            if let Some(expected_tile) = tile_from_char(c) {
                // The first character is shifted by 4 pixels due to the PPU fetcher delay.
                let x_offset = i * 8 + 4;
                if x_offset + 8 > 160 {
                    break;
                }

                if !self.tile_matches(actual_rgba, expected_tile, x_offset) {
                    return TestResult::Failed(format!(
                        "Framebuffer mismatch at tile {i} (expected '{c}')",
                    ));
                }
            } else {
                break;
            }
        }

        TestResult::Passed
    }

    fn tile_matches(&self, actual_rgba: &[u8], expected_tile: &[u8; 64], x_offset: usize) -> bool {
        for y in 0..8 {
            for x in 0..8 {
                let pixel_idx = (y * 160 + x_offset + x) * 4;
                let r = actual_rgba[pixel_idx];
                let g = actual_rgba[pixel_idx + 1];
                let b = actual_rgba[pixel_idx + 2];

                // Gambatte's tilesAreEqual uses & 0xF8F8F8 for comparison.
                // We consider a pixel black if its top 5 bits are all 0.
                let is_black = (r & 0xF8) == 0 && (g & 0xF8) == 0 && (b & 0xF8) == 0;
                let expected_black = expected_tile[y * 8 + x] != 0;

                if is_black != expected_black {
                    return false;
                }
            }
        }
        true
    }
}

impl CompletionCheck for GambatteCheck {
    fn check(&self, _gb: &mut ceres_core::Gb<DummyAudioCallback>) -> Option<TestResult> {
        let cur = self.frame.get();
        self.frame.set(cur + 1);
        None
    }

    fn on_timeout(&self, gb: &mut ceres_core::Gb<DummyAudioCallback>) -> TestResult {
        self.evaluate(gb)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Helper
// ────────────────────────────────────────────────────────────────────────────

fn parse_expected_outputs(filename: &str) -> (Option<String>, Option<String>) {
    let basename = filename.split('/').last().unwrap_or(filename);
    let s = basename
        .strip_suffix(".gbc")
        .or_else(|| basename.strip_suffix(".gb"))
        .unwrap_or(basename);

    let mut dmg_out = None;
    let mut cgb_out = None;

    if let Some(pos) = s.find("dmg08_cgb04c_out") {
        let val = s[pos + 16..].to_string();
        dmg_out = Some(val.clone());
        cgb_out = Some(val);
    } else {
        if let Some(pos) = s.find("dmg08_out") {
            dmg_out = Some(s[pos + 9..].to_string());
        }
        if let Some(pos) = s.find("cgb04c_out") {
            cgb_out = Some(s[pos + 10..].to_string());
        } else if let Some(pos) = s.find("_out") {
            if cgb_out.is_none() {
                cgb_out = Some(s[pos + 4..].to_string());
            }
        }
    }

    (dmg_out, cgb_out)
}

/// Run a Gambatte test ROM.
///
/// This function parses the expected model and output string directly from the
/// ROM filename, matching the official Gambatte test runner logic.
fn run_gambatte_test(relative_path: &str) -> TestResult {
    let (dmg_out, cgb_out) = parse_expected_outputs(relative_path);

    let rom_data = match load_test_rom(relative_path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    if let Some(expected) = dmg_out {
        let config = TestConfig {
            model: Model::DmgB,
            timeout_frames: 15,
            test_name: format!("{} (DMG)", relative_path),
            run_bootrom: false,
            ..TestConfig::default()
        };

        let mut runner = match TestRunner::new(
            rom_data.clone(),
            config,
            Box::new(GambatteCheck::new(expected)),
        ) {
            Ok(runner) => runner,
            Err(e) => return TestResult::Error(format!("Failed to create DMG test runner: {e}")),
        };

        let res = runner.run();
        if res != TestResult::Passed {
            return res;
        }
    }

    if let Some(expected) = cgb_out {
        let config = TestConfig {
            model: Model::CgbE,
            timeout_frames: 15,
            test_name: format!("{} (CGB)", relative_path),
            run_bootrom: false,
            ..TestConfig::default()
        };

        let mut runner =
            match TestRunner::new(rom_data, config, Box::new(GambatteCheck::new(expected))) {
                Ok(runner) => runner,
                Err(e) => {
                    return TestResult::Error(format!("Failed to create CGB test runner: {e}"));
                }
            };

        let res = runner.run();
        if res != TestResult::Passed {
            return res;
        }
    }

    TestResult::Passed
}

macro_rules! gambatte_test {
    ($name:ident, $path:expr) => {
        #[test]
        fn $name() {
            let result = run_gambatte_test($path);
            assert_eq!(result, TestResult::Passed, "{result:?}");
        }
    };
}

// ────────────────────────────────────────────────────────────────────────────
// Undefined opcode tests
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_undef_op_d3,
    "gambatte/undef_ops/undef_op_d3_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_db,
    "gambatte/undef_ops/undef_op_db_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_e3,
    "gambatte/undef_ops/undef_op_e3_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_e4,
    "gambatte/undef_ops/undef_op_e4_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_eb,
    "gambatte/undef_ops/undef_op_eb_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_ec,
    "gambatte/undef_ops/undef_op_ec_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_ed,
    "gambatte/undef_ops/undef_op_ed_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_f4,
    "gambatte/undef_ops/undef_op_f4_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_fc,
    "gambatte/undef_ops/undef_op_fc_dmg08_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_undef_op_fd,
    "gambatte/undef_ops/undef_op_fd_dmg08_cgb04c_out01.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// HALT bug tests
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_halt_bug_noime_lda_3c,
    "gambatte/halt/noime_ifandie_halt_lda_3c_dmg08_cgb04c_out3F.gbc"
);

gambatte_test!(
    gambatte_halt_bug_noime_sra,
    "gambatte/halt/noime_ifandie_halt_sra_dmg08_cgb04c_outF1.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// IRQ precedence / IF-clobber tests
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_irq_precedence_if_and_ie_0_if_1,
    "gambatte/irq_precedence/if_and_ie_0_if_1_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_irq_precedence_if_and_ie_0_if_2,
    "gambatte/irq_precedence/if_and_ie_0_if_2_dmg08_cgb04c_outE1.gbc"
);

gambatte_test!(
    gambatte_irq_precedence_if_and_ie_0_vector_1,
    "gambatte/irq_precedence/if_and_ie_0_vector_1_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_irq_precedence_if_and_ie_0_vector_2,
    "gambatte/irq_precedence/if_and_ie_0_vector_2_dmg08_cgb04c_out50.gbc"
);

gambatte_test!(
    gambatte_irq_precedence_if_and_ie_0_vector_3,
    "gambatte/irq_precedence/if_and_ie_0_vector_3_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_irq_precedence_if_and_ie_0_vector_4,
    "gambatte/irq_precedence/if_and_ie_0_vector_4_dmg08_cgb04c_out50.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// OAM access timing tests
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_oam_access_10spritesprline_postread_1,
    "gambatte/oam_access/10spritesprline_postread_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_10spritesprline_postread_2,
    "gambatte/oam_access/10spritesprline_postread_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_midread_1,
    "gambatte/oam_access/midread_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_midread_2,
    "gambatte/oam_access/midread_2_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_midread_3,
    "gambatte/oam_access/midread_3_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_midwrite_1,
    "gambatte/oam_access/midwrite_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_midwrite_2,
    "gambatte/oam_access/midwrite_2_dmg08_out1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_midwrite_3,
    "gambatte/oam_access/midwrite_3_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_1,
    "gambatte/oam_access/postread_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_2,
    "gambatte/oam_access/postread_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_scx2_1,
    "gambatte/oam_access/postread_scx2_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_scx2_2,
    "gambatte/oam_access/postread_scx2_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_scx3_1,
    "gambatte/oam_access/postread_scx3_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_scx3_2,
    "gambatte/oam_access/postread_scx3_2_dmg08_xout1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_scx3_3,
    "gambatte/oam_access/postread_scx3_3_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_scx5_1,
    "gambatte/oam_access/postread_scx5_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_scx5_2,
    "gambatte/oam_access/postread_scx5_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_postwrite_1,
    "gambatte/oam_access/postwrite_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_postwrite_2,
    "gambatte/oam_access/postwrite_2_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_oam_access_postwrite_2_scx3,
    "gambatte/oam_access/postwrite_2_scx3_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_oam_access_preread_1,
    "gambatte/oam_access/preread_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_preread_2,
    "gambatte/oam_access/preread_2_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_preread_lcdoffset1_1,
    "gambatte/oam_access/preread_lcdoffset1_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_preread_lcdoffset1_2,
    "gambatte/oam_access/preread_lcdoffset1_2_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_prewrite_1,
    "gambatte/oam_access/prewrite_1_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_oam_access_prewrite_2,
    "gambatte/oam_access/prewrite_2_dmg08_out1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_prewrite_3,
    "gambatte/oam_access/prewrite_3_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_oam_access_prewrite_lcdoffset1_1,
    "gambatte/oam_access/prewrite_lcdoffset1_1_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_oam_access_prewrite_lcdoffset1_2,
    "gambatte/oam_access/prewrite_lcdoffset1_2_cgb04c_out0.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// Sprite count / Mode 3 duration tests
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_sprites_10spritesprline_m3stat_1,
    "gambatte/sprites/10spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_10spritesprline_m3stat_2,
    "gambatte/sprites/10spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_1spritesprline_m3stat_1,
    "gambatte/sprites/1spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_1spritesprline_m3stat_2,
    "gambatte/sprites/1spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_2spritesprline_m3stat_1,
    "gambatte/sprites/2spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_2spritesprline_m3stat_2,
    "gambatte/sprites/2spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_3spritesprline_m3stat_1,
    "gambatte/sprites/3spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_3spritesprline_m3stat_2,
    "gambatte/sprites/3spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_4spritesprline_m3stat_1,
    "gambatte/sprites/4spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_4spritesprline_m3stat_2,
    "gambatte/sprites/4spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_5spritesprline_m3stat_1,
    "gambatte/sprites/5spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_5spritesprline_m3stat_2,
    "gambatte/sprites/5spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_6spritesprline_m3stat_1,
    "gambatte/sprites/6spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_6spritesprline_m3stat_2,
    "gambatte/sprites/6spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_7spritesprline_m3stat_1,
    "gambatte/sprites/7spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_7spritesprline_m3stat_2,
    "gambatte/sprites/7spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_8spritesprline_m3stat_1,
    "gambatte/sprites/8spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_8spritesprline_m3stat_2,
    "gambatte/sprites/8spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sprites_9spritesprline_m3stat_1,
    "gambatte/sprites/9spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_9spritesprline_m3stat_2,
    "gambatte/sprites/9spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc"
);

#[test]
fn gambatte_sprites_10spritesprline_10xposa7_m3stat_1() {
    let result = run_gambatte_test(
        "gambatte/sprites/10spritesPrLine_10xposA7_m3stat_1_dmg08_cgb04c_out3.gbc",
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_10spritesprline_10xposa7_m3stat_2() {
    let result = run_gambatte_test(
        "gambatte/sprites/10spritesPrLine_10xposA7_m3stat_2_dmg08_cgb04c_out0.gbc",
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

gambatte_test!(
    gambatte_sprites_10spritesprline_1xpos0_m3stat_1,
    "gambatte/sprites/10spritesPrLine_1xpos0_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_sprites_10spritesprline_1xpos0_m3stat_2,
    "gambatte/sprites/10spritesPrLine_1xpos0_m3stat_2_dmg08_cgb04c_out0.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// PPU timing and interrupt tests (DMG focus)
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_lycint_lycirq_1,
    "gambatte/lycint_lycirq/lycint_lycirq_1_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_m2int_m2irq_1,
    "gambatte/m2int_m2irq/m2int_m2irq_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0irq_1,
    "gambatte/m0int_m0irq/m0int_m0irq_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycint_m1stat_1,
    "gambatte/m1/lycint_m1stat_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lyc143_m1irq_1,
    "gambatte/m1/lycint143_m1irq_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1irq_late_enable_1,
    "gambatte/m1/m1irq_late_enable_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m1irq_m0disable_1,
    "gambatte/m1/m1irq_m0disable_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_lycint_ly_1,
    "gambatte/lycint_ly/lycint_ly_1_dmg08_cgb04c_out5.gbc"
);

gambatte_test!(
    gambatte_lycint_lycflag_1,
    "gambatte/lycint_lycflag/lycint_lycflag_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2int_m0stat_1,
    "gambatte/m2int_m0stat/m2int_m0stat_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx2_1,
    "gambatte/m0int_m0stat/m0int_m0stat_scx2_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx2_2,
    "gambatte/m0int_m0stat/m0int_m0stat_scx2_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx3_1,
    "gambatte/m0int_m0stat/m0int_m0stat_scx3_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx3_2,
    "gambatte/m0int_m0stat/m0int_m0stat_scx3_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2int_m2stat_1,
    "gambatte/m2int_m2stat/m2int_m2stat_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2int_m2stat_2,
    "gambatte/m2int_m2stat/m2int_m2stat_2_dmg08_cgb04c_out3.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// PPU enable/disable timing tests
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_scx_m3_extend_2,
    "gambatte/scx_during_m3/scx_m3_extend_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0_disable_1,
    "gambatte/m0enable/disable_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0_disable_3,
    "gambatte/m0enable/disable_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_enable_1,
    "gambatte/m0enable/m0_enable_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_enable_2,
    "gambatte/m0enable/m0_enable_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_enable_3,
    "gambatte/m0enable/m0_enable_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_enable_4,
    "gambatte/m0enable/m0_enable_4_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_enable_5,
    "gambatte/m0enable/m0_enable_5_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_reenable_1,
    "gambatte/m0enable/reenable_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_reenable_2,
    "gambatte/m0enable/reenable_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_disable_scx1_1,
    "gambatte/m0enable/disable_scx1_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0_disable_scx1_2,
    "gambatte/m0enable/disable_scx1_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0_disable_scx2_1,
    "gambatte/m0enable/disable_scx2_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0_disable_scx2_2,
    "gambatte/m0enable/disable_scx2_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_window_late_disable_0,
    "gambatte/window/late_disable_0_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_window_m2int_wxa6_m0irq_1,
    "gambatte/window/m2int_wxA6_m0irq_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2_disable_2,
    "gambatte/m2enable/disable_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2_disable_by_m1enable_ly0_1,
    "gambatte/m2enable/disable_by_m1enable_ly0_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2_late_enable_1,
    "gambatte/m2enable/late_enable_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2_late_enable_ly0_1,
    "gambatte/m2enable/late_enable_ly0_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_window_late_disable_early_scx03_wx0f_1,
    "gambatte/window/late_disable_early_scx03_wx0f_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_window_late_disable_early_scx03_wx10_1,
    "gambatte/window/late_disable_early_scx03_wx10_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_window_late_disable_early_scx03_wx11_1,
    "gambatte/window/late_disable_early_scx03_wx11_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_window_late_disable_early_scx03_wx12_1,
    "gambatte/window/late_disable_early_scx03_wx12_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_window_late_disable_late_scx03_wx11_1,
    "gambatte/window/late_disable_late_scx03_wx11_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_window_late_disable_late_scx03_wx12_1,
    "gambatte/window/late_disable_late_scx03_wx12_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycint_m0stat_1,
    "gambatte/lycint_m0stat/lycint_m0stat_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycint_m0stat_2,
    "gambatte/lycint_m0stat/lycint_m0stat_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2int_m3stat_1,
    "gambatte/m2int_m3stat/m2int_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_m2int_m3stat_2,
    "gambatte/m2int_m3stat/m2int_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m3stat_1,
    "gambatte/m0int_m3stat/m0int_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_m0int_m3stat_2,
    "gambatte/m0int_m3stat/m0int_m3stat_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lyc0int_m0irq_1,
    "gambatte/lyc0int_m0irq/lyc0int_m0irq_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lyc0int_m0irq_2,
    "gambatte/lyc0int_m0irq/lyc0int_m0irq_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc153int_m2irq_1,
    "gambatte/lyc153int_m2irq/lyc153int_m2irq_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lyc153int_m2irq_2,
    "gambatte/lyc153int_m2irq/lyc153int_m2irq_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0irq_2,
    "gambatte/m0int_m0irq/m0int_m0irq_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2int_m0irq_1,
    "gambatte/m2int_m0irq/m2int_m0irq_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2int_m0irq_2,
    "gambatte/m2int_m0irq/m2int_m0irq_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lycint_lycirq_2,
    "gambatte/lycint_lycirq/lycint_lycirq_2_dmg08_cgb04c_out3.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// Timer accuracy tests
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_div_start_inc_1_cgb,
    "gambatte/div/start_inc_1_cgb04c_out1E.gbc"
);

gambatte_test!(
    gambatte_div_start_inc_2_cgb,
    "gambatte/div/start_inc_2_cgb04c_out1F.gbc"
);

gambatte_test!(
    gambatte_div_start_stop1_inc_1,
    "gambatte/div/start_stop1_inc_1_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_div_start_stop1_inc_2,
    "gambatte/div/start_stop1_inc_2_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_div_start_stop2_inc_1,
    "gambatte/div/start_stop2_inc_1_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_div_start_stop2_inc_2,
    "gambatte/div/start_stop2_inc_2_cgb04c_out01.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_div_write_start_1,
    "gambatte/tima/tc00_div_write_start_1_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_div_write_start_1,
    "gambatte/tima/tc01_div_write_start_1_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_1a,
    "gambatte/tima/tc00_late_div_write_1a_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_1b,
    "gambatte/tima/tc00_late_div_write_1b_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_div_write_1a,
    "gambatte/tima/tc01_late_div_write_1a_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_div_write_1b,
    "gambatte/tima/tc01_late_div_write_1b_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_ff_tma_3,
    "gambatte/tima/tc01_1stopstart_ff_tma_3_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_stop_inc_2,
    "gambatte/tima/tc01_late_stop_inc_2_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_ff_tma_2,
    "gambatte/tima/tc01_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset3_irq_1,
    "gambatte/tima/tc01_1stopstart_offset3_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_nop_div_write_start_2,
    "gambatte/tima/tc01_nop_div_write_start_2_dmg08_cgb04c_outF2.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_start_1_cgb04c_out_f0,
    "gambatte/tima/tc00_start_1_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_div_write_2a,
    "gambatte/tima/tc01_late_div_write_2a_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_stop_inc_1,
    "gambatte/tima/tc00_late_stop_inc_1_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_div_write_3a,
    "gambatte/tima/tc01_late_div_write_3a_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_ff_tma_3,
    "gambatte/tima/tc01_ff_tma_3_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_fe_ff_2,
    "gambatte/tima/tc01_fe_ff_2_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_tima_tma_1,
    "gambatte/tima/tc01_late_tima_tma_1_dmg08_cgb04c_out11.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_div_write_start_2,
    "gambatte/tima/tc00_div_write_start_2_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_irq_2,
    "gambatte/tima/tc01_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_nop_div_write_start_1,
    "gambatte/tima/tc01_nop_div_write_start_1_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_1stopstart_offset2_ff_tma_1,
    "gambatte/tima/tc00_1stopstart_offset2_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset1_ff_tma_2,
    "gambatte/tima/tc01_1stopstart_offset1_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset2_irq_2,
    "gambatte/tima/tc01_1stopstart_offset2_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_stop_inc_1,
    "gambatte/tima/tc01_late_stop_inc_1_dmg08_cgb04c_outFD.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_4b,
    "gambatte/tima/tc00_late_div_write_4b_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_ff_tma_2,
    "gambatte/tima/tc01_1stopstart_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_irq_2,
    "gambatte/tima/tc01_1stopstart_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_irq_late_retrigger_3,
    "gambatte/tima/tc00_irq_late_retrigger_3_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_1stopstart_ff_tma_1,
    "gambatte/tima/tc00_1stopstart_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_if_2,
    "gambatte/tima/tc00_late_div_write_if_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_div_write_4a,
    "gambatte/tima/tc01_late_div_write_4a_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_stop_irq_2,
    "gambatte/tima/tc01_late_stop_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_tc01_5,
    "gambatte/tima/tc00_late_tc01_5_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_irq_2,
    "gambatte/tima/tc00_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_3a,
    "gambatte/tima/tc00_late_div_write_3a_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_tc01_3,
    "gambatte/tima/tc00_late_tc01_3_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_fe_ff_2,
    "gambatte/tima/tc00_fe_ff_2_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_stop_of_2,
    "gambatte/tima/tc01_late_stop_of_2_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset1_ff_tma_1,
    "gambatte/tima/tc01_1stopstart_offset1_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset1_irq_2,
    "gambatte/tima/tc01_1stopstart_offset1_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_div_write_2b,
    "gambatte/tima/tc01_late_div_write_2b_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_irq_ifw_2,
    "gambatte/tima/tc00_irq_ifw_2_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_tima_tma_3,
    "gambatte/tima/tc01_late_tima_tma_3_dmg08_cgb04c_out11.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_1stopstart_offset1_ff_tma_1,
    "gambatte/tima/tc00_1stopstart_offset1_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset1_ff_tma_3,
    "gambatte/tima/tc01_1stopstart_offset1_ff_tma_3_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_div_write_start_2,
    "gambatte/tima/tc01_div_write_start_2_dmg08_cgb04c_outF2.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_tima_tma_2,
    "gambatte/tima/tc01_late_tima_tma_2_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_stop_irq_2,
    "gambatte/tima/tc00_late_stop_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_tma_next_2,
    "gambatte/tima/tc01_tma_next_2_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_irq_1,
    "gambatte/tima/tc00_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_if_1b,
    "gambatte/tima/tc00_late_div_write_if_1b_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_ff_tma_2,
    "gambatte/tima/tc00_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_tma_2,
    "gambatte/tima/tc01_late_tma_2_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_fe_ff_1,
    "gambatte/tima/tc00_fe_ff_1_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_1stopstart_offset2_ff_tma_2,
    "gambatte/tima/tc00_1stopstart_offset2_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_ff_tma_1,
    "gambatte/tima/tc01_1stopstart_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_1stopstart_offset2_ff_tma_3,
    "gambatte/tima/tc00_1stopstart_offset2_ff_tma_3_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_tc01_1,
    "gambatte/tima/tc00_late_tc01_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset2_ff_tma_1,
    "gambatte/tima/tc01_1stopstart_offset2_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_tc01_late_tc00_of_2,
    "gambatte/tima/tc00_tc01_late_tc00_of_2_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_4a,
    "gambatte/tima/tc00_late_div_write_4a_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_tc01_2,
    "gambatte/tima/tc00_late_tc01_2_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_stop_irq_1,
    "gambatte/tima/tc01_late_stop_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset1_irq_1,
    "gambatte/tima/tc01_1stopstart_offset1_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_nop_div_write_start_2,
    "gambatte/tima/tc00_nop_div_write_start_2_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_if_1a,
    "gambatte/tima/tc00_late_div_write_if_1a_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_irq_1,
    "gambatte/tima/tc01_1stopstart_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_start_2_cgb04c_out_f1,
    "gambatte/tima/tc00_start_2_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_irq_late_retrigger_1,
    "gambatte/tima/tc00_irq_late_retrigger_1_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_stop_of_1,
    "gambatte/tima/tc01_late_stop_of_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_stop_inc_2,
    "gambatte/tima/tc00_late_stop_inc_2_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_1stopstart_offset1_ff_tma_2,
    "gambatte/tima/tc00_1stopstart_offset1_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_ff_tma_1,
    "gambatte/tima/tc01_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset3_ff_tma_3,
    "gambatte/tima/tc01_1stopstart_offset3_ff_tma_3_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_tc01_ff_tma_2,
    "gambatte/tima/tc00_tc01_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset3_ff_tma_1,
    "gambatte/tima/tc01_1stopstart_offset3_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_irq_1,
    "gambatte/tima/tc01_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_tma_1,
    "gambatte/tima/tc01_late_tma_1_dmg08_cgb04c_out11.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_tc01_8,
    "gambatte/tima/tc00_late_tc01_8_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset2_ff_tma_2,
    "gambatte/tima/tc01_1stopstart_offset2_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset2_ff_tma_3,
    "gambatte/tima/tc01_1stopstart_offset2_ff_tma_3_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_tc01_7,
    "gambatte/tima/tc00_late_tc01_7_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset3_irq_2,
    "gambatte/tima/tc01_1stopstart_offset3_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_fe_ff_1,
    "gambatte/tima/tc01_fe_ff_1_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_tc01_late_tc00_of_1,
    "gambatte/tima/tc00_tc01_late_tc00_of_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_ff_tma_1,
    "gambatte/tima/tc00_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_tima_irq_1,
    "gambatte/tima/tc01_late_tima_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_1stopstart_ff_tma_2,
    "gambatte/tima/tc00_1stopstart_ff_tma_2_dmg08_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_div_write_3b,
    "gambatte/tima/tc00_late_div_write_3b_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_stop_of_1,
    "gambatte/tima/tc00_late_stop_of_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_div_write_3b,
    "gambatte/tima/tc01_late_div_write_3b_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_1stopstart_offset1_ff_tma_3,
    "gambatte/tima/tc00_1stopstart_offset1_ff_tma_3_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_tc01_ff_tma_1,
    "gambatte/tima/tc00_tc01_ff_tma_1_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_tima_inc_2,
    "gambatte/tima/tc01_late_tima_inc_2_dmg08_cgb04c_out10.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_nop_div_write_start_1,
    "gambatte/tima/tc00_nop_div_write_start_1_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_ff_tma_3,
    "gambatte/tima/tc00_ff_tma_3_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_irq_ifw_1,
    "gambatte/tima/tc00_irq_ifw_1_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset2_irq_1,
    "gambatte/tima/tc01_1stopstart_offset2_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_irq_late_retrigger_2,
    "gambatte/tima/tc00_irq_late_retrigger_2_dmg08_outE4_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_late_tc01_4,
    "gambatte/tima/tc00_late_tc01_4_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_tma_next_1,
    "gambatte/tima/tc01_tma_next_1_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_1stopstart_offset3_ff_tma_2,
    "gambatte/tima/tc01_1stopstart_offset3_ff_tma_2_dmg08_cgb04c_out00.gbc"
);
