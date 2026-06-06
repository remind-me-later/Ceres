//! Integration tests using the Gambatte test ROM suite.
//!
//! These tests correspond to the Gambatte-derived unit tests in
//! `ceres-core/src/sm83/tests.rs`. The purpose is to verify that the same
//! hardware behaviors that pass at the unit-test level also pass end-to-end
//! with the full emulator stack (PPU, APU, memory, boot sequence, etc.).
//!
//! # Pass criteria
//!
//! These tests intercept the CPU right when it jumps to the `lprint_a`
//! routine (at address 0x7000). At this point, the test logic is complete
//! and the result is stored in the `A` register. This bypasses the need
//! to wait for the test to draw its result to the screen as tiles, which
//! improves execution speed and provides better error isolation.

use ceres_core::{GbBuilder, Model};
use ceres_test_runner::{
    load_test_rom,
    test_runner::{DummyAudioCallback, TestResult},
};

// ────────────────────────────────────────────────────────────────────────────
// Helper
// ────────────────────────────────────────────────────────────────────────────

fn parse_expected_outputs(filename: &str) -> (Option<u8>, Option<u8>) {
    let basename = filename.split('/').last().unwrap_or(filename);
    let s = basename
        .strip_suffix(".gbc")
        .or_else(|| basename.strip_suffix(".gb"))
        .unwrap_or(basename);

    let mut dmg_out = None;
    let mut cgb_out = None;

    let parse_hex = |val_str: &str| -> Option<u8> {
        // The output string might be e.g. "0A", "F1", "3", etc.
        // It's the hex representation of the A register.
        // In the original, they were strings of hex digits.
        u8::from_str_radix(val_str, 16).ok()
    };

    if let Some(pos) = s.find("dmg08_cgb04c_out") {
        let val_str = &s[pos + 16..];
        let val = parse_hex(val_str);
        dmg_out = val;
        cgb_out = val;
    } else {
        if let Some(pos) = s.find("dmg08_out") {
            let dmg_str = &s[pos + 9..];
            if let Some(end_pos) = dmg_str.find("_cgb04c_out") {
                dmg_out = parse_hex(&dmg_str[..end_pos]);
            } else {
                dmg_out = parse_hex(dmg_str);
            }
        }
        if let Some(pos) = s.find("cgb04c_out") {
            cgb_out = parse_hex(&s[pos + 10..]);
        } else if let Some(pos) = s.find("_out") {
            if cgb_out.is_none() && dmg_out.is_none() {
                cgb_out = parse_hex(&s[pos + 4..]);
            }
        }
    }

    (dmg_out, cgb_out)
}

