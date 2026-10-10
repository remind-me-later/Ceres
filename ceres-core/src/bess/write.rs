use super::{CGB_PALETTES_SIZE, CORE_BLOCK_SIZE, INFO_BLOCK_SIZE, INFO_TITLE_SIZE, RTC_BLOCK_SIZE};
use alloc::vec::Vec;

use crate::{
    AudioCallback, Cartridge, CgbMode, Gb, Model,
    memory::{Hram, Wram},
    ppu::{Oam, Vram},
};

pub(crate) struct Writer<'a> {
    buf: &'a mut Vec<u8>,
    position: usize,
}

impl<'a> Writer<'a> {
    pub(crate) const fn new(buf: &'a mut Vec<u8>) -> Self {
        Self { buf, position: 0 }
    }

    pub(crate) fn save_state<A: AudioCallback>(&mut self, gb: &Gb<A>, secs_since_unix_epoch: u64) {
        let cgb = matches!(gb.cgb_mode, CgbMode::Cgb);
        let palette = if cgb { CGB_PALETTES_SIZE } else { 0 };
        let sizes = WrittenSizes {
            ram: u32::from(if cgb { Wram::SIZE_CGB } else { Wram::SIZE_GB }),
            vram: u32::from(if cgb { Vram::SIZE_CGB } else { Vram::SIZE_GB }),
            #[expect(clippy::cast_possible_truncation, reason = "128 KiB at most")]
            mbc_ram: gb.cart.ram().len() as u32,
            oam: u32::from(Oam::SIZE),
            hram: u32::from(Hram::SIZE),
            bg_palette: palette,
            obj_palette: palette,
        };

        // Write RAM
        self.write_all(&gb.wram.wram()[..sizes.ram as usize]);

        // Write VRAM
        self.write_all(&gb.ppu.vram().bytes()[..sizes.vram as usize]);

        // Write MBC RAM
        self.write_all(gb.cart.ram());

        // Write OAM
        self.write_all(&gb.ppu.oam().bytes()[..sizes.oam as usize]);

        // Write HRAM
        self.write_all(gb.hram.hram().as_slice());

        // Write the background and object palettes
        if cgb {
            self.write_all(&[0; 2 * CGB_PALETTES_SIZE as usize]);
        }
        #[expect(clippy::cast_possible_truncation)]
        let offset_to_first_block = { self.position as u32 };

        self.write_name_block();
        self.write_info_block(&gb.cart);
        self.write_core_block(gb, &sizes);
        self.write_rtc_block(secs_since_unix_epoch, &gb.cart);
        self.write_end_block();
        self.write_footer(offset_to_first_block);
    }

    fn write_all(&mut self, buf: &[u8]) {
        self.buf.extend_from_slice(buf);
        self.position += buf.len();
    }

    fn write_block_header(&mut self, name: [u8; 4], size: u32) {
        self.write_all(&name);
        self.write_all(&size.to_le_bytes());
    }

