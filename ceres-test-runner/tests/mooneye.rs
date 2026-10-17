//! Integration tests for the Mooneye and Wilbertpol test suites.
//!
//! - Mooneye tests live under `external/test-roms/mooneye-test-suite/`.
//! - Wilbertpol tests are an extended fork under
//!   `external/test-roms/mooneye-test-suite-wilbertpol/`.
//!
//! Tests that exist in both suites run from the Wilbertpol ROMs to exercise
//! the most up-to-date versions of those tests. The ROMs report in the
//! registers (see `RegisterCheck`) and run from the state the boot ROM
//! leaves; the boot tests also run from the real boot ROM.
//!
//! Each ROM runs on a model its name says it was verified on: `-dmgABC`,
//! `-dmg0`, `-mgb`, `-sgb`... on the matching DMG revision, `-C` and `-cgb*`
//! on a CGB-E (or the revision named), `-GS` on a DMG-B.

use ceres_core::Model;
use ceres_test_runner::{
    Run,
    checks::{RegisterCheck, TestResult},
    rom_test, run_exact_screenshot, run_ranked_screenshot, timeouts,
};

/// Root paths to the two test suites.
const MOONEYE: &str = "mooneye-test-suite";
const WILBERTPOL: &str = "mooneye-test-suite-wilbertpol";

fn run(suite: &str, rom: &str, model: Model, boot_rom: bool) -> TestResult {
    let run = Run::new(format!("{suite}/{rom}"), model).timeout(timeouts::MOONEYE);
    let run = if boot_rom { run } else { run.skip_boot_rom() };
    run.check(RegisterCheck)
}

/// A ROM run from the state the boot ROM leaves.
fn mooneye(suite: &str, rom: &str, model: Model) -> TestResult {
    run(suite, rom, model, false)
}

/// A boot-state test runs twice: from the state the boot ROM leaves and from
/// the real boot ROM, which must leave the same machine behind.
fn boot(suite: &str, rom: &str, model: Model) -> TestResult {
    let post_boot = run(suite, rom, model, false);
    if !post_boot.is_passed() {
        return post_boot;
    }
    run(suite, rom, model, true)
}

/// A ROM whose screen must be the reference screenshot (`manual-only/`).
fn screenshot(suite: &str, rom: &str, model: Model) -> TestResult {
    run_exact_screenshot(&format!("{suite}/{rom}"), model, timeouts::MOONEYE)
}

// =============================================================================
// Root level acceptance tests
// =============================================================================

