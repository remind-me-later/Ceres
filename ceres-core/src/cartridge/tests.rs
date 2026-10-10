//! Bank switching of each mapper, on ROMs whose banks hold their own number.

use super::{Cartridge, HEADER_LOGO, NINTENDO_LOGO, ram_size::RAMSize, rom_size::ROMSize};
use alloc::{boxed::Box, vec};

const BANK: usize = 0x4000;

/// A ROM of `banks` banks whose first two bytes hold the bank number.
fn rom(cart_type: u8, ram_size: u8, banks: usize) -> Box<[u8]> {
    let mut rom = vec![0; banks * BANK];
    for (bank, chunk) in (0_u16..).zip(rom.as_chunks_mut::<BANK>().0) {
        chunk[..2].copy_from_slice(&bank.to_le_bytes());
    }
    rom[0x147] = cart_type;
    rom[0x149] = ram_size;
    rom.into_boxed_slice()
}

#[expect(clippy::expect_used, reason = "the test ROMs are valid")]
fn cart(rom: Box<[u8]>) -> Cartridge {
    Cartridge::new(rom).expect("a valid ROM")
}

/// The bank mapped at `addr` (0x0000 or 0x4000).
fn bank(cart: &Cartridge, addr: u16) -> u16 {
    u16::from_le_bytes([cart.read_rom(addr), cart.read_rom(addr + 1)])
}

fn enable_ram(cart: &mut Cartridge) {
    cart.write_rom(0x0000, 0x0A);
}

#[test]
fn cartridge_types() {
    let supported = [
        (0x00, false),
        (0x01, false),
        (0x02, false),
        (0x03, true),
        (0x05, false),
        (0x06, true),
        (0x0F, true),
        (0x10, true),
        (0x11, false),
        (0x12, false),
        (0x13, true),
        (0x19, false),
        (0x1A, false),
        (0x1B, true),
    ];
    for cart_type in 0..=u8::MAX {
        let expected = supported
            .iter()
            .find(|&&(t, _)| t == cart_type)
            .map(|&(_, battery)| battery);
        let battery = Cartridge::new(rom(cart_type, 0, 2))
            .ok()
            .map(|c| c.has_battery());
        assert_eq!(battery, expected, "type {cart_type:02X}");
    }
}

#[test]
fn sizes() {
    for (len, size) in [
        (0x150, Some(0x8000)),
        (0x8000, Some(0x8000)),
        (0x8001, Some(0x1_0000)),
        (0x80_0000, Some(0x80_0000)),
        (0x80_0001, None),
    ] {
        assert_eq!(
            ROMSize::from_len(len).ok().map(ROMSize::size_bytes),
            size,
            "ROM of {len:X} bytes"
        );
    }
    for (byte, size) in [
        (0, Some(0)),
        (1, None),
        (2, Some(0x2000)),
        (3, Some(0x8000)),
        (4, Some(0x2_0000)),
        (5, Some(0x1_0000)),
        (6, None),
    ] {
        assert_eq!(
            RAMSize::new(byte).ok().map(RAMSize::size_bytes),
            size,
            "RAM size {byte}"
        );
    }
}

#[test]
fn mbc1() {
    let mut c = cart(rom(0x01, 0, 128));
    assert_eq!((bank(&c, 0), bank(&c, 0x4000)), (0, 1), "power on");
    c.write_rom(0x2000, 0);
    assert_eq!(bank(&c, 0x4000), 1, "bank 0 maps bank 1");
    c.write_rom(0x2000, 0x25);
    assert_eq!(bank(&c, 0x4000), 0x05, "5 bits");
    c.write_rom(0x4000, 2);
    assert_eq!(bank(&c, 0x4000), 0x45, "upper bits");
    assert_eq!(bank(&c, 0), 0, "mode 0");
    c.write_rom(0x6000, 1);
    assert_eq!(bank(&c, 0), 0x40, "mode 1");
    c.write_rom(0x2000, 0x20);
    assert_eq!(bank(&c, 0x4000), 0x41, "bank 0x40 maps 0x41");

    let mut small = cart(rom(0x01, 0, 32));
    small.write_rom(0x4000, 3);
    small.write_rom(0x2000, 7);
    assert_eq!(bank(&small, 0x4000), 7, "upper bits past the ROM");
    small.write_rom(0x2000, 0);
    assert_eq!(bank(&small, 0x4000), 1, "bank 0x60 maps 0x61");
}

