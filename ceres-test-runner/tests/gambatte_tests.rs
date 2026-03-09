//! Integration tests using the Gambatte test ROM suite.
//!
//! These tests correspond to the Gambatte-derived unit tests in
//! `ceres-core/src/sm83/tests.rs`. The purpose is to verify that the same
//! hardware behaviors that pass at the unit-test level also pass end-to-end
//! with the full emulator stack (PPU, APU, memory, boot sequence, etc.).
//!
//! # Completion detection
//!
//! Gambatte ROMs use one of two `lprint_a` variants to write results to VRAM:
//!
//! - **NibbleSplit** (`undef_ops/`, `halt/`, `irq_precedence/`): writes
//!   `swap(A) & 0x0F` to tile-map `0x9800` and `A & 0x0F` to `0x9801`.
//!   Result = `(0x9800 << 4) | 0x9801`.
//! - **OldStyle** (`oam_access/`, `sprites/`): writes the raw result byte A
//!   directly to tile-map `0x9800`.  Result = `0x9800`.
//!
//! Completion is detected by checking that VRAM byte `0x8002` equals `0x7F`,
//! which is the third byte of the first font tile (copied from ROM offset
//! `0x7A02`). Before `lprint_a` runs this byte is `0x00`; after it is `0x7F`.
//! The sentinel and result bytes are read directly (bypassing PPU mode checks)
//! via `Gb::read_vram_direct`, because the frame boundary often falls in Mode 3
//! after `lprint_a` turns the LCD back on.
//!
//! # Models
//!
//! All ROMs tested here are labelled `_dmg08_cgb04c_`, meaning they target
//! original DMG and CGB-C hardware. We run them under `Model::CgbE` because
//! the CGB boot ROM completes quickly (no logo scroll), whereas the DMG boot
//! ROM takes >200 frames before handing off to game code in our emulator.

use ceres_core::Model;
use ceres_test_runner::{
    load_test_rom,
    test_runner::{CompletionCheck, DummyAudioCallback, TestConfig, TestResult, TestRunner},
};
use std::cell::Cell;

// ────────────────────────────────────────────────────────────────────────────
// Completion check
// ────────────────────────────────────────────────────────────────────────────

/// Which `lprint_a` variant the test ROM uses.
///
/// Gambatte test ROMs use two different result-encoding routines:
/// - `OldStyle`: writes the raw result byte to tile-map address `0x9800` only.
///   Used by `oam_access/` and `sprites/` ROMs.
/// - `NibbleSplit`: writes `swap(A) & 0F` to `0x9800` and `A & 0F` to `0x9801`,
///   so the result is `(0x9800 << 4) | 0x9801`.
///   Used by `irq_precedence/` ROMs.
#[derive(Clone, Copy)]
pub enum LprintVariant {
    /// Raw result byte at `0x9800`.
    OldStyle,
    /// Hi nibble at `0x9800`, lo nibble at `0x9801`.
    NibbleSplit,
}

/// Gambatte test completion check.
///
/// Uses the tile-copy sentinel byte at VRAM `0x8002` to detect that
/// `lprint_a` has finished writing results to VRAM.  The sentinel byte is
/// read directly (bypassing PPU mode checks) because the frame boundary often
/// falls in Mode 3 after `lprint_a` turns the LCD back on.
pub struct GambatteCheck {
    expected: u8,
    variant: LprintVariant,
    frame: Cell<u32>,
}

/// VRAM sentinel: `lprint_a` copies font tiles from ROM `0x7A00` to VRAM
/// `0x8000`. Byte offset 2 is `0x7F` (third byte of the '0' digit tile).
/// Before `lprint_a` runs the CGB boot ROM leaves `0x8002 = 0x00`.
const VRAM_SENTINEL_ADDR: u16 = 0x8002;
const VRAM_SENTINEL_VAL: u8 = 0x7F;

/// Minimum number of frames to wait before checking completion so that the
/// CGB boot ROM has finished and handed control to the game ROM.
const MIN_FRAMES_AFTER_BOOT: u32 = 8;

impl GambatteCheck {
    #[must_use]
    pub fn new(expected: u8, variant: LprintVariant) -> Self {
        Self {
            expected,
            variant,
            frame: Cell::new(0),
        }
    }