rom_test!(test_add_sp_e_timing: mooneye(WILBERTPOL, "acceptance/add_sp_e_timing.gb", Model::CgbE));
rom_test!(test_call_cc_timing: mooneye(WILBERTPOL, "acceptance/call_cc_timing.gb", Model::CgbE));
rom_test!(test_call_cc_timing2: mooneye(WILBERTPOL, "acceptance/call_cc_timing2.gb", Model::CgbE));
rom_test!(test_call_timing: mooneye(WILBERTPOL, "acceptance/call_timing.gb", Model::CgbE));
rom_test!(test_call_timing2: mooneye(WILBERTPOL, "acceptance/call_timing2.gb", Model::CgbE));
rom_test!(test_di_timing_gs: mooneye(WILBERTPOL, "acceptance/di_timing-GS.gb", Model::DmgB));
rom_test!(test_div_timing: mooneye(WILBERTPOL, "acceptance/div_timing.gb", Model::CgbE));
rom_test!(test_ei_sequence: mooneye(MOONEYE, "acceptance/ei_sequence.gb", Model::CgbE));
rom_test!(test_ei_timing: mooneye(WILBERTPOL, "acceptance/ei_timing.gb", Model::CgbE));
rom_test!(test_halt_ime0_ei: mooneye(WILBERTPOL, "acceptance/halt_ime0_ei.gb", Model::CgbE));
rom_test!(
    test_halt_ime0_nointr_timing: mooneye(WILBERTPOL, "acceptance/halt_ime0_nointr_timing.gb", Model::CgbE)
);
rom_test!(
    test_halt_ime1_timing: mooneye(WILBERTPOL, "acceptance/halt_ime1_timing.gb", Model::CgbE)
);
rom_test!(
    test_halt_ime1_timing2_gs: mooneye(WILBERTPOL, "acceptance/halt_ime1_timing2-GS.gb", Model::DmgB)
);
rom_test!(test_if_ie_registers: mooneye(WILBERTPOL, "acceptance/if_ie_registers.gb", Model::CgbE));
rom_test!(test_intr_timing: mooneye(WILBERTPOL, "acceptance/intr_timing.gb", Model::CgbE));
rom_test!(test_jp_cc_timing: mooneye(WILBERTPOL, "acceptance/jp_cc_timing.gb", Model::CgbE));
rom_test!(test_jp_timing: mooneye(WILBERTPOL, "acceptance/jp_timing.gb", Model::CgbE));
rom_test!(
    test_ld_hl_sp_e_timing: mooneye(WILBERTPOL, "acceptance/ld_hl_sp_e_timing.gb", Model::CgbE)
);
rom_test!(test_oam_dma_restart: mooneye(WILBERTPOL, "acceptance/oam_dma_restart.gb", Model::CgbE));
rom_test!(test_oam_dma_start: mooneye(WILBERTPOL, "acceptance/oam_dma_start.gb", Model::CgbE));
rom_test!(test_oam_dma_timing: mooneye(WILBERTPOL, "acceptance/oam_dma_timing.gb", Model::CgbE));
rom_test!(test_pop_timing: mooneye(WILBERTPOL, "acceptance/pop_timing.gb", Model::CgbE));
rom_test!(test_push_timing: mooneye(WILBERTPOL, "acceptance/push_timing.gb", Model::CgbE));
rom_test!(test_rapid_di_ei: mooneye(WILBERTPOL, "acceptance/rapid_di_ei.gb", Model::CgbE));
rom_test!(test_ret_cc_timing: mooneye(WILBERTPOL, "acceptance/ret_cc_timing.gb", Model::CgbE));
rom_test!(test_ret_timing: mooneye(WILBERTPOL, "acceptance/ret_timing.gb", Model::CgbE));
rom_test!(
    test_reti_intr_timing: mooneye(WILBERTPOL, "acceptance/reti_intr_timing.gb", Model::CgbE)
);
rom_test!(test_reti_timing: mooneye(WILBERTPOL, "acceptance/reti_timing.gb", Model::CgbE));
rom_test!(test_rst_timing: mooneye(WILBERTPOL, "acceptance/rst_timing.gb", Model::CgbE));

// =============================================================================
// Boot register tests (model-specific)
// =============================================================================

rom_test!(test_boot_div2_s: boot(MOONEYE, "acceptance/boot_div2-S.gb", Model::Sgb2));
rom_test!(test_boot_div_cgb0: boot(MOONEYE, "misc/boot_div-cgb0.gb", Model::Cgb0));
rom_test!(test_boot_div_cgbabcde: boot(MOONEYE, "misc/boot_div-cgbABCDE.gb", Model::CgbE));
rom_test!(test_boot_div_a: boot(MOONEYE, "misc/boot_div-A.gb", Model::Agb));
rom_test!(test_boot_div_dmg0: boot(MOONEYE, "acceptance/boot_div-dmg0.gb", Model::Dmg0));
rom_test!(test_boot_div_dmgabcmgb: boot(MOONEYE, "acceptance/boot_div-dmgABCmgb.gb", Model::DmgB));
rom_test!(test_boot_div_s: boot(MOONEYE, "acceptance/boot_div-S.gb", Model::Sgb));
rom_test!(test_boot_hwio_c: boot(WILBERTPOL, "misc/boot_hwio-C.gb", Model::CgbE));
rom_test!(test_boot_hwio_dmg0: boot(MOONEYE, "acceptance/boot_hwio-dmg0.gb", Model::Dmg0));
rom_test!(test_boot_hwio_g: boot(WILBERTPOL, "acceptance/boot_hwio-G.gb", Model::DmgB));
rom_test!(test_boot_hwio_s: boot(WILBERTPOL, "misc/boot_hwio-S.gb", Model::Sgb));
rom_test!(test_boot_regs_a: boot(WILBERTPOL, "misc/boot_regs-A.gb", Model::Agb));
rom_test!(test_boot_regs_cgb: boot(WILBERTPOL, "misc/boot_regs-cgb.gb", Model::Cgb0));
rom_test!(test_boot_regs_dmg: boot(WILBERTPOL, "acceptance/boot_regs-dmg.gb", Model::DmgB));
rom_test!(test_boot_regs_dmg0: boot(MOONEYE, "acceptance/boot_regs-dmg0.gb", Model::Dmg0));
rom_test!(test_boot_regs_mgb: boot(WILBERTPOL, "misc/boot_regs-mgb.gb", Model::Mgb));
rom_test!(test_boot_regs_sgb: boot(WILBERTPOL, "misc/boot_regs-sgb.gb", Model::Sgb));
rom_test!(test_boot_regs_sgb2: boot(WILBERTPOL, "misc/boot_regs-sgb2.gb", Model::Sgb2));

