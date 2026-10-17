use crate::{AudioCallback, Gb, GbBuilder, Model, Sample};
use alloc::{boxed::Box, vec, vec::Vec};

struct Silent;

impl AudioCallback for Silent {
    fn audio_sample(&self, _: Sample, _: Sample) {}
}

/// A 32 KiB ROM that enables the cartridge RAM, writes 0x42 to $C123 and
/// $A045, then loops.
fn rom(cart_type: u8, ram_size: u8, cgb: bool) -> Box<[u8]> {
    const CODE: [u8; 15] = [
        0x3E, 0x0A, 0xEA, 0x00, 0x00, // ld [$0000], $0A
        0x3E, 0x42, 0xEA, 0x23, 0xC1, // ld [$C123], $42
        0xEA, 0x45, 0xA0, // ld [$A045], a
        0x18, 0xFE, // jr @
    ];
    let mut rom = vec![0; 0x8000];
    rom[0x100..0x100 + CODE.len()].copy_from_slice(&CODE);
    rom[0x143] = if cgb { 0x80 } else { 0 };
    rom[0x147] = cart_type;
    rom[0x149] = ram_size;
    rom.into_boxed_slice()
}

#[expect(clippy::expect_used, reason = "the test ROMs are valid")]
fn build(rom: Box<[u8]>, model: Model) -> Gb<Silent> {
    GbBuilder::new(48000, Silent)
        .with_model(model)
        .with_run_bootrom(false)
        .with_rom(rom)
        .expect("a valid ROM")
        .build()
}

#[test]
fn round_trip() {
    let carts = [
        ("MBC1+RAM+BATTERY", 0x03, 0x02),
        ("MBC1+RAM", 0x02, 0x02),
        ("MBC2+BATTERY", 0x06, 0x00),
        ("MBC2+BATTERY with a RAM size", 0x06, 0x02),
    ];
    for (model, cgb) in [
        (Model::DmgB, false),
        (Model::CgbE, false),
        (Model::CgbE, true),
    ] {
        for (name, cart_type, ram_size) in carts {
            let mut gb = build(rom(cart_type, ram_size, cgb), model);
            gb.run_frame();
            assert_eq!(gb.cart.ram()[0x45] & 0xF, 2, "{name}, CGB {cgb}: ran");

            let mut state = Vec::new();
            gb.save_state(&mut state, 0);
            let mut loaded = build(rom(cart_type, ram_size, cgb), model);
            assert!(
                loaded.load_state(&state, 0).is_ok(),
                "{name}, CGB {cgb}: loads"
            );
            assert_eq!(loaded.read_mem(0xC123), 0x42, "{name}, CGB {cgb}: WRAM");
            assert_eq!(loaded.cart.ram(), gb.cart.ram(), "{name}, CGB {cgb}: RAM");
        }
    }
}

/// A 64 KiB MBC1 ROM that sets registers and the CPU's to known values,
/// maps ROM bank 2, then loops. Each bank starts with its number.
fn register_rom(cgb: bool) -> Box<[u8]> {
    const CODE: [u8; 48] = [
        0x3E, 0x12, 0xE0, 0x43, // ld a, $12 ; ldh [SCX], a
        0x3E, 0x34, 0xE0, 0x42, // ld a, $34 ; ldh [SCY], a
        0x3E, 0xE4, 0xE0, 0x47, // ld a, $E4 ; ldh [BGP], a
        0x3E, 0x56, 0xE0, 0x06, // ld a, $56 ; ldh [TMA], a
        0x3E, 0x80, 0xE0, 0x68, // ld a, $80 ; ldh [BCPS], a
        0x3E, 0x1F, 0xE0, 0x69, // ld a, $1F ; ldh [BCPD], a
        0x3E, 0x03, 0xE0, 0x70, // ld a, $03 ; ldh [SVBK], a
        0x3E, 0x02, 0xEA, 0x00, 0x20, // ld a, $02 ; ld [$2000], a
        0x01, 0x34, 0x12, // ld bc, $1234
        0x11, 0x78, 0x56, // ld de, $5678
        0x21, 0xBC, 0x9A, // ld hl, $9ABC
        0x31, 0xF0, 0xDF, // ld sp, $DFF0
        0xFB, // ei
        0x18, 0xFE, // jr @
    ];
    let mut rom = vec![0; 0x10000];
    rom[0x100..0x100 + CODE.len()].copy_from_slice(&CODE);
    rom[0x143] = if cgb { 0x80 } else { 0 };
    rom[0x147] = 0x01; // MBC1
    rom[0x148] = 0x01; // 64 KiB
    for bank in 1..4_u8 {
        rom[0x4000 * usize::from(bank)] = bank;
    }
    rom.into_boxed_slice()
}

