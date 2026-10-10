//! Best Effort Save State (https://github.com/LIJI32/SameBoy/blob/master/BESS.md).
//! Every integer is in little-endian byte order.

mod read;
mod write;

/// CORE: the version, the model, the CPU registers and the I/O registers,
/// then a size and an offset for each of the 7 memories.
const CORE_BLOCK_SIZE: u32 = 0xD0;
/// The CPU state (0x10 bytes) and the I/O registers (0x80).
const CORE_REGISTERS_SIZE: usize = 0x90;
/// INFO: the title (0x10 bytes) and the global checksum.
const INFO_BLOCK_SIZE: u32 = 0x12;
const INFO_TITLE_SIZE: usize = 0x10;
/// RTC: the 10 clock registers (4 bytes each) and a 64-bit timestamp.
const RTC_BLOCK_SIZE: u32 = 0x30;
/// The CGB's background and object palette memories.
const CGB_PALETTES_SIZE: u32 = 0x40;
/// The footer: the offset of the first block, then "BESS".
const FOOTER_SIZE: usize = 8;

pub(crate) use read::Reader;
pub(crate) use write::Writer;

#[cfg(test)]
mod tests {
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
        for (model, cgb) in [(Model::DmgB, false), (Model::CgbE, false), (Model::CgbE, true)] {
            for (name, cart_type, ram_size) in carts {
                let mut gb = build(rom(cart_type, ram_size, cgb), model);
                gb.run_frame();
                assert_eq!(gb.cart.ram()[0x45] & 0xF, 2, "{name}, CGB {cgb}: ran");

                let mut state = Vec::new();
                gb.save_data(&mut state, 0);
                let mut loaded = build(rom(cart_type, ram_size, cgb), model);
                assert!(
                    loaded.load_data(&state, 0).is_ok(),
                    "{name}, CGB {cgb}: loads"
                );
                assert_eq!(loaded.read_mem(0xC123), 0x42, "{name}, CGB {cgb}: WRAM");
                assert_eq!(loaded.cart.ram(), gb.cart.ram(), "{name}, CGB {cgb}: RAM");
            }
        }
    }
}