// =============================================================================
// bits/ tests
// =============================================================================

rom_test!(test_bits_mem_oam: mooneye(WILBERTPOL, "acceptance/bits/mem_oam.gb", Model::CgbE));
rom_test!(test_bits_reg_f: mooneye(WILBERTPOL, "acceptance/bits/reg_f.gb", Model::CgbE));
rom_test!(test_bits_unused_hwio_c: mooneye(WILBERTPOL, "misc/bits/unused_hwio-C.gb", Model::CgbC));
rom_test!(
    test_bits_unused_hwio_gs: mooneye(WILBERTPOL, "acceptance/bits/unused_hwio-GS.gb", Model::DmgB)
);

// =============================================================================
// instr/ tests
// =============================================================================

rom_test!(test_instr_daa: mooneye(MOONEYE, "acceptance/instr/daa.gb", Model::CgbE));

// =============================================================================
// interrupts/ tests
// =============================================================================

rom_test!(
    test_interrupts_ie_push: mooneye(MOONEYE, "acceptance/interrupts/ie_push.gb", Model::CgbE)
);

// =============================================================================
// oam_dma/ tests
// =============================================================================

rom_test!(test_oam_dma_basic: mooneye(MOONEYE, "acceptance/oam_dma/basic.gb", Model::CgbE));
rom_test!(test_oam_dma_reg_read: mooneye(MOONEYE, "acceptance/oam_dma/reg_read.gb", Model::CgbE));
rom_test!(
    test_oam_dma_sources_gs: mooneye(MOONEYE, "acceptance/oam_dma/sources-GS.gb", Model::DmgB)
);

// =============================================================================
// gpu/ tests (PPU)
// =============================================================================