fn saved(gb: &Gb<Silent>) -> Vec<u8> {
    let mut state = Vec::new();
    gb.save_state(&mut state, 0);
    state
}

#[test]
fn restores_the_cpu_and_the_registers() {
    for (model, cgb) in [
        (Model::DmgB, false),
        (Model::Sgb, false),
        (Model::CgbE, false),
        (Model::CgbC, true),
    ] {
        let mut gb = build(register_rom(cgb), model);
        // (The SGB's boot ROM runs at build time and its time is paid back
        // over the first 30 frames.)
        for _ in 0..40 {
            gb.run_frame();
        }
        let state = saved(&gb);
        let mut loaded = build(register_rom(cgb), model);
        assert!(loaded.load_state(&state, 0).is_ok(), "{cgb}: loads");

        let (a, b) = (&gb.cpu, &loaded.cpu);
        assert_eq!(
            (a.pc(), a.af(), a.bc(), a.de(), a.hl(), a.sp(), a.ime()),
            (b.pc(), b.af(), b.bc(), b.de(), b.hl(), b.sp(), b.ime()),
            "{cgb}: CPU"
        );
        assert_eq!(gb.read_mem(0x4000), 2, "{cgb}: the ROM ran");
        assert_eq!(loaded.read_mem(0x4000), 2, "{cgb}: ROM bank");
        for reg in [0xFF43, 0xFF42, 0xFF47, 0xFF06, 0xFF40, 0xFF44] {
            assert_eq!(loaded.read_mem(reg), gb.read_mem(reg), "{cgb}: {reg:04X}");
        }
        assert_eq!(loaded.cgb_mode, gb.cgb_mode, "{cgb}: CGB mode");
        if cgb {
            assert_eq!(loaded.read_mem(0xFF70) & 7, 3, "SVBK");
            assert_eq!(loaded.ppu.bcp().raw(), gb.ppu.bcp().raw(), "palettes");
        }
        assert_eq!(saved(&loaded).len(), state.len(), "{cgb}: same layout");
    }
}

/// A block's name and contents.
#[derive(Clone)]
struct Block {
    name: [u8; 4],
    payload: Vec<u8>,
}

/// The 4 bytes at `at`.
#[expect(clippy::expect_used, reason = "the states are well formed")]
fn word(state: &[u8], at: usize) -> [u8; 4] {
    *state[at..].first_chunk().expect("in the state")
}

fn u32_at(state: &[u8], at: usize) -> usize {
    u32::from_le_bytes(word(state, at)) as usize
}

/// A state split into the data before the blocks and the blocks.
fn split(state: &[u8]) -> (Vec<u8>, Vec<Block>) {
    let first = u32_at(state, state.len() - 8);
    let mut blocks = Vec::new();
    let mut at = first;
    loop {
        let name = word(state, at);
        let size = u32_at(state, at + 4);
        blocks.push(Block {
            name,
            payload: state[at + 8..at + 8 + size].to_vec(),
        });
        at += 8 + size;
        if &name == b"END " {
            break;
        }
    }
    (state[..first].to_vec(), blocks)
}

#[expect(clippy::cast_possible_truncation, reason = "small test states")]
const fn len32(len: usize) -> [u8; 4] {
    (len as u32).to_le_bytes()
}

