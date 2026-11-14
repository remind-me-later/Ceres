use crate::{
    AudioCallback, CgbMode, GbBuilder, Model, Sample,
    cartridge::{HEADER_LOGO, NINTENDO_LOGO},
    memory::{BANK, Wram, undecoded},
};
use alloc::{vec, vec::Vec};

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

/// An MBC1 cartridge with RAM, booting.
#[expect(clippy::expect_used, reason = "the test ROM is valid")]
fn dmg_with_mbc1_ram(model: Model) -> crate::Gb<Silent> {
    let mut rom = vec![0; 0x8000];
    rom[HEADER_LOGO..HEADER_LOGO + NINTENDO_LOGO.len()].copy_from_slice(&NINTENDO_LOGO);
    // MBC1 with RAM and a battery, 32 KiB of ROM, 8 KiB of RAM.
    rom[0x147] = 0x03;
    rom[0x149] = 0x02;
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0_u8, |sum, &byte| sum.wrapping_sub(byte).wrapping_sub(1));
    GbBuilder::new(48000, Silent)
        .with_model(model)
        .with_rom(rom.into_boxed_slice())
        .expect("a valid ROM")
        .build()
}

fn state(gb: &crate::Gb<Silent>) -> Vec<u8> {
    let mut buf = Vec::new();
    gb.save_state(&mut buf, 0);
    buf
}

const DMG_MODELS: [Model; 5] = [
    Model::Dmg0,
    Model::DmgB,
    Model::Mgb,
    Model::Sgb,
    Model::Sgb2,
];

/// Ceres's DMG routes the addresses as the netlist's chip decodes them
/// (`memory::netlist_proofs` proves what HRAM and the boot ROM then do):
/// FF00 to FF7F's undecoded addresses read FF and ignore writes, FF50 reads
/// the mapping in bit 0, and while the boot ROM is mapped a write to its
/// addresses does not reach the cartridge.
#[test]
fn dmg_map_is_the_netlists() {
    for (i, model) in DMG_MODELS.into_iter().enumerate() {
        let mut gb = dmg_with_mbc1_ram(model);
        assert!(gb.bootrom.is_enabled());

        for mapped in [true, false] {
            for addr in 0xFF00..=0xFF7F_u16 {
                if !undecoded(addr) {
                    continue;
                }
                assert_eq!(gb.read_mem(addr), 0xFF, "model {i} reads {addr:04X}");
                let before = state(&gb);
                for val in [0x00, 0xFF, 0x5A] {
                    gb.write_mem(addr, val);
                }
                assert!(before == state(&gb), "model {i} writes {addr:04X}");
            }
            let bank = super::io_addr(BANK);
            assert_eq!(
                gb.read_mem(bank),
                if mapped { 0xFE } else { 0xFF },
                "model {i} reads FF50"
            );

            // The boot ROM's addresses: RAM stays disabled while it is mapped.
            for addr in 0x0000..=0x00FF {
                gb.write_mem(addr, 0x0A);
            }
            gb.write_mem(0xA000, 0x00);
            assert_eq!(
                gb.read_mem(0xA000) == 0xFF,
                mapped,
                "model {i}: the cartridge saw the write (mapped: {mapped})"
            );
            gb.write_mem(0x0000, 0x00);

            // Bit 0 clear keeps the mapping; set, unmaps it.
            gb.write_mem(bank, 0xFE);
            assert_eq!(gb.bootrom.is_enabled(), mapped);
            gb.write_mem(bank, 0x01);
            assert!(!gb.bootrom.is_enabled());
        }
    }
}