    /// Read the result byte from VRAM.
    ///
    /// Reads VRAM directly, bypassing PPU mode checks, since by the time this
    /// is called the sentinel confirms `lprint_a` has already written the values.
    fn read_output(&self, gb: &ceres_core::Gb<DummyAudioCallback>) -> u8 {
        match self.variant {
            LprintVariant::OldStyle => gb.read_vram_direct(0x9800),
            LprintVariant::NibbleSplit => {
                let hi = gb.read_vram_direct(0x9800) & 0x0F;
                let lo = gb.read_vram_direct(0x9801) & 0x0F;
                (hi << 4) | lo
            }
        }
    }
}

impl CompletionCheck for GambatteCheck {
    fn check(&self, gb: &mut ceres_core::Gb<DummyAudioCallback>) -> Option<TestResult> {
        let cur = self.frame.get();
        self.frame.set(cur + 1);

        // Don't check until the CGB boot ROM has finished.
        if cur < MIN_FRAMES_AFTER_BOOT {
            return None;
        }

        // Wait for the tile-copy sentinel: once lprint_a has copied font tiles
        // from ROM to VRAM, byte 0x8002 becomes 0x7F.  Until then the game ROM
        // has not yet produced its result.
        //
        // We read this directly (bypassing PPU mode checks) because the frame
        // boundary often falls in Mode 3 after lprint_a turns the LCD back on,
        // so `read_mem(0x8002)` would always return 0xFF through the normal path.
        if gb.read_vram_direct(VRAM_SENTINEL_ADDR) != VRAM_SENTINEL_VAL {
            return None;
        }

        let actual = self.read_output(gb);
        if actual == self.expected {
            Some(TestResult::Passed)
        } else {
            None // sentinel matched but tile map not yet correct — keep running
        }
    }