fn join(data: &[u8], blocks: &[Block]) -> Vec<u8> {
    let mut state = data.to_vec();
    for block in blocks {
        state.extend_from_slice(&block.name);
        state.extend_from_slice(&len32(block.payload.len()));
        state.extend_from_slice(&block.payload);
    }
    state.extend_from_slice(&len32(data.len()));
    state.extend_from_slice(b"BESS");
    state
}

/// Whether the state, changed by `change`, loads on a fresh machine; a
/// state that does not load leaves the machine as it was.
fn loads(model: Model, cgb: bool, change: impl FnOnce(&mut Vec<Block>)) -> bool {
    let mut gb = build(register_rom(cgb), model);
    gb.run_frame();
    let (data, mut blocks) = split(&saved(&gb));
    change(&mut blocks);
    let mut loaded = build(register_rom(cgb), model);
    let before = loaded.cpu.pc();
    let ok = loaded.load_state(&join(&data, &blocks), 0).is_ok();
    if !ok {
        assert_eq!(loaded.cpu.pc(), before, "a failed load changes nothing");
    }
    ok
}

#[expect(clippy::expect_used, reason = "the tests ask for blocks they wrote")]
fn index(blocks: &[Block], name: [u8; 4]) -> usize {
    blocks
        .iter()
        .position(|block| block.name == name)
        .expect("the block")
}

fn payload(blocks: &mut [Block], name: [u8; 4]) -> &mut Vec<u8> {
    let i = index(blocks, name);
    &mut blocks[i].payload
}

#[test]
fn writes_the_blocks_in_order() {
    let gb = build(register_rom(true), Model::CgbE);
    let (_, blocks) = split(&saved(&gb));
    let names: Vec<_> = blocks.iter().map(|block| block.name).collect();
    assert_eq!(
        names,
        [*b"NAME", *b"INFO", *b"CORE", *b"XOAM", *b"MBC ", *b"END "]
    );
    let core = &blocks[2].payload;
    assert_eq!(core.len(), 0xD0);
    assert_eq!(&core[4..8], b"CCE ");
    // KEY0 says CGB mode, BANK that the boot ROM is gone.
    assert_eq!(core[0x18 + 0x4C], 0x80);
    assert_eq!(core[0x18 + 0x50], 1);
    // MBC1: RAM enable, the ROM bank registers, the mode.
    assert_eq!(blocks[4].payload.len() % 3, 0);

    for (model, id) in [
        (Model::Sgb2, b"S2  "),
        (Model::Agb, b"CA  "),
        (Model::Mgb, b"GM  "),
    ] {
        let (_, other) = split(&saved(&build(register_rom(false), model)));
        assert_eq!(&other[2].payload[4..8], id);
    }
}

#[test]
fn follows_the_spec_on_invalid_states() {
    let m = Model::CgbE;
    // Unknown blocks are ignored, and so is excess CORE data.
    assert!(loads(m, true, |b| {
        let end = index(b, *b"END ");
        b.insert(
            end,
            Block {
                name: *b"ZZZZ",
                payload: vec![1, 2, 3],
            },
        );
    }));
    assert!(loads(m, true, |b| payload(b, *b"CORE").extend([0; 16])));
    // A newer minor version loads; another major does not.
    assert!(loads(m, true, |b| payload(b, *b"CORE")[2] = 9));
    assert!(!loads(m, true, |b| payload(b, *b"CORE")[0] = 2));
    // A missing END, a duplicate CORE, a known block before CORE.
    assert!(!loads(m, true, |b| {
        b.pop();
    }));
    assert!(!loads(m, true, |b| {
        let core = b[index(b, *b"CORE")].clone();
        b.insert(3, core);
    }));
    assert!(!loads(m, true, |b| {
        let xoam = b.remove(index(b, *b"XOAM"));
        b.insert(0, xoam);
    }));
    // Bad lengths, and MBC writes outside the cartridge.
    assert!(!loads(m, true, |b| {
        payload(b, *b"XOAM").pop();
    }));
    assert!(!loads(m, true, |b| payload(b, *b"MBC ").push(0)));
    assert!(!loads(m, true, |b| {
        payload(b, *b"MBC ").extend([0x00, 0x80, 0x00]);
    }));
    assert!(!loads(m, true, |b| payload(b, *b"END ").push(0)));
    // A memory past the end of the file.
    assert!(!loads(m, true, |b| payload(b, *b"CORE")[0x9C + 3] = 0x7F));
    // Another family of models, or an SGB block on a CGB.
    assert!(!loads(m, true, |b| payload(b, *b"CORE")[4] = b'G'));
    assert!(!loads(m, true, |b| {
        let end = index(b, *b"END ");
        b.insert(
            end,
            Block {
                name: *b"SGB ",
                payload: vec![0; 0x39],
            },
        );
    }));
    // A revision of the same family is fine.
    assert!(loads(Model::CgbC, true, |b| payload(b, *b"CORE")[6] = b'E'));
}

