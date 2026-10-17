use crate::{
    AudioCallback, CgbMode, GbBuilder, Model, Sample,
    cartridge::{HEADER_LOGO, NINTENDO_LOGO},
    memory::Wram,
};
use alloc::vec;

struct Silent;

impl AudioCallback for Silent {
    fn audio_sample(&self, _: Sample, _: Sample) {}
}

/// A DMG game booted by the CGB boot ROM ends up in DMG mode with WRAM
/// bank 1 mapped: the boot ROM selects it (it used bank 2) after the switch
/// to DMG mode, and still can.
#[test]
#[expect(clippy::expect_used, reason = "the test ROM is valid")]
fn cgb_boot_rom_leaves_dmg_games_on_wram_bank_1() {
    let mut rom = vec![0; 0x8000];
    rom[HEADER_LOGO..HEADER_LOGO + NINTENDO_LOGO.len()].copy_from_slice(&NINTENDO_LOGO);
    // `jr @` at the entry point.
    rom[0x100..0x102].copy_from_slice(&[0x18, 0xFE]);
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0_u8, |sum, &byte| sum.wrapping_sub(byte).wrapping_sub(1));

    let mut gb = GbBuilder::new(48000, Silent)
        .with_model(Model::CgbE)
        .with_rom(rom.into_boxed_slice())
        .expect("a valid ROM")
        .build();
    for _ in 0..600 {
        if !gb.bootrom.is_enabled() {
            break;
        }
        gb.run_frame();
    }

    assert!(!gb.bootrom.is_enabled(), "the boot ROM finished");
    assert!(matches!(gb.cgb_mode, CgbMode::Dmg | CgbMode::Compat));
    assert_eq!(gb.wram.svbk().bank_offset(), Wram::BANK_SIZE);
}