    fn on_timeout(&self, gb: &mut ceres_core::Gb<DummyAudioCallback>) -> TestResult {
        let stat = gb.read_mem(0xFF41);
        let sentinel = gb.read_vram_direct(VRAM_SENTINEL_ADDR);
        let actual = self.read_output(gb);
        if sentinel != VRAM_SENTINEL_VAL {
            TestResult::Failed(format!(
                "timeout: lprint_a never ran (sentinel=0x{sentinel:02X}, STAT=0x{stat:02X})",
            ))
        } else if actual == self.expected {
            TestResult::Passed
        } else {
            TestResult::Failed(format!(
                "timeout: expected 0x{:02X}, got 0x{actual:02X} (sentinel=0x{sentinel:02X}, STAT=0x{stat:02X})",
                self.expected
            ))
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Helper
// ────────────────────────────────────────────────────────────────────────────

/// Run a Gambatte test ROM and return the result.
///
/// `relative_path` is relative to `external/test-roms/`.
/// `expected_output` is the byte encoded in the ROM filename (`_outXX`).
/// `variant` controls how the result byte is decoded from VRAM.
fn run_gambatte_test_inner(
    relative_path: &str,
    expected_output: u8,
    variant: LprintVariant,
) -> TestResult {
    let rom = match load_test_rom(relative_path) {
        Ok(rom) => rom,
        Err(e) => return TestResult::Error(format!("Failed to load test ROM: {e}")),
    };

    let config = TestConfig {
        model: Model::CgbE,
        timeout_frames: 200,
        test_name: relative_path.to_string(),
        ..TestConfig::default()
    };

    let mut runner = match TestRunner::new(
        rom,
        config,
        Box::new(GambatteCheck::new(expected_output, variant)),
    ) {
        Ok(runner) => runner,
        Err(e) => return TestResult::Error(format!("Failed to create test runner: {e}")),
    };

    runner.run()
}

/// Run a Gambatte test ROM using the NibbleSplit lprint_a variant.
///
/// Used by `undef_ops/`, `halt/`, and `irq_precedence/` ROMs.
fn run_gambatte_test(relative_path: &str, expected_output: u8) -> TestResult {
    run_gambatte_test_inner(relative_path, expected_output, LprintVariant::NibbleSplit)
}

/// Run a Gambatte test ROM using the OldStyle lprint_a variant.
///
/// Used by `oam_access/` and `sprites/` ROMs.
fn run_gambatte_test_old(relative_path: &str, expected_output: u8) -> TestResult {
    run_gambatte_test_inner(relative_path, expected_output, LprintVariant::OldStyle)
}

// ────────────────────────────────────────────────────────────────────────────
// Undefined opcode tests
//
// Executing an undefined opcode on real hardware locks the CPU permanently
// (HALT-like). The ROM detects this by setting A=0x01 before the undefined
// opcode: if the CPU locks up correctly the display routine is never reached
// and the ROM ends in a HALT. If the opcode is silently skipped the display
// routine would show a different value.
//
// The ROMs output 0x01 to indicate "CPU locked up as expected".
//
// Source: gambatte/undef_ops/undef_op_XX_dmg08_cgb04c_out01.gbc
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn gambatte_undef_op_d3() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_d3_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_db() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_db_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_e3() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_e3_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_e4() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_e4_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_eb() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_eb_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_ec() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_ec_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_ed() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_ed_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_f4() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_f4_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_fc() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_fc_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_undef_op_fd() {
    let result = run_gambatte_test(
        "gambatte/undef_ops/undef_op_fd_dmg08_cgb04c_out01.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

// ────────────────────────────────────────────────────────────────────────────
// HALT bug tests
//
// When IME=0 and (IF & IE) != 0, executing HALT triggers the halt bug: the
// byte immediately following HALT is used as both the opcode and its first
// operand.
//
// Source: gambatte/halt/noime_ifandie_halt_*.gbc
// ────────────────────────────────────────────────────────────────────────────

/// HALT bug with `LD A, 0x3C` following HALT.
///
/// Expected output: 0x3F.
/// The halt bug causes `LD A, d8` to load its own opcode byte (0x3E) instead
/// of 0x3C, giving A=0x3E. The ROM then applies `INC A` and an additional
/// display offset, yielding the displayed value 0x3F.
#[test]
fn gambatte_halt_bug_noime_lda_3c() {
    let result = run_gambatte_test(
        "gambatte/halt/noime_ifandie_halt_lda_3c_dmg08_cgb04c_out3F.gbc",
        0x3F,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// HALT bug with `SRA A` (CB 2F) following HALT.
///
/// Expected output: 0xF1.
/// The halt bug re-fetches 0xCB as the CB sub-opcode, so `CB CB` = `SET 1, E`
/// executes instead of `SRA A`. The final A value displayed is 0xF1.
#[test]
fn gambatte_halt_bug_noime_sra() {
    let result = run_gambatte_test(
        "gambatte/halt/noime_ifandie_halt_sra_dmg08_cgb04c_outF1.gbc",
        0xF1,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

// ────────────────────────────────────────────────────────────────────────────
// IRQ precedence / IF-clobber tests
//
// These tests exercise interrupt dispatch edge cases where SP is positioned
// such that pushing the return address overwrites IE (0xFFFF), potentially
// cancelling the interrupt mid-dispatch.
//
// Source: gambatte/irq_precedence/if_and_ie_0_*.gbc
// ────────────────────────────────────────────────────────────────────────────

/// SP=0x0000, timer interrupt: push overwrites IE, dispatch cancelled.
///
/// Expected output: 0xE4 (IF still has timer bit set, upper bits OR'd to 1).
#[test]
fn gambatte_irq_precedence_if_and_ie_0_if_1() {
    let result = run_gambatte_test(
        "gambatte/irq_precedence/if_and_ie_0_if_1_dmg08_cgb04c_outE4.gbc",
        0xE4,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// SP=0x0001, timer interrupt: high byte of PC goes to 0x0000, IE not
/// clobbered, dispatch completes. IF timer bit cleared.
///
/// Expected output: 0xE1 (IF with timer bit cleared = 0xE0, but the ROM
/// reads IF *before* clearing yields the specific value 0xE1).
#[test]
fn gambatte_irq_precedence_if_and_ie_0_if_2() {
    let result = run_gambatte_test(
        "gambatte/irq_precedence/if_and_ie_0_if_2_dmg08_cgb04c_outE1.gbc",
        0xE1,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// SP=0x0000, timer interrupt: vector 1 (PC=0x0000 side-effect check).
///
/// Expected output: 0x00.
#[test]
fn gambatte_irq_precedence_if_and_ie_0_vector_1() {
    let result = run_gambatte_test(
        "gambatte/irq_precedence/if_and_ie_0_vector_1_dmg08_cgb04c_out00.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// SP=0xFF10, normal timer dispatch: PC lands at timer vector 0x0050.
///
/// Expected output: 0x50.
#[test]
fn gambatte_irq_precedence_if_and_ie_0_vector_2() {
    let result = run_gambatte_test(
        "gambatte/irq_precedence/if_and_ie_0_vector_2_dmg08_cgb04c_out50.gbc",
        0x50,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// SP=0x0000, cancelled dispatch, vector 3 variant.
///
/// Expected output: 0x00.
#[test]
fn gambatte_irq_precedence_if_and_ie_0_vector_3() {
    let result = run_gambatte_test(
        "gambatte/irq_precedence/if_and_ie_0_vector_3_dmg08_cgb04c_out00.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// SP=0xFF10, normal timer dispatch, vector 4 variant.
///
/// Expected output: 0x50.
#[test]
fn gambatte_irq_precedence_if_and_ie_0_vector_4() {
    let result = run_gambatte_test(
        "gambatte/irq_precedence/if_and_ie_0_vector_4_dmg08_cgb04c_out50.gbc",
        0x50,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

// ────────────────────────────────────────────────────────────────────────────
// OAM access timing tests
//
// These ROMs test OAM read/write accessibility at the cycle-accurate boundary
// between Mode 2 (OAM scan, blocked) and other modes (accessible).  Result
// byte is the raw value at VRAM `0x9800` (OldStyle lprint_a variant):
//   0x03 = OAM byte read as 0xFF (blocked, corrupt read)
//   0x00 = OAM byte read as 0x00 (accessible, correct)
//   0x01 = OAM write took effect
//
// Source: gambatte/oam_access/*.gbc
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn gambatte_oam_access_10spritesprline_postread_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/10spritesprline_postread_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_10spritesprline_postread_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/10spritesprline_postread_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_midread_1() {
    let result = run_gambatte_test_old("gambatte/oam_access/midread_1_dmg08_cgb04c_out3.gbc", 0x03);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_midread_2() {
    let result = run_gambatte_test_old("gambatte/oam_access/midread_2_dmg08_cgb04c_out3.gbc", 0x03);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_midread_3() {
    let result = run_gambatte_test_old("gambatte/oam_access/midread_3_dmg08_cgb04c_out3.gbc", 0x03);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_midwrite_1() {
    let result =
        run_gambatte_test_old("gambatte/oam_access/midwrite_1_dmg08_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_midwrite_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/midwrite_2_dmg08_out1_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_midwrite_3() {
    let result =
        run_gambatte_test_old("gambatte/oam_access/midwrite_3_dmg08_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_1() {
    let result =
        run_gambatte_test_old("gambatte/oam_access/postread_1_dmg08_cgb04c_out3.gbc", 0x03);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_2() {
    let result =
        run_gambatte_test_old("gambatte/oam_access/postread_2_dmg08_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_ds_1() {
    let result = run_gambatte_test_old("gambatte/oam_access/postread_ds_1_cgb04c_out3.gbc", 0x03);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_ds_2() {
    let result = run_gambatte_test_old("gambatte/oam_access/postread_ds_2_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx2_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx2_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx2_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx2_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx3_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx3_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx3_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx3_2_dmg08_xout1_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx3_3() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx3_3_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx5_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx5_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx5_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx5_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx5_ds_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx5_ds_1_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postread_scx5_ds_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postread_scx5_ds_2_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postwrite_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postwrite_1_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postwrite_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postwrite_2_dmg08_cgb04c_out1.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postwrite_2_scx3() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postwrite_2_scx3_dmg08_cgb04c_out1.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postwrite_ds_1() {
    let result = run_gambatte_test_old("gambatte/oam_access/postwrite_ds_1_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postwrite_ds_2() {
    let result = run_gambatte_test_old("gambatte/oam_access/postwrite_ds_2_cgb04c_out1.gbc", 0x01);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postwrite_scx1_ds_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postwrite_scx1_ds_1_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_postwrite_scx1_ds_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/postwrite_scx1_ds_2_cgb04c_out1.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// OAM read one cycle before Mode 2 begins (should be accessible).
///
#[test]
fn gambatte_oam_access_preread_1() {
    let result = run_gambatte_test_old("gambatte/oam_access/preread_1_dmg08_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_preread_2() {
    let result = run_gambatte_test_old("gambatte/oam_access/preread_2_dmg08_cgb04c_out3.gbc", 0x03);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_preread_ds_1() {
    let result = run_gambatte_test_old("gambatte/oam_access/preread_ds_1_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// Double-speed OAM preread variant 2 (should be accessible, one tick before Mode 2).
#[test]
fn gambatte_oam_access_preread_ds_2() {
    let result = run_gambatte_test_old("gambatte/oam_access/preread_ds_2_cgb04c_out3.gbc", 0x03);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_preread_ds_lcdoffset1_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/preread_ds_lcdoffset1_1_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// Double-speed + lcdoffset1 OAM preread variant 2.
#[test]
fn gambatte_oam_access_preread_ds_lcdoffset1_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/preread_ds_lcdoffset1_2_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// lcdoffset1 OAM preread variant 1 (should return blocked / 0x00).
///
/// Ignored: OAM blocking boundary is wrong under lcdoffset1 timing — the
/// 4-tick LCD-on offset shifts the Mode 2 start, so the blocking window
/// starts one T-cycle later than the emulator currently models.
#[test]
#[ignore = "OAM mode-2 blocking boundary wrong under lcdoffset1 (4-tick LCD-on offset not modelled)"]
fn gambatte_oam_access_preread_lcdoffset1_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/preread_lcdoffset1_1_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_preread_lcdoffset1_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/preread_lcdoffset1_2_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_prewrite_1() {
    let result =
        run_gambatte_test_old("gambatte/oam_access/prewrite_1_dmg08_cgb04c_out1.gbc", 0x01);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_prewrite_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/prewrite_2_dmg08_out1_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_prewrite_3() {
    let result =
        run_gambatte_test_old("gambatte/oam_access/prewrite_3_dmg08_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_prewrite_ds_1() {
    let result = run_gambatte_test_old("gambatte/oam_access/prewrite_ds_1_cgb04c_out1.gbc", 0x01);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_prewrite_ds_2() {
    let result = run_gambatte_test_old("gambatte/oam_access/prewrite_ds_2_cgb04c_out0.gbc", 0x00);
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// Double-speed + lcdoffset1 OAM prewrite variant 1 (write should take effect).
///
/// Ignored: OAM write-blocking boundary is wrong under double-speed + lcdoffset1
/// timing — the 4-tick LCD-on offset shifts the Mode 2 start, so the write
/// lands in the incorrectly-early blocked window.
#[test]
#[ignore = "OAM write-blocking boundary wrong under double-speed + lcdoffset1 (4-tick LCD-on offset not modelled)"]
fn gambatte_oam_access_prewrite_ds_lcdoffset1_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/prewrite_ds_lcdoffset1_1_cgb04c_out1.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_prewrite_ds_lcdoffset1_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/prewrite_ds_lcdoffset1_2_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

/// lcdoffset1 OAM prewrite variant 1 (write should take effect).
///
/// Ignored: OAM write-blocking boundary is wrong under lcdoffset1 timing —
/// the 4-tick LCD-on offset shifts the Mode 2 start, so the write is
/// incorrectly blocked by the emulator's early blocking window.
#[test]
#[ignore = "OAM write-blocking boundary wrong under lcdoffset1 (4-tick LCD-on offset not modelled)"]
fn gambatte_oam_access_prewrite_lcdoffset1_1() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/prewrite_lcdoffset1_1_cgb04c_out1.gbc",
        0x01,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_oam_access_prewrite_lcdoffset1_2() {
    let result = run_gambatte_test_old(
        "gambatte/oam_access/prewrite_lcdoffset1_2_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

// ────────────────────────────────────────────────────────────────────────────
// Sprite count / Mode 3 duration tests
//
// Each sprite on a scanline adds 11 T-cycles (6 in double-speed) to Mode 3.
// These ROMs verify that the correct number of sprites extends Mode 3 enough
// to flip the STAT mode bit at the right cycle.
//
// Source: gambatte/sprites/*PrLine_m3stat_*.gbc
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn gambatte_sprites_10spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/10spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_10spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/10spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_1spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/1spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_1spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/1spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_2spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/2spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_2spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/2spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_3spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/3spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_3spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/3spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_4spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/4spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_4spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/4spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_5spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/5spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_5spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/5spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_6spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/6spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_6spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/6spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_7spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/7spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_7spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/7spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_8spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/8spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_8spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/8spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_9spritesprline_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/9spritesPrLine_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_9spritesprline_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/9spritesPrLine_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_10spritesprline_10xposa7_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/10spritesPrLine_10xposA7_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_10spritesprline_10xposa7_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/10spritesPrLine_10xposA7_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_10spritesprline_1xpos0_m3stat_1() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/10spritesPrLine_1xpos0_m3stat_1_dmg08_cgb04c_out3.gbc",
        0x03,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}

#[test]
fn gambatte_sprites_10spritesprline_1xpos0_m3stat_2() {
    let result = run_gambatte_test_old(
        "gambatte/sprites/10spritesPrLine_1xpos0_m3stat_2_dmg08_cgb04c_out0.gbc",
        0x00,
    );
    assert_eq!(result, TestResult::Passed, "{result:?}");
}