rom_test!(
    test_gpu_hblank_ly_scx_timing_c: mooneye(WILBERTPOL, "acceptance/gpu/hblank_ly_scx_timing-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_hblank_ly_scx_timing_gs: mooneye(WILBERTPOL, "acceptance/gpu/hblank_ly_scx_timing-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_hblank_ly_scx_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/hblank_ly_scx_timing_nops.gb", Model::CgbE)
);
rom_test!(
    test_gpu_hblank_ly_scx_timing_variant_nops: mooneye(WILBERTPOL, "acceptance/gpu/hblank_ly_scx_timing_variant_nops.gb", Model::CgbE)
);
rom_test!(
    test_gpu_intr_0_timing: mooneye(WILBERTPOL, "acceptance/gpu/intr_0_timing.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_1_2_timing_gs: mooneye(WILBERTPOL, "acceptance/gpu/intr_1_2_timing-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_1_timing: mooneye(WILBERTPOL, "acceptance/gpu/intr_1_timing.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_0_timing: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_0_timing.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_scx1_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_scx1_timing_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_scx2_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_scx2_timing_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_scx3_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_scx3_timing_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_scx4_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_scx4_timing_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_scx5_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_scx5_timing_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_scx6_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_scx6_timing_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_scx7_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_scx7_timing_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_scx8_timing_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_scx8_timing_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_timing: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_timing.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_timing_sprites: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_timing_sprites.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_timing_sprites_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_timing_sprites_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_timing_sprites_scx1_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_timing_sprites_scx1_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_timing_sprites_scx2_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_timing_sprites_scx2_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_timing_sprites_scx3_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_timing_sprites_scx3_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode0_timing_sprites_scx4_nops: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode0_timing_sprites_scx4_nops.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_mode3_timing: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_mode3_timing.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_oam_ok_timing: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_oam_ok_timing.gb", Model::DmgB)
);
rom_test!(
    test_gpu_intr_2_timing: mooneye(WILBERTPOL, "acceptance/gpu/intr_2_timing.gb", Model::DmgB)
);
rom_test!(
    test_gpu_lcdon_mode_timing: mooneye(WILBERTPOL, "acceptance/gpu/lcdon_mode_timing.gb", Model::DmgB)
);
rom_test!(
    test_gpu_lcdon_timing_gs: mooneye(MOONEYE, "acceptance/ppu/lcdon_timing-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_lcdon_write_timing_gs: mooneye(MOONEYE, "acceptance/ppu/lcdon_write_timing-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly00_01_mode0_2: mooneye(WILBERTPOL, "acceptance/gpu/ly00_01_mode0_2.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly00_mode0_2_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly00_mode0_2-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly00_mode1_0_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly00_mode1_0-GS.gb", Model::DmgB)
);

// "-C" is any CGB: on the CGB-C (gambatte's enable_display/frame1_*
// tests) mode 1 ends a dot before this test expects.
rom_test!(
    test_gpu_ly00_mode1_2_c: mooneye(WILBERTPOL, "acceptance/gpu/ly00_mode1_2-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_ly00_mode2_3: mooneye(WILBERTPOL, "acceptance/gpu/ly00_mode2_3.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly00_mode3_0: mooneye(WILBERTPOL, "acceptance/gpu/ly00_mode3_0.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly143_144_145: mooneye(WILBERTPOL, "acceptance/gpu/ly143_144_145.gb", Model::Mgb)
);
rom_test!(
    test_gpu_ly143_144_152_153: mooneye(WILBERTPOL, "acceptance/gpu/ly143_144_152_153.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly143_144_mode0_1: mooneye(WILBERTPOL, "acceptance/gpu/ly143_144_mode0_1.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly143_144_mode3_0: mooneye(WILBERTPOL, "acceptance/gpu/ly143_144_mode3_0.gb", Model::DmgB)
);

// The `-C` ly_* ROMs were measured on a CGB-D/E class unit: on CGB-C and
// earlier LY reads 0 one M-cycle sooner on line 153.
rom_test!(test_gpu_ly_lyc_0_c: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_0-C.gb", Model::CgbE));
rom_test!(test_gpu_ly_lyc_0_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_0-GS.gb", Model::DmgB));
rom_test!(
    test_gpu_ly_lyc_0_write_c: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_0_write-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_ly_lyc_0_write_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_0_write-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly_lyc_144_c: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_144-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_ly_lyc_144_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_144-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly_lyc_153_c: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_153-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_ly_lyc_153_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_153-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_ly_lyc_153_write_c: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_153_write-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_ly_lyc_153_write_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_153_write-GS.gb", Model::DmgB)
);
rom_test!(test_gpu_ly_lyc_c: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc-C.gb", Model::CgbE));
rom_test!(test_gpu_ly_lyc_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc-GS.gb", Model::DmgB));
rom_test!(
    test_gpu_ly_lyc_write_c: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_write-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_ly_lyc_write_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly_lyc_write-GS.gb", Model::DmgB)
);

// See `test_gpu_ly_lyc_0_c` for the model choice.
rom_test!(
    test_gpu_ly_new_frame_c: mooneye(WILBERTPOL, "acceptance/gpu/ly_new_frame-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_ly_new_frame_gs: mooneye(WILBERTPOL, "acceptance/gpu/ly_new_frame-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_stat_irq_blocking: mooneye(WILBERTPOL, "acceptance/gpu/stat_irq_blocking.gb", Model::DmgB)
);
rom_test!(
    test_gpu_stat_lyc_onoff: mooneye(MOONEYE, "acceptance/ppu/stat_lyc_onoff.gb", Model::DmgB)
);
rom_test!(
    test_gpu_stat_write_if_c: mooneye(WILBERTPOL, "acceptance/gpu/stat_write_if-C.gb", Model::CgbC)
);
rom_test!(
    test_gpu_stat_write_if_gs: mooneye(WILBERTPOL, "acceptance/gpu/stat_write_if-GS.gb", Model::DmgB)
);
rom_test!(
    test_gpu_vblank_if_timing: mooneye(WILBERTPOL, "acceptance/gpu/vblank_if_timing.gb", Model::CgbE)
);
rom_test!(
    test_gpu_vblank_stat_intr_c: mooneye(WILBERTPOL, "misc/gpu/vblank_stat_intr-C.gb", Model::CgbE)
);
rom_test!(
    test_gpu_vblank_stat_intr_gs: mooneye(WILBERTPOL, "acceptance/gpu/vblank_stat_intr-GS.gb", Model::Mgb)
);

// =============================================================================
// serial/ tests
// =============================================================================

rom_test!(
    test_serial_boot_sclk_align_dmgabcmgb: mooneye(MOONEYE, "acceptance/serial/boot_sclk_align-dmgABCmgb.gb", Model::DmgB)
);

// =============================================================================
// timer/ tests
// =============================================================================

rom_test!(test_timer_div_write: mooneye(WILBERTPOL, "acceptance/timer/div_write.gb", Model::CgbE));
rom_test!(test_timer_if: mooneye(WILBERTPOL, "acceptance/timer/timer_if.gb", Model::CgbE));
rom_test!(
    test_timer_rapid_toggle: mooneye(WILBERTPOL, "acceptance/timer/rapid_toggle.gb", Model::CgbE)
);
rom_test!(test_timer_tim00: mooneye(WILBERTPOL, "acceptance/timer/tim00.gb", Model::CgbE));
rom_test!(
    test_timer_tim00_div_trigger: mooneye(WILBERTPOL, "acceptance/timer/tim00_div_trigger.gb", Model::CgbE)
);
rom_test!(test_timer_tim01: mooneye(WILBERTPOL, "acceptance/timer/tim01.gb", Model::CgbE));
rom_test!(
    test_timer_tim01_div_trigger: mooneye(WILBERTPOL, "acceptance/timer/tim01_div_trigger.gb", Model::CgbE)
);
rom_test!(test_timer_tim10: mooneye(WILBERTPOL, "acceptance/timer/tim10.gb", Model::CgbE));
rom_test!(
    test_timer_tim10_div_trigger: mooneye(WILBERTPOL, "acceptance/timer/tim10_div_trigger.gb", Model::CgbE)
);
rom_test!(test_timer_tim11: mooneye(WILBERTPOL, "acceptance/timer/tim11.gb", Model::CgbE));
rom_test!(
    test_timer_tim11_div_trigger: mooneye(WILBERTPOL, "acceptance/timer/tim11_div_trigger.gb", Model::CgbE)
);
rom_test!(
    test_timer_tima_reload: mooneye(WILBERTPOL, "acceptance/timer/tima_reload.gb", Model::CgbE)
);
rom_test!(
    test_timer_tima_write_reloading: mooneye(WILBERTPOL, "acceptance/timer/tima_write_reloading.gb", Model::CgbE)
);
rom_test!(
    test_timer_tma_write_reloading: mooneye(WILBERTPOL, "acceptance/timer/tma_write_reloading.gb", Model::CgbE)
);

// =============================================================================
// emulator-only/ tests
// =============================================================================
//
// MBC1 tests live under mooneye-test-suite/emulator-only/mbc1/, while
// Wilbertpol's only MBC test is mbc1_rom_4banks.gb at the suite root.

rom_test!(test_mbc1_bits_bank1: mooneye(MOONEYE, "emulator-only/mbc1/bits_bank1.gb", Model::CgbE));
rom_test!(test_mbc1_bits_bank2: mooneye(MOONEYE, "emulator-only/mbc1/bits_bank2.gb", Model::CgbE));
rom_test!(test_mbc1_bits_mode: mooneye(MOONEYE, "emulator-only/mbc1/bits_mode.gb", Model::CgbE));
rom_test!(test_mbc1_bits_ramg: mooneye(MOONEYE, "emulator-only/mbc1/bits_ramg.gb", Model::CgbE));
rom_test!(
    test_mbc1_multicart_rom_8mb: mooneye(MOONEYE, "emulator-only/mbc1/multicart_rom_8Mb.gb", Model::CgbE)
);
rom_test!(test_mbc1_ram_64kb: mooneye(MOONEYE, "emulator-only/mbc1/ram_64kb.gb", Model::CgbE));
rom_test!(test_mbc1_ram_256kb: mooneye(MOONEYE, "emulator-only/mbc1/ram_256kb.gb", Model::CgbE));
rom_test!(
    test_mbc1_rom_4banks: mooneye(WILBERTPOL, "emulator-only/mbc1_rom_4banks.gb", Model::CgbE)
);
rom_test!(test_mbc1_rom_512kb: mooneye(MOONEYE, "emulator-only/mbc1/rom_512kb.gb", Model::CgbE));
rom_test!(test_mbc1_rom_1mb: mooneye(MOONEYE, "emulator-only/mbc1/rom_1Mb.gb", Model::CgbE));
rom_test!(test_mbc1_rom_2mb: mooneye(MOONEYE, "emulator-only/mbc1/rom_2Mb.gb", Model::CgbE));
rom_test!(test_mbc1_rom_4mb: mooneye(MOONEYE, "emulator-only/mbc1/rom_4Mb.gb", Model::CgbE));
rom_test!(test_mbc1_rom_8mb: mooneye(MOONEYE, "emulator-only/mbc1/rom_8Mb.gb", Model::CgbE));
rom_test!(test_mbc1_rom_16mb: mooneye(MOONEYE, "emulator-only/mbc1/rom_16Mb.gb", Model::CgbE));
rom_test!(test_mbc2_bits_ramg: mooneye(MOONEYE, "emulator-only/mbc2/bits_ramg.gb", Model::CgbE));
rom_test!(test_mbc2_bits_romb: mooneye(MOONEYE, "emulator-only/mbc2/bits_romb.gb", Model::CgbE));
rom_test!(
    test_mbc2_bits_unused: mooneye(MOONEYE, "emulator-only/mbc2/bits_unused.gb", Model::CgbE)
);
rom_test!(test_mbc2_ram: mooneye(MOONEYE, "emulator-only/mbc2/ram.gb", Model::CgbE));
rom_test!(test_mbc2_rom_512kb: mooneye(MOONEYE, "emulator-only/mbc2/rom_512kb.gb", Model::CgbE));
rom_test!(test_mbc2_rom_1mb: mooneye(MOONEYE, "emulator-only/mbc2/rom_1Mb.gb", Model::CgbE));
rom_test!(test_mbc2_rom_2mb: mooneye(MOONEYE, "emulator-only/mbc2/rom_2Mb.gb", Model::CgbE));
rom_test!(test_mbc5_rom_512kb: mooneye(MOONEYE, "emulator-only/mbc5/rom_512kb.gb", Model::CgbE));
rom_test!(test_mbc5_rom_1mb: mooneye(MOONEYE, "emulator-only/mbc5/rom_1Mb.gb", Model::CgbE));
rom_test!(test_mbc5_rom_2mb: mooneye(MOONEYE, "emulator-only/mbc5/rom_2Mb.gb", Model::CgbE));
rom_test!(test_mbc5_rom_4mb: mooneye(MOONEYE, "emulator-only/mbc5/rom_4Mb.gb", Model::CgbE));
rom_test!(test_mbc5_rom_8mb: mooneye(MOONEYE, "emulator-only/mbc5/rom_8Mb.gb", Model::CgbE));
rom_test!(test_mbc5_rom_16mb: mooneye(MOONEYE, "emulator-only/mbc5/rom_16Mb.gb", Model::CgbE));
rom_test!(test_mbc5_rom_32mb: mooneye(MOONEYE, "emulator-only/mbc5/rom_32Mb.gb", Model::CgbE));
rom_test!(test_mbc5_rom_64mb: mooneye(MOONEYE, "emulator-only/mbc5/rom_64Mb.gb", Model::CgbE));

// =============================================================================
// manual-only/ tests
// =============================================================================

rom_test!(
    test_manual_sprite_priority_dmg: screenshot(WILBERTPOL, "manual-only/sprite_priority.gb", Model::DmgB)
);
rom_test!(
    test_manual_sprite_priority_cgb: screenshot(WILBERTPOL, "manual-only/sprite_priority.gb", Model::CgbE)
);

// =============================================================================
// madness/ tests
// =============================================================================

// An OAM DMA during `halt` leaves a stray sprite on the MGB; Ceres shows none.
rom_test!(
    #[ignore = "the MGB's OAM DMA during halt sprite glitch is not emulated"]
    test_madness_mgb_oam_dma_halt_sprites: run_ranked_screenshot(
        &format!("{WILBERTPOL}/madness/mgb_oam_dma_halt_sprites.gb"),
        &format!("{WILBERTPOL}/madness/mgb_oam_dma_halt_sprites_expected.png"),
        Model::Mgb,
        timeouts::MOONEYE
    )
);