/// Run a Gambatte test ROM.
///
/// This intercepts execution at PC == 0x7000 (`lprint_a`), checking the A
/// register against the expected test result.
fn run_gambatte_test(relative_path: &str) -> TestResult {
    let (dmg_out, cgb_out) = parse_expected_outputs(relative_path);

    let rom_data = match load_test_rom(relative_path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    // We'll give the test up to 15 frames of cycles to reach 0x7000.
    // 15 frames * ~70224 cycles/frame
    let timeout_cycles = 15 * 70224;

    if let Some(expected) = dmg_out {
        println!(
            "Running DMG test for {} expecting A={:02X}",
            relative_path, expected
        );

        let mut gb = match GbBuilder::new(48000, DummyAudioCallback::default())
            .with_model(Model::DmgB)
            .with_run_bootrom(false)
            .with_rom(rom_data.clone().into_boxed_slice())
        {
            Ok(builder) => builder.build(),
            Err(e) => return TestResult::Error(format!("Failed to build DMG: {e}")),
        };

        let mut reached_7000 = false;
        for _ in 0..timeout_cycles {
            gb.step_cpu();
            if gb.cpu_pc() == 0x7000 {
                let a = gb.cpu_a();
                if a == expected {
                    reached_7000 = true;
                    break;
                } else {
                    return TestResult::Failed(format!(
                        "Test failed (DMG). Expected A={:02X}, got A={:02X}",
                        expected, a
                    ));
                }
            }
        }

        if !reached_7000 {
            return TestResult::Failed("Test timed out (DMG) without reaching PC=0x7000".into());
        }
    }

    if let Some(expected) = cgb_out {
        println!(
            "Running CGB test for {} expecting A={:02X}",
            relative_path, expected
        );

        let model = if relative_path.contains("cgb04c") {
            Model::CgbC
        } else {
            Model::CgbE
        };

        let mut gb = match GbBuilder::new(48000, DummyAudioCallback::default())
            .with_model(model)
            .with_run_bootrom(false)
            .with_rom(rom_data.into_boxed_slice())
        {
            Ok(builder) => builder.build(),
            Err(e) => return TestResult::Error(format!("Failed to build CGB: {e}")),
        };

        let mut reached_7000 = false;
        for _ in 0..timeout_cycles {
            gb.step_cpu();
            if gb.cpu_pc() == 0x7000 {
                let a = gb.cpu_a();
                if a == expected {
                    reached_7000 = true;
                    break;
                } else {
                    return TestResult::Failed(format!(
                        "Test failed (CGB). Expected A={:02X}, got A={:02X}",
                        expected, a
                    ));
                }
            }
        }

        if !reached_7000 {
            return TestResult::Failed("Test timed out (CGB) without reaching PC=0x7000".into());
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

gambatte_test!(
    gambatte_lycint_m0stat_ds_1,
    "gambatte/lycint_m0stat/lycint_m0stat_ds_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycint_m0stat_ds_2,
    "gambatte/lycint_m0stat/lycint_m0stat_ds_2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_ds_1,
    "gambatte/m0int_m0stat/m0int_m0stat_ds_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_ds_2,
    "gambatte/m0int_m0stat/m0int_m0stat_ds_2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2int_m0irq_ds_1,
    "gambatte/m2int_m0irq/m2int_m0irq_ds_1_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_m2int_m0irq_ds_2,
    "gambatte/m2int_m0irq/m2int_m0irq_ds_2_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_ds_1,
    "gambatte/oam_access/postread_ds_1_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_oam_access_postread_ds_2,
    "gambatte/oam_access/postread_ds_2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_scx_m3_extend_1,
    "gambatte/scx_during_m3/scx_m3_extend_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_scx_m3_extend_ds_1,
    "gambatte/scx_during_m3/scx_m3_extend_ds_1_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_trigger_delay_1,
    "gambatte/lycEnable/lyc_ff45_trigger_delay_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_trigger_delay_2,
    "gambatte/lycEnable/lyc_ff45_trigger_delay_2_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_trigger_delay_ds_1,
    "gambatte/lycEnable/lyc_ff45_trigger_delay_ds_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_trigger_delay_1,
    "gambatte/lycEnable/lyc_ff41_trigger_delay_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_trigger_delay_2,
    "gambatte/lycEnable/lyc_ff41_trigger_delay_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lywrite77_ly44_m3_ly,
    "gambatte/lywrite/lywrite77_ly44_m3_ly_dmg08_cgb04c_out44.gbc"
);

gambatte_test!(
    gambatte_lywrite77_ly44_m3_stat,
    "gambatte/lywrite/lywrite77_ly44_m3_stat_dmg08_cgb04c_outC7.gbc"
);

gambatte_test!(
    gambatte_lywrite77_ly97_ly,
    "gambatte/lywrite/lywrite77_ly97_ly_dmg08_cgb04c_out97.gbc"
);

gambatte_test!(
    gambatte_lywrite77_ly97_stat,
    "gambatte/lywrite/lywrite77_ly97_stat_dmg08_cgb04c_outC5.gbc"
);

gambatte_test!(
    gambatte_m0_trigger_delay_1,
    "gambatte/m0enable/m0_trigger_delay_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0_trigger_delay_2,
    "gambatte/m0enable/m0_trigger_delay_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2_late_enable_1_v2,
    "gambatte/m2enable/late_enable_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m2_late_enable_2,
    "gambatte/m2enable/late_enable_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(gambatte_jpadirq_1, "gambatte/jpadirq_1.gbc");

gambatte_test!(
    gambatte_halt_m1int_ly_1,
    "gambatte/halt/m1int_ly_1_dmg08_cgb04c_out90.gbc"
);

gambatte_test!(
    gambatte_halt_m1int_ly_2,
    "gambatte/halt/m1int_ly_2_dmg08_out90_cgb04c_out91.gbc"
);

gambatte_test!(
    gambatte_halt_m0irq_m0stat_scx2_1,
    "gambatte/halt/m0irq_m0stat_scx2_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_scx_during_m3_scx_0367c0_1,
    "gambatte/scx_during_m3/scx_0367c0/scx_during_m3_1.gbc"
);

gambatte_test!(
    gambatte_scx_during_m3_scx_0367c0_ds_1,
    "gambatte/scx_during_m3/scx_0367c0/scx_during_m3_ds_1.gbc"
);

gambatte_test!(
    gambatte_window_m2int_wx03_scx3_m3stat_1,
    "gambatte/window/m2int_wx03_scx3_m3stat_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_window_late_enable_after_vblank_ds_1,
    "gambatte/window/late_enable_afterVblank_ds_1_cgb04c_out3.gbc"
);

gambatte_test!(gambatte_scy_during_m3_1, "gambatte/scy/scy_during_m3_1.gbc");

gambatte_test!(
    gambatte_scy_during_m3_ds_1,
    "gambatte/scy/scy_during_m3_ds_1.gbc"
);

gambatte_test!(
    gambatte_oam_access_preread_ds_2,
    "gambatte/oam_access/preread_ds_2_cgb04c_out3.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// MISC STAT IRQ
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_miscmstatirq_lcdoff_statirqen_if,
    "gambatte/miscmstatirq/lcdoff_statirqen_if_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_miscmstatirq_lycflag_statwirq_1,
    "gambatte/miscmstatirq/lycflag_statwirq_1_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_miscmstatirq_m0statwirq_1,
    "gambatte/miscmstatirq/m0statwirq_1_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_miscmstatirq_m1statwirq_1,
    "gambatte/miscmstatirq/m1statwirq_1_dmg08_out3.gb"
);

// ────────────────────────────────────────────────────────────────────────────
// Missing TIMA
// ────────────────────────────────────────────────────────────────────────────

gambatte_test!(
    gambatte_tima_tc00_start_3,
    "gambatte/tima/tc00_start_3_dmg08_outF0.gbc"
);

gambatte_test!(
    gambatte_tima_tc00_start_4,
    "gambatte/tima/tc00_start_4_dmg08_outF1.gbc"
);

gambatte_test!(
    gambatte_tima_tc01_late_tima_inc_1,
    "gambatte/tima/tc01_late_tima_inc_1_dmg08_cgb04c_out11.gbc"
);

// ────────────────────────────────────────────────────────────────────────────
// Non-PPU smoke tests — useful for triaging regressions in DMA, serial, APU,
// CGB speed switch, and CPU HALT/IME behaviour without PPU timing in the loop.
// ────────────────────────────────────────────────────────────────────────────

// DIV register increment behaviour on DMG (the 6 CGB variant are already
// in the suite; these are the DMG-only start_inc pairs).
gambatte_test!(
    gambatte_div_start_inc_1_dmg,
    "gambatte/div/start_inc_1_dmg08_outAB.gb"
);

gambatte_test!(
    gambatte_div_start_inc_2_dmg,
    "gambatte/div/start_inc_2_dmg08_outAC.gb"
);

// Serial: SC=0x81 written after a DIV write / NOPs — checks that the
// transfer-start timing and the IF flag are set correctly.
gambatte_test!(
    gambatte_serial_div_write_start_wait_read_if_1,
    "gambatte/serial/div_write_start_wait_read_if_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_serial_div_write_start_wait_read_if_2,
    "gambatte/serial/div_write_start_wait_read_if_2_dmg08_cgb04c_outE8.gbc"
);

gambatte_test!(
    gambatte_serial_nopx1_div_write_start_wait_read_if_1,
    "gambatte/serial/nopx1_div_write_start_wait_read_if_1_dmg08_cgb04c_outE0.gbc"
);

// CGB DMA logic (no PPU timing in the assertion — LY=0x99 is just a delay
// mechanism, the actual assertion is on the destination/source wrap result).
gambatte_test!(
    gambatte_dma_dst_wrap_1,
    "gambatte/dma/dma_dst_wrap_1_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_dma_src_wrap_1,
    "gambatte/dma/dma_src_wrap_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_dma_ff51_bits,
    "gambatte/dma/ff51_bits_cgb04c_outFF.gbc"
);

// APU: ch1 length-counter reset on DIV write, and ch1 init triggering the
// sweep counter — pure APU logic, no PPU involved.
gambatte_test!(
    gambatte_sound_ch1_div_write_reset_length_counter_timing_nr52_1,
    "gambatte/sound/ch1_div_write_reset_length_counter_timing_nr52_1_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_sound_ch1_init_reset_sweep_counter_timing_nr52_1,
    "gambatte/sound/ch1_init_reset_sweep_counter_timing_nr52_1_dmg08_cgb04c_out1.gbc"
);

// CGB speed switch: KEY1 register read after setting/unsetting the
// prepared bit. No PPU state is asserted.
gambatte_test!(
    gambatte_speedchange_key1_set,
    "gambatte/speedchange/key1_set_dmg08_outFF_cgb04c_out7F.gbc"
);

gambatte_test!(
    gambatte_speedchange_key1_set_unset,
    "gambatte/speedchange/key1_set_unset_dmg08_outFF_cgb04c_out7E.gbc"
);

// HALT/IME/IF: the HALT bug (EI + HALT executes the next instruction twice),
// IME off + HALT + SRA, and the IME-on-but-no-IRQ case. None of these read
// PPU registers, so they're independent of the scanline renderer.
gambatte_test!(
    gambatte_halt_ifandie_ei_halt_sra,
    "gambatte/halt/ifandie_ei_halt_sra_dmg08_cgb04c_out0A.gbc"
);

gambatte_test!(
    gambatte_halt_ime_noie_nolcdirq_readstat,
    "gambatte/halt/ime_noie_nolcdirq_readstat_dmg08_cgb_blank.gb"
);

gambatte_test!(
    gambatte_halt_noime_noie_nolcdirq_readstat,
    "gambatte/halt/noime_noie_nolcdirq_readstat_dmg08_cgb_blank.gb"
);

// ────────────────────────────────────────────────────────────────────────────
// Non-PPU smoke tests — second batch. Useful for triangulating regressions
// in serial, DMA, APU, CGB speed switch, and IF/IE interaction without
// the cycle-accurate PPU in the loop.
// ────────────────────────────────────────────────────────────────────────────

// Serial: basic start, read SB, read SC, clear IF, stop, trigger int, SC=0x80.
gambatte_test!(
    gambatte_serial_nopx1_start83_wait_read_if_1,
    "gambatte/serial/nopx1_start83_wait_read_if_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_serial_start_wait_read_if_1,
    "gambatte/serial/start_wait_read_if_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_serial_start_wait_read_sb_1,
    "gambatte/serial/start_wait_read_sb_1_dmg08_cgb04c_out7F.gbc"
);

gambatte_test!(
    gambatte_serial_start_wait_read_sc_1,
    "gambatte/serial/start_wait_read_sc_1_dmg08_outFF_cgb04c_outFD.gbc"
);

gambatte_test!(
    gambatte_serial_start_wait_clear_if_read_if_1,
    "gambatte/serial/start_wait_clear_if_read_if_1_dmg08_cgb04c_outE8.gbc"
);

gambatte_test!(
    gambatte_serial_start_wait_stop_read_if_1,
    "gambatte/serial/start_wait_stop_read_if_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_serial_start_wait_trigger_int8_read_if_1,
    "gambatte/serial/start_wait_trigger_int8_read_if_1_dmg08_cgb04c_outE8.gbc"
);

gambatte_test!(
    gambatte_serial_start_wait_sc80_read_if_1,
    "gambatte/serial/start_wait_sc80_read_if_1_dmg08_cgb04c_outE0.gbc"
);

// CGB DMA logic: read-side tests for each destination region (HRAM, OAM, VRAM)
// plus the HRAM-source result. The test waits for LY=0x99 as a delay mechanism;
// the assertion is on the DMA result, not on PPU timing.
gambatte_test!(
    gambatte_dma_hiram_read,
    "gambatte/dma/dma_hiram_read_cgb04c_out7.gbc"
);

gambatte_test!(
    gambatte_dma_hiram_read_result,
    "gambatte/dma/dma_hiram_read_result_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_dma_oam_read,
    "gambatte/dma/dma_oam_read_cgb04c_out7.gbc"
);

gambatte_test!(
    gambatte_dma_vram_read,
    "gambatte/dma/dma_vram_read_cgb04c_out7.gbc"
);

// APU ch1/ch2: late DIV write behaviour, length counter reset, init reset.
// These are pure APU state machine tests with no PPU involvement.
gambatte_test!(
    gambatte_sound_ch1_late_div_write_nr52_1a,
    "gambatte/sound/ch1_late_div_write_nr52_1a_dmg08_cgb04c_outF1.gbc"
);

gambatte_test!(
    gambatte_sound_ch1_late_div_write_nr52_1b,
    "gambatte/sound/ch1_late_div_write_nr52_1b_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_sound_ch2_div_write_reset_length_counter_timing_nr52_1,
    "gambatte/sound/ch2_div_write_reset_length_counter_timing_nr52_1_dmg08_cgb04c_outF2.gbc"
);

gambatte_test!(
    gambatte_sound_ch2_init_reset_length_counter_timing_nr52_1,
    "gambatte/sound/ch2_init_reset_length_counter_timing_nr52_1_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_sound_ch2_late_div_write_nr52_1a,
    "gambatte/sound/ch2_late_div_write_nr52_1a_dmg08_cgb04c_outF2.gbc"
);

gambatte_test!(
    gambatte_sound_ch2_late_div_write_nr52_1b,
    "gambatte/sound/ch2_late_div_write_nr52_1b_dmg08_cgb04c_outF0.gbc"
);

// CGB speed switch: DIV and TIMA behaviour across the speed change boundary.
// KEY1 reads after the change verify the prepared/current-speed bits.
gambatte_test!(
    gambatte_speedchange2_div_1,
    "gambatte/speedchange/speedchange2_div_1_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_speedchange2_div_nop_1,
    "gambatte/speedchange/speedchange2_div_nop_1_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_speedchange2_key1,
    "gambatte/speedchange/speedchange2_key1_cgb04c_out7E.gbc"
);

gambatte_test!(
    gambatte_speedchange2_tima00_1a,
    "gambatte/speedchange/speedchange2_tima00_1a_cgb04c_out00.gbc"
);

gambatte_test!(
    gambatte_speedchange2_tima01_1,
    "gambatte/speedchange/speedchange2_tima01_1_cgb04c_out09.gbc"
);

gambatte_test!(
    gambatte_speedchange2_tima02_1a,
    "gambatte/speedchange/speedchange2_tima02_1a_cgb04c_out02.gbc"
);

// IRQ precedence: the if_and_ie_0_* tests (which test pure IF/IE interaction
// without PPU STAT) are already in the suite. These additional non-PPU
// coverage comes from sound and speedchange variants below.

// APU ch1/ch2: ch2 late DIV write variants and ch1 init reset sweep counter
// variants. These complement the ch1/ch2 tests above.
gambatte_test!(
    gambatte_sound_ch2_late_div_write_nr52_2a,
    "gambatte/sound/ch2_late_div_write_nr52_2a_dmg08_cgb04c_outF2.gbc"
);

gambatte_test!(
    gambatte_sound_ch2_late_div_write_nr52_2b,
    "gambatte/sound/ch2_late_div_write_nr52_2b_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_sound_ch1_init_reset_sweep_counter_timing_nr52_2,
    "gambatte/sound/ch1_init_reset_sweep_counter_timing_nr52_2_dmg08_out0_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_sound_ch2_init_reset_length_counter_timing_nr52_2,
    "gambatte/sound/ch2_init_reset_length_counter_timing_nr52_2_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_speedchange2_tima01_nop_1,
    "gambatte/speedchange/speedchange2_tima01_nop_1_cgb04c_out0A.gbc"
);

gambatte_test!(
    gambatte_speedchange2_tima03_1a,
    "gambatte/speedchange/speedchange2_tima03_1a_cgb04c_out00.gbc"
);
gambatte_test!(
    gambatte_ifandie_ei_halt_m2int_m0stat_1_dmg08_cgb04c_out0,
    "gambatte/halt/ifandie_ei_halt_m2int_m0stat_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_ifandie_ei_halt_m2int_m0stat_2_dmg08_cgb04c_out2,
    "gambatte/halt/ifandie_ei_halt_m2int_m0stat_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx2_1a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0int_halt_m0stat_scx2_1a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx2_1b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx2_1b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx2_2a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0int_halt_m0stat_scx2_2a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx2_2b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx2_2b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx2_3a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0int_halt_m0stat_scx2_3a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx2_3b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx2_3b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx2_4a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0int_halt_m0stat_scx2_4a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx2_4b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx2_4b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_1a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_1a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_1b_dmg08_out0_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_1b_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_1c_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_1c_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_2a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_2a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_2b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_2b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_3a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_3a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_3b_dmg08_out0_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_3b_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_3c_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_3c_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_4a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_4a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_4b_dmg08_out0_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_4b_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0int_halt_m0stat_scx3_4c_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0int_halt_m0stat_scx3_4c_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_dec_scx2_1_dmg08_cgb04c_out7,
    "gambatte/halt/late_m0irq_halt_dec_scx2_1_dmg08_cgb04c_out7.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_dec_scx2_2_dmg08_cgb04c_out6,
    "gambatte/halt/late_m0irq_halt_dec_scx2_2_dmg08_cgb04c_out6.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_dec_scx3_1_dmg08_cgb04c_out7,
    "gambatte/halt/late_m0irq_halt_dec_scx3_1_dmg08_cgb04c_out7.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_dec_scx3_2_dmg08_cgb04c_out6,
    "gambatte/halt/late_m0irq_halt_dec_scx3_2_dmg08_cgb04c_out6.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx2_1a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0irq_halt_m0stat_scx2_1a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx2_1b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx2_1b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx2_2a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0irq_halt_m0stat_scx2_2a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx2_2b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx2_2b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx2_3a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0irq_halt_m0stat_scx2_3a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx2_3b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx2_3b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx2_4a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0irq_halt_m0stat_scx2_4a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx2_4b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx2_4b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_1a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_1a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_1b_dmg08_out0_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_1b_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_1c_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_1c_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_2a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_2a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_2b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_2b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_3a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_3a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_3b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_3b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_4a_dmg08_cgb04c_out0,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_4a_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_halt_m0stat_scx3_4b_dmg08_cgb04c_out2,
    "gambatte/halt/late_m0irq_halt_m0stat_scx3_4b_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lycirq_m2stat_1_dmg08_cgb04c_out2,
    "gambatte/halt/lycirq_m2stat_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lycirq_m2stat_2_dmg08_out2_cgb04c_out3,
    "gambatte/halt/lycirq_m2stat_2_dmg08_out2_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_lycirq_m2stat_3_dmg08_cgb04c_out3,
    "gambatte/halt/lycirq_m2stat_3_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx2_1_dmg08_cgb04c_out0,
    "gambatte/halt/m0int_m0stat_scx2_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx2_2_dmg08_cgb04c_out2,
    "gambatte/halt/m0int_m0stat_scx2_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx2_ds_1_cgb04c_out0,
    "gambatte/halt/m0int_m0stat_scx2_ds_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx2_ds_2_cgb04c_out2,
    "gambatte/halt/m0int_m0stat_scx2_ds_2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx3_1_dmg08_cgb04c_out0,
    "gambatte/halt/m0int_m0stat_scx3_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx3_2_dmg08_out0_cgb04c_out2,
    "gambatte/halt/m0int_m0stat_scx3_2_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx3_3_dmg08_cgb04c_out2,
    "gambatte/halt/m0int_m0stat_scx3_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx3_ds_1_cgb04c_out0,
    "gambatte/halt/m0int_m0stat_scx3_ds_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx3_ds_2_cgb04c_out2,
    "gambatte/halt/m0int_m0stat_scx3_ds_2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx4_1_dmg08_cgb04c_out0,
    "gambatte/halt/m0int_m0stat_scx4_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx4_2_dmg08_out0_cgb04c_out2,
    "gambatte/halt/m0int_m0stat_scx4_2_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx4_3_dmg08_cgb04c_out2,
    "gambatte/halt/m0int_m0stat_scx4_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx5_1_dmg08_cgb04c_out0,
    "gambatte/halt/m0int_m0stat_scx5_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0int_m0stat_scx5_2_dmg08_cgb04c_out2,
    "gambatte/halt/m0int_m0stat_scx5_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx2_2_dmg08_cgb04c_out2,
    "gambatte/halt/m0irq_m0stat_scx2_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx2_ds_1_cgb04c_out0,
    "gambatte/halt/m0irq_m0stat_scx2_ds_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx2_ds_2_cgb04c_out2,
    "gambatte/halt/m0irq_m0stat_scx2_ds_2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx3_1_dmg08_cgb04c_out0,
    "gambatte/halt/m0irq_m0stat_scx3_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx3_2_dmg08_out0_cgb04c_out2,
    "gambatte/halt/m0irq_m0stat_scx3_2_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx3_3_dmg08_cgb04c_out2,
    "gambatte/halt/m0irq_m0stat_scx3_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx3_ds_1_cgb04c_out0,
    "gambatte/halt/m0irq_m0stat_scx3_ds_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx3_ds_2_cgb04c_out2,
    "gambatte/halt/m0irq_m0stat_scx3_ds_2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx4_1_dmg08_cgb04c_out0,
    "gambatte/halt/m0irq_m0stat_scx4_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx4_2_dmg08_out0_cgb04c_out2,
    "gambatte/halt/m0irq_m0stat_scx4_2_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx4_3_dmg08_cgb04c_out2,
    "gambatte/halt/m0irq_m0stat_scx4_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx5_1_dmg08_cgb04c_out0,
    "gambatte/halt/m0irq_m0stat_scx5_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0irq_m0stat_scx5_2_dmg08_cgb04c_out2,
    "gambatte/halt/m0irq_m0stat_scx5_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m1int_ly_3_dmg08_cgb04c_out91,
    "gambatte/halt/m1int_ly_3_dmg08_cgb04c_out91.gbc"
);

gambatte_test!(
    gambatte_noime_ifandie_m2int_m0stat_1_dmg08_cgb04c_out0,
    "gambatte/halt/noime_ifandie_m2int_m0stat_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_noime_ifandie_m2int_m0stat_2_dmg08_cgb04c_out2,
    "gambatte/halt/noime_ifandie_m2int_m0stat_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_noime_m2irq_m0stat_1_dmg08_cgb04c_out0,
    "gambatte/halt/noime_m2irq_m0stat_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_noime_m2irq_m0stat_2_dmg08_cgb04c_out2,
    "gambatte/halt/noime_m2irq_m0stat_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_if_via_sp_if_1_dmg08_cgb04c_outFD,
    "gambatte/irq_precedence/late_if_via_sp_if_1_dmg08_cgb04c_outFD.gbc"
);

gambatte_test!(
    gambatte_late_if_via_sp_if_2_dmg08_cgb04c_outE0,
    "gambatte/irq_precedence/late_if_via_sp_if_2_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_retrigger_1_dmg08_cgb04c_outE2,
    "gambatte/irq_precedence/late_m0irq_retrigger_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_retrigger_2_dmg08_cgb04c_outE0,
    "gambatte/irq_precedence/late_m0irq_retrigger_2_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_retrigger_ds_1_cgb04c_outE2,
    "gambatte/irq_precedence/late_m0irq_retrigger_ds_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_retrigger_ds_2_cgb04c_outE0,
    "gambatte/irq_precedence/late_m0irq_retrigger_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_retrigger_scx1_1_dmg08_cgb04c_outE2,
    "gambatte/irq_precedence/late_m0irq_retrigger_scx1_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_retrigger_scx1_2_dmg08_cgb04c_outE0,
    "gambatte/irq_precedence/late_m0irq_retrigger_scx1_2_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_retrigger_scx1_ds_1_cgb04c_outE2,
    "gambatte/irq_precedence/late_m0irq_retrigger_scx1_ds_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_retrigger_scx1_ds_2_cgb04c_outE0,
    "gambatte/irq_precedence/late_m0irq_retrigger_scx1_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_vs_tima_scx2_1_dmg08_cgb04c_out4,
    "gambatte/irq_precedence/late_m0irq_vs_tima_scx2_1_dmg08_cgb04c_out4.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_vs_tima_scx2_2_dmg08_cgb04c_out2,
    "gambatte/irq_precedence/late_m0irq_vs_tima_scx2_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_vs_tima_scx2_halt_1_dmg08_cgb04c_out4,
    "gambatte/irq_precedence/late_m0irq_vs_tima_scx2_halt_1_dmg08_cgb04c_out4.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_vs_tima_scx2_halt_2_dmg08_cgb04c_out2,
    "gambatte/irq_precedence/late_m0irq_vs_tima_scx2_halt_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_vs_tima_scx3_1_dmg08_cgb04c_out4,
    "gambatte/irq_precedence/late_m0irq_vs_tima_scx3_1_dmg08_cgb04c_out4.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_vs_tima_scx3_2_dmg08_cgb04c_out2,
    "gambatte/irq_precedence/late_m0irq_vs_tima_scx3_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_vs_tima_scx3_halt_1_dmg08_cgb04c_out4,
    "gambatte/irq_precedence/late_m0irq_vs_tima_scx3_halt_1_dmg08_cgb04c_out4.gbc"
);

gambatte_test!(
    gambatte_late_m0irq_vs_tima_scx3_halt_2_dmg08_cgb04c_out2,
    "gambatte/irq_precedence/late_m0irq_vs_tima_scx3_halt_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_tc00_1stopstart_ff_tma_3_dmg08_cgb04c_outFE,
    "gambatte/tima/tc00_1stopstart_ff_tma_3_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tc00_irq_ds_1_cgb04c_outE0,
    "gambatte/tima/tc00_irq_ds_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tc00_irq_ds_2_cgb04c_outE4,
    "gambatte/tima/tc00_irq_ds_2_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tc00_irq_ifw_ds_1_cgb04c_outE4,
    "gambatte/tima/tc00_irq_ifw_ds_1_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tc00_irq_ifw_ds_2_cgb04c_outE0,
    "gambatte/tima/tc00_irq_ifw_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tc00_irq_late_retrigger_ds_1_cgb04c_outE4,
    "gambatte/tima/tc00_irq_late_retrigger_ds_1_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_tc00_irq_late_retrigger_ds_2_cgb04c_outE0,
    "gambatte/tima/tc00_irq_late_retrigger_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tc00_late_div_write_2a_dmg08_cgb04c_outFE,
    "gambatte/tima/tc00_late_div_write_2a_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tc00_late_div_write_2b_dmg08_cgb04c_outFF,
    "gambatte/tima/tc00_late_div_write_2b_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tc00_late_stop_irq_1_dmg08_cgb04c_outE0,
    "gambatte/tima/tc00_late_stop_irq_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_tc00_late_stop_of_2_dmg08_cgb04c_outFE,
    "gambatte/tima/tc00_late_stop_of_2_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tc00_late_tc01_6_dmg08_cgb04c_outFE,
    "gambatte/tima/tc00_late_tc01_6_dmg08_cgb04c_outFE.gbc"
);

gambatte_test!(
    gambatte_tc00_tc01_ff_tma_3_dmg08_cgb04c_outF0,
    "gambatte/tima/tc00_tc01_ff_tma_3_dmg08_cgb04c_outF0.gbc"
);

gambatte_test!(
    gambatte_tc01_late_div_write_4b_dmg08_cgb04c_outFF,
    "gambatte/tima/tc01_late_div_write_4b_dmg08_cgb04c_outFF.gbc"
);

gambatte_test!(
    gambatte_tc01_late_tima_irq_2_dmg08_cgb04c_outE4,
    "gambatte/tima/tc01_late_tima_irq_2_dmg08_cgb04c_outE4.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_00_00_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_00_00_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_00_40_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_00_40_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_00_bf_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_00_bf_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_00_ff_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_00_ff_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_40_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_40_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_40_40_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_40_40_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_40_bf_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_40_bf_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_40_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_40_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_bf_00_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_bf_00_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_bf_40_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_bf_40_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_bf_bf_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_bf_bf_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_bf_ff_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_bf_ff_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ff_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ff_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ff_40_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ff_40_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ff_bf_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ff_bf_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ff_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ff_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly00_10_50_1_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly00_10_50_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly00_10_50_2_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly00_10_50_2_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly00_10_50_ds_1_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly00_10_50_ds_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly00_10_50_ds_2_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly00_10_50_ds_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly00_10_50_ds_lcdoffset1_1_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly00_10_50_ds_lcdoffset1_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly00_10_50_ds_lcdoffset1_2_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly00_10_50_ds_lcdoffset1_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly00_10_50_lcdoffset3_1_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly00_10_50_lcdoffset3_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly00_10_50_lcdoffset3_2_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly00_10_50_lcdoffset3_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly94_00_50_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly94_00_50_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly94_10_40_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly94_10_40_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_ly94_10_50_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_ly94_10_50_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_1_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_2_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_2_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_3_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_3_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_4_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_4_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_5_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_5_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_6_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_6_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_7_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_7_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_8_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_8_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_9_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_early_ly44_lyc44_08_40_9_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_1_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_2_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_2_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_3_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_3_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_4_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_4_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_ds_1_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_ds_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_ds_2_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_ds_3_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_ds_3_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_ds_4_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_late_ly44_lyc44_08_40_ds_4_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_00_40_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_00_40_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_00_48_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_00_48_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_08_40_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_08_40_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_08_48_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_08_48_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_08_ff_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_08_ff_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_b7_40_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_b7_40_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_b7_f7_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_b7_f7_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_bf_40_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_bf_40_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycstatwirq_trigger_m0_ly44_lyc44_bf_ff_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycstatwirq_trigger_m0_ly44_lyc44_bf_ff_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_1_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_2_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_2_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_3_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_3_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_4_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_4_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_5_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_5_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_6_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_6_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_7_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_7_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_8_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_8_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_early_ly44_9_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_early_ly44_9_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_1_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_2_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_2_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_3_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_3_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_4_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_4_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_ds_1_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_ds_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_ds_2_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_ds_3_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_ds_3_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_ds_4_cgb04c_outE0,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_ds_4_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_1_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_2_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_2_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_3_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_3_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_4_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_4_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_5_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_5_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_ds_1_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_ds_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_ds_2_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_ds_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_ds_3_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_ds_3_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_m0_late_ly44_lyc45_ds_4_cgb04c_outE2,
    "gambatte/miscmstatirq/lycwirq_trigger_m0_late_ly44_lyc45_ds_4_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_00_00_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_00_00_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_00_08_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/m0statwirq_trigger_00_08_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_00_f7_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_00_f7_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_00_ff_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/m0statwirq_trigger_00_ff_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_08_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_08_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_08_08_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_08_08_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_08_f7_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_08_f7_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_08_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_08_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_f7_00_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_f7_00_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_f7_08_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/m0statwirq_trigger_f7_08_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_f7_f7_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_f7_f7_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_f7_ff_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/m0statwirq_trigger_f7_ff_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_ff_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_ff_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_ff_08_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_ff_08_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_ff_f7_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_ff_f7_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_ff_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m0statwirq_trigger_ff_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_ly44_lyc44_00_08_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/m0statwirq_trigger_ly44_lyc44_00_08_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_ly44_lyc44_40_08_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/m0statwirq_trigger_ly44_lyc44_40_08_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_m0statwirq_trigger_ly44_lyc44_40_48_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/m0statwirq_trigger_ly44_lyc44_40_48_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_00_00_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_00_00_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_00_10_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/m1statwirq_trigger_00_10_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_00_ef_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_00_ef_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_00_ff_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/m1statwirq_trigger_00_ff_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_10_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_10_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_10_10_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_10_10_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_10_ef_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_10_ef_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_10_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_10_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ef_00_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ef_00_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ef_10_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/m1statwirq_trigger_ef_10_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ef_ef_dmg08_out2_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ef_ef_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ef_ff_dmg08_out2_cgb04c_out2,
    "gambatte/miscmstatirq/m1statwirq_trigger_ef_ff_dmg08_out2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ff_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ff_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ff_10_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ff_10_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ff_ef_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ff_ef_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ff_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ff_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_00_10_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_00_10_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_00_50_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_00_50_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_40_10_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_40_10_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_40_50_1_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_40_50_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_40_50_2_dmg08_outE0_cgb04c_outE2,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_40_50_2_dmg08_outE0_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_40_50_3_dmg08_cgb04c_outE2,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_40_50_3_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_40_50_dmg08_cgb04c_outE0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_40_50_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_40_50_ds_1_cgb04c_outE0,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_40_50_ds_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_m1statwirq_trigger_ly94_lyc94_40_50_ds_2_cgb04c_outE2,
    "gambatte/miscmstatirq/m1statwirq_trigger_ly94_lyc94_40_50_ds_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_00_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_00_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_00_20_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_00_20_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_00_df_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_00_df_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_00_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_00_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_20_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_20_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_20_20_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_20_20_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_20_df_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_20_df_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_20_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_20_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_df_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_df_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_df_20_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_df_20_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_df_df_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_df_df_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_df_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_df_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_ff_00_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_ff_00_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_ff_20_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_ff_20_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_ff_df_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_ff_df_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_m2statwirq_trigger_ff_ff_dmg08_cgb04c_out0,
    "gambatte/miscmstatirq/m2statwirq_trigger_ff_ff_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lycflag_statwirq_1_dmg08_out2,
    "gambatte/miscmstatirq/lycflag_statwirq_1_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_lycflag_statwirq_2_dmg08_out2,
    "gambatte/miscmstatirq/lycflag_statwirq_2_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_lycflag_statwirq_3_dmg08_out2,
    "gambatte/miscmstatirq/lycflag_statwirq_3_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_lycflag_statwirq_4_dmg08_out0,
    "gambatte/miscmstatirq/lycflag_statwirq_4_dmg08_out0.gb"
);

gambatte_test!(
    gambatte_m0statwirq_1_dmg08_out2,
    "gambatte/miscmstatirq/m0statwirq_1_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_m0statwirq_2_dmg08_out0,
    "gambatte/miscmstatirq/m0statwirq_2_dmg08_out0.gb"
);

gambatte_test!(
    gambatte_m0statwirq_3_dmg08_out0,
    "gambatte/miscmstatirq/m0statwirq_3_dmg08_out0.gb"
);

gambatte_test!(
    gambatte_m0statwirq_4_dmg08_out2,
    "gambatte/miscmstatirq/m0statwirq_4_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_m0statwirq_scx2_1_dmg08_out0,
    "gambatte/miscmstatirq/m0statwirq_scx2_1_dmg08_out0.gb"
);

gambatte_test!(
    gambatte_m0statwirq_scx2_2_dmg08_out2,
    "gambatte/miscmstatirq/m0statwirq_scx2_2_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_m0statwirq_scx3_1_dmg08_out0,
    "gambatte/miscmstatirq/m0statwirq_scx3_1_dmg08_out0.gb"
);

gambatte_test!(
    gambatte_m0statwirq_scx3_2_dmg08_out2,
    "gambatte/miscmstatirq/m0statwirq_scx3_2_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_m0statwirq_scx5_1_dmg08_out0,
    "gambatte/miscmstatirq/m0statwirq_scx5_1_dmg08_out0.gb"
);

gambatte_test!(
    gambatte_m0statwirq_scx5_2_dmg08_out2,
    "gambatte/miscmstatirq/m0statwirq_scx5_2_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_m1statwirq_1_dmg08_out3,
    "gambatte/miscmstatirq/m1statwirq_1_dmg08_out3.gb"
);

gambatte_test!(
    gambatte_m1statwirq_2_dmg08_out3,
    "gambatte/miscmstatirq/m1statwirq_2_dmg08_out3.gb"
);

gambatte_test!(
    gambatte_m1statwirq_3_dmg08_out2,
    "gambatte/miscmstatirq/m1statwirq_3_dmg08_out2.gb"
);

gambatte_test!(
    gambatte_m1statwirq_4_dmg08_out0,
    "gambatte/miscmstatirq/m1statwirq_4_dmg08_out0.gb"
);

gambatte_test!(
    gambatte_m2disable_dmg08_cgb_dmg08_out0,
    "gambatte/miscmstatirq/m2disable_dmg08_cgb_dmg08_out0.gb"
);

gambatte_test!(
    gambatte_early_ff41_response_1_cgb04c_out0,
    "gambatte/lycEnable/early_ff41_response_1_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_early_ff41_response_2_cgb04c_out7,
    "gambatte/lycEnable/early_ff41_response_2_cgb04c_out7.gbc"
);

gambatte_test!(
    gambatte_early_ff45_response_1_dmg08_cgb04c_out0,
    "gambatte/lycEnable/early_ff45_response_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_early_ff45_response_2_dmg08_cgb04c_out7,
    "gambatte/lycEnable/early_ff45_response_2_dmg08_cgb04c_out7.gbc"
);

gambatte_test!(
    gambatte_ff40_disable_1_dmg08_cgb04c_out0,
    "gambatte/lycEnable/ff40_disable_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_ff40_disable_2_dmg08_cgb04c_out2,
    "gambatte/lycEnable/ff40_disable_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff41_disable_1_dmg08_cgb04c_out0,
    "gambatte/lycEnable/ff41_disable_1_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_ff41_disable_2_dmg08_out0_cgb04c_out2,
    "gambatte/lycEnable/ff41_disable_2_dmg08_out0_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff41_disable_3_dmg08_cgb04c_out2,
    "gambatte/lycEnable/ff41_disable_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff41_disable_ds_1_cgb04c_out1,
    "gambatte/lycEnable/ff41_disable_ds_1_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_ff41_disable_ds_2_cgb04c_out3,
    "gambatte/lycEnable/ff41_disable_ds_2_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff41_reenable_1_dmg08_cgb04c_out2,
    "gambatte/lycEnable/ff41_reenable_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff41_reenable_2_dmg08_cgb04c_out2,
    "gambatte/lycEnable/ff41_reenable_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff41_reenable_3_dmg08_cgb04c_out1,
    "gambatte/lycEnable/ff41_reenable_3_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_ff45_disable_1_dmg08_cgb04c_out1,
    "gambatte/lycEnable/ff45_disable_1_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_ff45_disable_2_dmg08_out1_cgb04c_out3,
    "gambatte/lycEnable/ff45_disable_2_dmg08_out1_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_disable_3_dmg08_cgb04c_out3,
    "gambatte/lycEnable/ff45_disable_3_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_disable_ds_1_cgb04c_out1,
    "gambatte/lycEnable/ff45_disable_ds_1_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_ff45_disable_ds_2_cgb04c_out3,
    "gambatte/lycEnable/ff45_disable_ds_2_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_1_dmg08_cgb04c_out3,
    "gambatte/lycEnable/ff45_enable_weirdpoint_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_2_dmg08_out3_cgb04c_out1,
    "gambatte/lycEnable/ff45_enable_weirdpoint_2_dmg08_out3_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_3_dmg08_out1_cgb04c_out3,
    "gambatte/lycEnable/ff45_enable_weirdpoint_3_dmg08_out1_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_4_dmg08_cgb04c_out3,
    "gambatte/lycEnable/ff45_enable_weirdpoint_4_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_ds_1_cgb04c_out3,
    "gambatte/lycEnable/ff45_enable_weirdpoint_ds_1_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_ds_2_cgb04c_out1,
    "gambatte/lycEnable/ff45_enable_weirdpoint_ds_2_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_ds_3_cgb04c_out1,
    "gambatte/lycEnable/ff45_enable_weirdpoint_ds_3_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_ds_4_cgb04c_out3,
    "gambatte/lycEnable/ff45_enable_weirdpoint_ds_4_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_ds_lcdoffset1_1_cgb04c_out2,
    "gambatte/lycEnable/ff45_enable_weirdpoint_ds_lcdoffset1_1_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_ds_lcdoffset1_2_cgb04c_out0,
    "gambatte/lycEnable/ff45_enable_weirdpoint_ds_lcdoffset1_2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_ds_lcdoffset1_3_cgb04c_out0,
    "gambatte/lycEnable/ff45_enable_weirdpoint_ds_lcdoffset1_3_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_ds_lcdoffset1_4_cgb04c_out2,
    "gambatte/lycEnable/ff45_enable_weirdpoint_ds_lcdoffset1_4_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_lcdoffset1_1_cgb04c_out2,
    "gambatte/lycEnable/ff45_enable_weirdpoint_lcdoffset1_1_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_lcdoffset1_2_cgb04c_out0,
    "gambatte/lycEnable/ff45_enable_weirdpoint_lcdoffset1_2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_ff45_enable_weirdpoint_lcdoffset1_3_cgb04c_out2,
    "gambatte/lycEnable/ff45_enable_weirdpoint_lcdoffset1_3_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff45_reenable_1_dmg08_cgb04c_out3,
    "gambatte/lycEnable/ff45_reenable_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_ff45_reenable_2_dmg08_cgb04c_out2,
    "gambatte/lycEnable/ff45_reenable_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_ff45_reenable_3_dmg08_cgb04c_out1,
    "gambatte/lycEnable/ff45_reenable_3_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_1_dmg08_cgb04c_out2,
    "gambatte/lycEnable/late_ff41_enable_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_2_dmg08_out2_cgb04c_out0,
    "gambatte/lycEnable/late_ff41_enable_2_dmg08_out2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_3_dmg08_cgb04c_out0,
    "gambatte/lycEnable/late_ff41_enable_3_dmg08_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_after_m2int_disable_dmg08_cgb04c_out2,
    "gambatte/lycEnable/late_ff41_enable_after_m2int_disable_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_after_m2int_dmg08_cgb04c_out2,
    "gambatte/lycEnable/late_ff41_enable_after_m2int_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_ds_1_cgb04c_out3,
    "gambatte/lycEnable/late_ff41_enable_ds_1_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_ds_2_cgb04c_out1,
    "gambatte/lycEnable/late_ff41_enable_ds_2_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_ds_lcdoffset1_1_cgb04c_out2,
    "gambatte/lycEnable/late_ff41_enable_ds_lcdoffset1_1_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_ds_lcdoffset1_2_cgb04c_out0,
    "gambatte/lycEnable/late_ff41_enable_ds_lcdoffset1_2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_lcdoffset1_1_cgb04c_out2,
    "gambatte/lycEnable/late_ff41_enable_lcdoffset1_1_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_ff41_enable_lcdoffset1_2_cgb04c_out0,
    "gambatte/lycEnable/late_ff41_enable_lcdoffset1_2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_1_dmg08_cgb04c_out3,
    "gambatte/lycEnable/late_ff45_enable_1_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_2_dmg08_out3_cgb04c_out1,
    "gambatte/lycEnable/late_ff45_enable_2_dmg08_out3_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_3_dmg08_cgb04c_out1,
    "gambatte/lycEnable/late_ff45_enable_3_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_after_m2int_dmg08_cgb04c_out2,
    "gambatte/lycEnable/late_ff45_enable_after_m2int_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_ds_1_cgb04c_out3,
    "gambatte/lycEnable/late_ff45_enable_ds_1_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_ds_2_cgb04c_out1,
    "gambatte/lycEnable/late_ff45_enable_ds_2_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_ds_lcdoffset1_1_cgb04c_out2,
    "gambatte/lycEnable/late_ff45_enable_ds_lcdoffset1_1_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_ds_lcdoffset1_2_cgb04c_out0,
    "gambatte/lycEnable/late_ff45_enable_ds_lcdoffset1_2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_lcdoffset1_1_cgb04c_out2,
    "gambatte/lycEnable/late_ff45_enable_lcdoffset1_1_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_late_ff45_enable_lcdoffset1_2_cgb04c_out0,
    "gambatte/lycEnable/late_ff45_enable_lcdoffset1_2_cgb04c_out0.gbc"
);

gambatte_test!(
    gambatte_lcdoff_lycirqen_1_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lcdoff_lycirqen_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lcdoff_lycirqen_2_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lcdoff_lycirqen_2_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lcdoff_lycirqen_3_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lcdoff_lycirqen_3_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lcdoff_lycirqen_4_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lcdoff_lycirqen_4_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff41_disable_1_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_ff41_disable_1_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff41_disable_2_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff41_disable_2_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff41_disable_ds_1_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_ff41_disable_ds_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff41_disable_ds_2_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff41_disable_ds_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_disable_1_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_ff45_disable_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_disable_2_dmg08_outE0_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff45_disable_2_dmg08_outE0_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_disable_3_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff45_disable_3_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_disable_ds_1_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_ff45_disable_ds_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_disable_ds_2_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff45_disable_ds_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_enable_weirdpoint_1_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff45_enable_weirdpoint_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_enable_weirdpoint_2_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff45_enable_weirdpoint_2_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_enable_weirdpoint_3_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff45_enable_weirdpoint_3_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_ff45_enable_weirdpoint_4_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_ff45_enable_weirdpoint_4_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_late_ff45_enable_1_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_late_ff45_enable_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_late_ff45_enable_2_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_late_ff45_enable_2_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_late_ff45_enable_3_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_late_ff45_enable_3_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_m1disable_1_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_m1disable_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_m1disable_2_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_m1disable_2_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_m1disable_3_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_m1disable_3_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc0_m1disable_ds_1_cgb04c_outE2,
    "gambatte/lycEnable/lyc0_m1disable_ds_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc0_m1disable_ds_2_cgb04c_outE0,
    "gambatte/lycEnable/lyc0_m1disable_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_enable_m1disable_1_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_enable_m1disable_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_enable_m1disable_2_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_enable_m1disable_2_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_enable_m1disable_3_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_enable_m1disable_3_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff41_enable_1_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff41_enable_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff41_enable_2_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff41_enable_2_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff41_enable_ds_1_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff41_enable_ds_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff41_enable_ds_2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff41_enable_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff41_enable_ds_lcdoffset1_1_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff41_enable_ds_lcdoffset1_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff41_enable_ds_lcdoffset1_2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff41_enable_ds_lcdoffset1_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff41_enable_lcdoffset1_1_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff41_enable_lcdoffset1_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff41_enable_lcdoffset1_2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff41_enable_lcdoffset1_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_1_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff45_enable_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_2_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff45_enable_2_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_3_dmg08_outE0_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff45_enable_3_dmg08_outE0_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_4_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff45_enable_4_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_5_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff45_enable_5_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_ds_1_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff45_enable_ds_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_ds_2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff45_enable_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_ds_3_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff45_enable_ds_3_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_ds_4_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff45_enable_ds_4_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_ds_5_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff45_enable_ds_5_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_ds_6_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff45_enable_ds_6_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_ds_lcdoffset1_1_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff45_enable_ds_lcdoffset1_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_ds_lcdoffset1_2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff45_enable_ds_lcdoffset1_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_lcdoffset1_1_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_ff45_enable_lcdoffset1_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_ff45_enable_lcdoffset1_2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_ff45_enable_lcdoffset1_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_m1disable_1_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_late_m1disable_1_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_m1disable_2_dmg08_outE2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_m1disable_2_dmg08_outE2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_late_m1disable_3_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_late_m1disable_3_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc153_m1disable_ds_1_cgb04c_outE2,
    "gambatte/lycEnable/lyc153_m1disable_ds_1_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lyc153_m1disable_ds_2_cgb04c_outE0,
    "gambatte/lycEnable/lyc153_m1disable_ds_2_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_1_dmg08_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_1_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_2_dmg08_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_2_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_3_dmg08_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_4_dmg08_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_4_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_5_dmg08_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_5_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_ds_1_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_ds_1_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_ds_2_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_ds_2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_ds_3_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_ds_3_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_ds_4_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_ds_4_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_ds_5_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_ds_5_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff41_enable_ds_6_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff41_enable_ds_6_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_disable2_1_dmg08_cgb04c_out1,
    "gambatte/lycEnable/lyc_ff45_disable2_1_dmg08_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_disable2_2_dmg08_out1_cgb04c_out3,
    "gambatte/lycEnable/lyc_ff45_disable2_2_dmg08_out1_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_disable2_3_dmg08_cgb04c_out3,
    "gambatte/lycEnable/lyc_ff45_disable2_3_dmg08_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_disable2_ds_1_cgb04c_out1,
    "gambatte/lycEnable/lyc_ff45_disable2_ds_1_cgb04c_out1.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_disable2_ds_2_cgb04c_out3,
    "gambatte/lycEnable/lyc_ff45_disable2_ds_2_cgb04c_out3.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_trigger_delay_3_dmg08_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff45_trigger_delay_3_dmg08_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lyc_ff45_trigger_delay_ds_2_cgb04c_out2,
    "gambatte/lycEnable/lyc_ff45_trigger_delay_ds_2_cgb04c_out2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_1_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_1_dmg08_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_2_dmg08_outE0_cgb04c_outE2,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_2_dmg08_outE0_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_3_dmg08_cgb04c_outE2,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_3_dmg08_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_ds_1_cgb04c_outE0,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_ds_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_ds_2_cgb04c_outE2,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_ds_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_ds_lcdoffset1_1_cgb04c_outE0,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_ds_lcdoffset1_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_ds_lcdoffset1_2_cgb04c_outE2,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_ds_lcdoffset1_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_lcdoffset1_1_cgb04c_outE0,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_lcdoffset1_1_cgb04c_outE0.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly00_stat50_lcdoffset1_2_cgb04c_outE2,
    "gambatte/lycEnable/lycwirq_trigger_ly00_stat50_lcdoffset1_2_cgb04c_outE2.gbc"
);

gambatte_test!(
    gambatte_lycwirq_trigger_ly94_stat50_dmg08_cgb04c_outE0,
    "gambatte/lycEnable/lycwirq_trigger_ly94_stat50_dmg08_cgb04c_outE0.gbc"
);

