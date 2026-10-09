//! Integration tests using Aappleby's GBMicrotest ROMs.
//!
//! Source: <https://github.com/aappleby/GBMicrotest>. The ROMs were checked
//! on a real DMG-CPU-08 and report through HRAM: `0xFF80` holds the actual
//! result, `0xFF81` the expected one and `0xFF82` is `0x01` on success or
//! `0xFF` on failure.

use ceres_core::{AudioCallback, GbBuilder, Model, Sample};
use ceres_test_runner::{load_test_rom, test_roms_dir};

/// Enough for every self-checking ROM (`is_if_set_during_ime0` needs ~23).
const MAX_FRAMES: u32 = 60;

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

/// Hardware-verified failures: SameBoy (the model for the PPU/CPU timing)
/// fails these in exactly the same way.
const KNOWN_FAILURES: &[&str] = &["halt_op_dupe_delay.gb", "stat_write_glitch_l154_d.gb"];

struct NoAudio;

impl AudioCallback for NoAudio {
    fn audio_sample(&self, _l: Sample, _r: Sample) {}
}

/// `Some(true)` if the ROM reported success, `Some(false)` on failure.
fn run_rom(name: &str) -> Option<bool> {
    let rom = load_test_rom(&format!("gbmicrotest/{name}")).expect("load ROM");
    let mut gb = GbBuilder::new(48000, NoAudio)
        .with_model(Model::DmgB)
        .with_run_bootrom(false)
        .with_rom(rom.into_boxed_slice())
        .expect("valid ROM")
        .build();

    for _ in 0..MAX_FRAMES {
        gb.run_frame();
        match gb.read_mem(0xFF82) {
            0x01 => return Some(true),
            0xFF => return Some(false),
            _ => (),
        }
    }
    None
}

#[test]
fn gbmicrotest_suite() {
    let dir = test_roms_dir().join("gbmicrotest");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("gbmicrotest directory")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| {
            std::path::Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gb"))
        })
        .collect();
    names.sort();

    let mut unexpected_failures = Vec::new();
    let mut unexpected_passes = Vec::new();
    let mut checked = 0;

    for name in &names {
        if NOT_SELF_CHECKING.contains(&name.as_str()) {
            continue;
        }
        checked += 1;
        let passed = run_rom(name) == Some(true);
        let known_failure = KNOWN_FAILURES.contains(&name.as_str());
        if !passed && !known_failure {
            unexpected_failures.push(name.clone());
        } else if passed && known_failure {
            unexpected_passes.push(name.clone());
        }
    }

    assert!(checked > 400, "only {checked} gbmicrotest ROMs were run");
    assert!(
        unexpected_failures.is_empty(),
        "gbmicrotest failures: {unexpected_failures:?}"
    );
    assert!(
        unexpected_passes.is_empty(),
        "gbmicrotest ROMs now pass, remove them from KNOWN_FAILURES: {unexpected_passes:?}"
    );
}
