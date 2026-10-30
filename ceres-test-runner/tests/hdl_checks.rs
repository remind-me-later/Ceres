//! The check ROMs of `hdl/check` that verify what they read and report as
//! Mooneye's tests do (`hdl/check/report.inc`): the PPU's registers, VRAM,
//! OAM and the bus side read back. They run on the readable model and on a
//! Game Boy as well; built by `make -C hdl`.

use {
    ceres_core::Model,
    ceres_test_runner::{
        Run,
        checks::{RegisterCheck, TestResult},
        timeouts,
    },
    std::path::Path,
};

/// A check ROM from `hdl/build`, on a DMG-B, from the state the boot ROM
/// leaves.
fn run(rom: &str) -> TestResult {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../hdl/build")
        .join(rom);
    Run::new(path, Model::DmgB)
        .timeout(timeouts::MOONEYE)
        .skip_boot_rom()
        .check(RegisterCheck)
}

#[test]
#[ignore = "needs the check ROMs: make -C hdl"]
fn ppu_regs() {
    assert_eq!(run("ppu_regs.gb"), TestResult::Passed);
}

#[test]
#[ignore = "needs the check ROMs: make -C hdl"]
fn vram_oam() {
    assert_eq!(run("vram_oam.gb"), TestResult::Passed);
}

#[test]
#[ignore = "needs the check ROMs: make -C hdl"]
fn bus() {
    assert_eq!(run("bus.gb"), TestResult::Passed);
}