#[test]
fn mbc1_multicart() {
    let mut rom = rom(0x01, 0, 64).into_vec();
    for chunk in rom.as_chunks_mut::<BANK>().0 {
        chunk[HEADER_LOGO..HEADER_LOGO + NINTENDO_LOGO.len()].copy_from_slice(&NINTENDO_LOGO);
    }
    let mut c = cart(rom.into_boxed_slice());
    c.write_rom(0x4000, 1);
    c.write_rom(0x2000, 2);
    assert_eq!(bank(&c, 0x4000), 18, "4 bits per quadrant");
    c.write_rom(0x2000, 0x10);
    assert_eq!(bank(&c, 0x4000), 16, "bit 4 cancels the 0 correction");
    assert_eq!(bank(&c, 0), 0, "mode 0");
    c.write_rom(0x6000, 1);
    assert_eq!(bank(&c, 0), 16, "mode 1 maps the quadrant");
}

#[test]
fn mbc1_ram() {
    let mut c = cart(rom(0x03, 3, 4));
    c.write_ram(0xA000, 0x12);
    assert_eq!(c.read_ram(0xA000), 0xFF, "disabled");
    enable_ram(&mut c);
    c.write_ram(0xA000, 0x12);
    c.write_rom(0x6000, 1);
    c.write_rom(0x4000, 2);
    assert_eq!(c.read_ram(0xA000), 0xFF, "bank 2");
    c.write_ram(0xA000, 0x22);
    c.write_rom(0x4000, 0);
    assert_eq!(c.read_ram(0xA000), 0x12, "bank 0");
    c.write_rom(0x0000, 0);
    assert_eq!(c.read_ram(0xA000), 0xFF, "disabled again");
}

#[test]
fn mbc2() {
    let mut c = cart(rom(0x06, 0, 16));
    c.write_rom(0x2100, 3);
    assert_eq!(bank(&c, 0x4000), 3, "bank");
    c.write_rom(0x2100, 0x10);
    assert_eq!(bank(&c, 0x4000), 1, "4 bits, 0 maps 1");
    c.write_rom(0x2000, 5);
    assert_eq!(bank(&c, 0x4000), 1, "bit 8 clear: RAM enable");

    enable_ram(&mut c);
    c.write_ram(0xA123, 0xAB);
    assert_eq!(c.read_ram(0xA123), 0xFB, "4-bit RAM");
    assert_eq!(c.read_ram(0xA323), 0xFB, "512 bytes, mirrored");
}

#[test]
fn mbc3() {
    let mut c = cart(rom(0x11, 0, 128));
    c.write_rom(0x2000, 0);
    assert_eq!(bank(&c, 0x4000), 1, "bank 0 maps bank 1");
    c.write_rom(0x2000, 0x85);
    assert_eq!(bank(&c, 0x4000), 5, "7 bits");

    let mut mbc30 = cart(rom(0x11, 0, 256));
    mbc30.write_rom(0x2000, 0x85);
    assert_eq!(bank(&mbc30, 0x4000), 0x85, "MBC30: 8 bits");

    let mut ram = cart(rom(0x13, 3, 4));
    enable_ram(&mut ram);
    ram.write_rom(0x4000, 2);
    ram.write_ram(0xA000, 0x22);
    ram.write_rom(0x4000, 0);
    assert_eq!(ram.read_ram(0xA000), 0xFF, "bank 0");
    ram.write_rom(0x4000, 2);
    assert_eq!(ram.read_ram(0xA000), 0x22, "bank 2");
}

#[test]
fn mbc3_rtc() {
    let mut c = cart(rom(0x10, 3, 4));
    enable_ram(&mut c);
    c.write_ram(0xA000, 0x33);
    c.write_rom(0x4000, 0x08);
    c.write_ram(0xA000, 30);
    c.write_rom(0x6000, 1);
    assert_eq!(c.read_ram(0xA000), 30, "seconds");
    c.write_rom(0x4000, 0);
    assert_eq!(c.read_ram(0xA000), 0x33, "RAM again");
}

#[test]
fn mbc5() {
    let mut c = cart(rom(0x19, 0, 512));
    c.write_rom(0x2000, 0x34);
    c.write_rom(0x3000, 1);
    assert_eq!(bank(&c, 0x4000), 0x134, "9 bits");
    c.write_rom(0x2000, 0);
    c.write_rom(0x3000, 0);
    assert_eq!(bank(&c, 0x4000), 0, "bank 0 is not corrected");

    let mut small = cart(rom(0x19, 0, 64));
    small.write_rom(0x2000, 0x45);
    assert_eq!(bank(&small, 0x4000), 5, "masked by the ROM size");

    let mut ram = cart(rom(0x1B, 4, 4));
    enable_ram(&mut ram);
    ram.write_rom(0x4000, 0x0F);
    ram.write_ram(0xBFFF, 0x5A);
    ram.write_rom(0x4000, 0);
    assert_eq!(ram.read_ram(0xBFFF), 0xFF, "bank 0");
    ram.write_rom(0x4000, 0x1F);
    assert_eq!(ram.read_ram(0xBFFF), 0x5A, "4 bits");
}
