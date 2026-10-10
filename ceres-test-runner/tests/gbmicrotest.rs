//! Integration tests using Aappleby's GBMicrotest ROMs.
//!
//! Source: <https://github.com/aappleby/GBMicrotest>. The ROMs were checked
//! on a real DMG-CPU-08 and report through HRAM: `0xFF80` holds the actual
//! result, `0xFF81` the expected one and `0xFF82` is `0x01` on success or
//! `0xFF` on failure.
//!
//! The ROMs that fail are listed in `gbmicrotest_known_failures.txt`: SameBoy,
//! the model for the PPU and CPU timing, fails them in exactly the same way.
//! The test fails on any other failure, and on a known failure that now
//! passes. Run with `BLESS=1` to rewrite the list.

use ceres_core::Model;
use ceres_test_runner::{
    Run, check_known_failures, checks::MicrotestCheck, collect_roms, test_roms_dir, timeouts,
};

/// ROMs that never report a result: test benches, visual or manual ROMs and
/// tests that only make sense with a screenshot.
const NOT_SELF_CHECKING: &[&str] = &[
    "000-oam_lock.gb",
    "000-write_to_x8000.gb",
    "001-vram_unlocked.gb",
    "002-vram_locked.gb",
    "004-tima_boot_phase.gb",
    "004-tima_cycle_timer.gb",
    "007-lcd_on_stat.gb",
    "400-dma.gb",
    "500-scx-timing.gb",
    "800-ppu-latch-scx.gb",
    "801-ppu-latch-scy.gb",
    "802-ppu-latch-tileselect.gb",
    "803-ppu-latch-bgdisplay.gb",
    "audio_testbench.gb",
    "cpu_bus_1.gb",
    "dma_basic.gb",
    "flood_vram.gb",
    "lcdon_write_timing.gb",
    "ly_while_lcd_off.gb",
    "minimal.gb",
    "mode2_stat_int_to_oam_unlock.gb",
    "oam_sprite_trashing.gb",
    "poweron.gb",
    "ppu_scx_vs_bgp.gb",
    "ppu_sprite_testbench.gb",
    "ppu_spritex_vs_scx.gb",
    "ppu_win_vs_wx.gb",
    "ppu_wx_early.gb",
    "temp.gb",
    "toggle_lcdc.gb",
    "wave_write_to_0xC003.gb",
];

#[test]
fn gbmicrotest_suite() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for path in collect_roms(&test_roms_dir().join("gbmicrotest")) {
        let name = path.file_name().and_then(|n| n.to_str()).expect("name");
        if NOT_SELF_CHECKING.contains(&name) {
            continue;
        }
        checked += 1;
        let result = Run::new(&path, Model::DmgB)
            .skip_boot_rom()
            .timeout(timeouts::GBMICROTEST)
            .check(MicrotestCheck);
        if !result.is_passed() {
            failures.push(name.to_string());
        }
    }
    assert!(checked > 400, "only {checked} gbmicrotest ROMs were run");
    check_known_failures("gbmicrotest_known_failures.txt", failures);
}