/// SameBoy puts its native state first: the memories BESS points to are
/// further in, at the offsets the CORE block gives.
#[test]
fn loads_a_state_with_native_data_first() {
    const NATIVE: usize = 0x123;
    let mut gb = build(register_rom(true), Model::CgbE);
    gb.run_frame();
    let (data, mut blocks) = split(&saved(&gb));
    let core = payload(&mut blocks, *b"CORE");
    for i in 0..7 {
        let at = 0x98 + 8 * i + 4;
        let offset = u32_at(core, at) + NATIVE;
        core[at..at + 4].copy_from_slice(&len32(offset));
    }
    let mut native = vec![0xAA; NATIVE];
    native.extend_from_slice(&data);
    let mut loaded = build(register_rom(true), Model::CgbE);
    assert!(matches!(
        loaded.load_state(&join(&native, &blocks), 0),
        Ok(())
    ));
    assert_eq!(loaded.wram.wram(), gb.wram.wram());
    assert_eq!(loaded.cpu.pc(), gb.cpu.pc());
}

/// Cut anywhere, a state fails to load instead of panicking.
#[test]
fn rejects_truncated_states() {
    let mut gb = build(register_rom(true), Model::CgbE);
    gb.run_frame();
    let state = saved(&gb);
    for len in 0..state.len() {
        assert!(gb.load_state(&state[..len], 0).is_err(), "{len} bytes");
    }
}

/// Booting with the saved progress: only the cartridge's RAM is taken, from
/// a state of any model.
#[test]
fn loads_only_the_save_data() {
    let mut gb = build(rom(0x03, 0x02, true), Model::CgbE);
    gb.run_frame();
    let state = saved(&gb);
    let mut dmg = build(rom(0x03, 0x02, true), Model::DmgB);
    assert!(dmg.load_state(&state, 0).is_err(), "another family");
    let pc = dmg.cpu.pc();
    assert!(matches!(dmg.load_save_data(&state, 0), Ok(())));
    assert_eq!(dmg.cart.ram(), gb.cart.ram(), "the cartridge RAM");
    assert_eq!(dmg.cpu.pc(), pc, "nothing else");
}

/// The states of older ceres versions only give their save data.
#[test]
fn takes_only_the_save_data_of_legacy_states() {
    let mut gb = build(rom(0x03, 0x02, true), Model::CgbE);
    gb.run_frame();
    let (data, mut blocks) = split(&saved(&gb));
    payload(&mut blocks, *b"NAME").clone_from(&b"Ceres, 0.1.0".to_vec());
    let state = join(&data, &blocks);
    let mut loaded = build(rom(0x03, 0x02, true), Model::CgbE);
    assert!(loaded.load_state(&state, 0).is_err(), "not resumed");
    assert!(matches!(loaded.load_save_data(&state, 0), Ok(())));
    assert_eq!(loaded.cart.ram(), gb.cart.ram());
}