    fn write_core_block<A: AudioCallback>(&mut self, gb: &Gb<A>, sizes: &WrittenSizes) {
        self.write_block_header(*b"CORE", CORE_BLOCK_SIZE);

        // BESS Version
        {
            const MAJOR_VERSION: u16 = 1;
            const MINOR_VERSION: u16 = 1;

            self.write_all(&MAJOR_VERSION.to_le_bytes());
            self.write_all(&MINOR_VERSION.to_le_bytes());
        }

        // Model
        {
            let model = match gb.model {
                Model::Dmg0 => "GD0 ",
                Model::DmgB => "GDB ",
                Model::Mgb => "GM  ",
                Model::Sgb => "GSB ",
                Model::Sgb2 => "GS2 ",
                Model::Cgb0 => "CC0 ",
                Model::CgbA => "CCA ",
                Model::CgbB => "CCB ",
                Model::CgbC => "CCC ",
                Model::CgbD => "CCD ",
                Model::CgbE => "CCE ",
                Model::Agb => "AGB ",
            };

            self.write_all(model.as_bytes());
        }

        // CPU Registers
        {
            self.write_all(&gb.cpu.pc().to_le_bytes()); // PC
            self.write_all(&gb.cpu.af().to_le_bytes()); // AF
            self.write_all(&gb.cpu.bc().to_le_bytes()); // BC
            self.write_all(&gb.cpu.de().to_le_bytes()); // DE
            self.write_all(&gb.cpu.hl().to_le_bytes()); // HL
            self.write_all(&gb.cpu.sp().to_le_bytes()); // SP
            self.write_all(&[u8::from(gb.cpu.ime())]); // IME
            self.write_all(&[gb.ints.read_ie()]); // IE

            // Execution state (TODO: stopped state)
            self.write_all(&[u8::from(gb.cpu.is_halted())]);
            // Reserved byte, must be zero according to BESS specification
            self.write_all(&[0]);

            // Every memory mapped register
            for i in 0xFF00..0xFF80 {
                self.write_all(&[gb.read_mem(i)]);
            }
        }

        // Sizes
        {
            self.write_all(&sizes.ram.to_le_bytes());
            self.write_all(&sizes.ram_offset().to_le_bytes());
            self.write_all(&sizes.vram.to_le_bytes());
            self.write_all(&sizes.vram_offset().to_le_bytes());
            self.write_all(&sizes.mbc_ram.to_le_bytes());
            self.write_all(&sizes.mbc_ram_offset().to_le_bytes());
            self.write_all(&sizes.oam.to_le_bytes());
            self.write_all(&sizes.oam_offset().to_le_bytes());
            self.write_all(&sizes.hram.to_le_bytes());
            self.write_all(&sizes.hram_offset().to_le_bytes());
            self.write_all(&sizes.bg_palette.to_le_bytes());
            self.write_all(&sizes.bg_palette_offset().to_le_bytes());
            self.write_all(&sizes.obj_palette.to_le_bytes());
            self.write_all(&sizes.obj_palette_offset().to_le_bytes());
        }
    }

    fn write_end_block(&mut self) {
        self.write_block_header(*b"END ", 0);
    }

    fn write_footer(&mut self, offset_to_first_block: u32) {
        const LITERAL: &[u8] = b"BESS";

        self.write_all(&offset_to_first_block.to_le_bytes());
        self.write_all(LITERAL);
    }

    fn write_info_block(&mut self, cart: &Cartridge) {
        self.write_block_header(*b"INFO", INFO_BLOCK_SIZE);

        // pad title to 0x10 bytes
        let mut title = [0; INFO_TITLE_SIZE];
        let title_bytes = cart.ascii_title();
        let title_len = title_bytes.len();
        title[0..title_len].copy_from_slice(title_bytes);

        self.write_all(&title);
        self.write_all(&cart.global_checksum().to_le_bytes());
    }

    fn write_name_block(&mut self) {
        const EMULATOR_NAME: &str = "Ceres, 0.1.0";

        #[expect(clippy::cast_possible_truncation)]
        self.write_block_header(*b"NAME", EMULATOR_NAME.len() as u32);
        self.write_all(EMULATOR_NAME.as_bytes());
    }

    fn write_rtc_block(&mut self, secs_since_unix_epoch: u64, cart: &Cartridge) {
        if let Some(rtc) = cart.rtc() {
            self.write_block_header(*b"RTC ", RTC_BLOCK_SIZE);

            // Each register is a byte and 3 bytes of padding: the real
            // registers, then the latched ones.
            for value in rtc.real().into_iter().chain(rtc.latched()) {
                self.write_all(&[value, 0, 0, 0]);
            }
            self.write_all(&secs_since_unix_epoch.to_le_bytes());
        }
    }
}

#[derive(Default)]
struct WrittenSizes {
    bg_palette: u32,
    hram: u32,
    mbc_ram: u32,
    oam: u32,
    obj_palette: u32,
    ram: u32,
    vram: u32,
}

impl WrittenSizes {
    const fn bg_palette_offset(&self) -> u32 {
        self.hram_offset() + self.hram
    }

    const fn hram_offset(&self) -> u32 {
        self.oam_offset() + self.oam
    }

    const fn mbc_ram_offset(&self) -> u32 {
        self.vram_offset() + self.vram
    }

    const fn oam_offset(&self) -> u32 {
        self.mbc_ram_offset() + self.mbc_ram
    }

    const fn obj_palette_offset(&self) -> u32 {
        self.bg_palette_offset() + self.bg_palette
    }

    #[expect(clippy::unused_self)]
    const fn ram_offset(&self) -> u32 {
        0
    }

    const fn vram_offset(&self) -> u32 {
        self.ram
    }
}
