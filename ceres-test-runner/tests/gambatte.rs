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

        let mut gb = match GbBuilder::new(48000, DummyAudioCallback::default())
            .with_model(Model::CgbE)
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
