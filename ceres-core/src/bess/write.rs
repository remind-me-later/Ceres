use {
    super::{
        CORE_BLOCK_SIZE, Execution, INFO_BLOCK_SIZE, IO_SIZE, Layout,
        MAJOR_VERSION, MINOR_VERSION, RTC_BLOCK_SIZE, XOAM_BLOCK_SIZE, model_id,
    },
    crate::{
        AudioCallback, Cartridge, CgbMode, Gb,
        memory::{BANK, HDMA1, KEY0, Wram},
        ppu::{ColorPalette, Vram},
    },
    alloc::vec::Vec,
    core::array,
};

/// KEY0 values: bit 2 set runs a DMG cartridge in compatibility mode.
const KEY0_CGB: u8 = 0x80;
const KEY0_DMG_MODE: u8 = 0x04;
/// The APU's registers, NR10-NR52.
const APU_FIRST: usize = 0x10;
const APU_LAST: usize = 0x26;

pub(crate) struct Writer<'a> {
    buf: &'a mut Vec<u8>,
    position: usize,
}

impl<'a> Writer<'a> {
    pub(crate) const fn new(buf: &'a mut Vec<u8>) -> Self {
        Self { buf, position: 0 }
    }

    pub(crate) fn save_state<A: AudioCallback>(&mut self, gb: &Gb<A>, secs_since_unix_epoch: u64) {
        let cgb = gb.model.is_cgb_hardware();
        let ram = if cgb { Wram::SIZE_CGB } else { Wram::SIZE_GB };
        let vram = if cgb { Vram::SIZE_CGB } else { Vram::SIZE_GB };
        // The palette memories are the CGB's only.
        let bg_palette = gb.ppu.bcp().raw();
        let obj_palette = gb.ppu.ocp().raw();
        let (bg_palette, obj_palette): (&[u8], &[u8]) = if cgb {
            (&bg_palette, &obj_palette)
        } else {
            (&[], &[])
        };
        // In the order of `Memory`.
        let memories = [
            &gb.wram.wram()[..usize::from(ram)],
            &gb.ppu.vram().bytes()[..usize::from(vram)],
            gb.cart.ram(),
            gb.ppu.oam().bytes(),
            gb.hram.hram().as_slice(),
            bg_palette,
            obj_palette,
        ];
        #[expect(clippy::cast_possible_truncation, reason = "128 KiB at most")]
        let layout = Layout::contiguous(memories.map(|m| m.len() as u32));
        for memory in memories {
            self.write_all(memory);
        }
        #[expect(clippy::cast_possible_truncation, reason = "a state is far below 4 GiB")]
        let offset_to_first_block = { self.position as u32 };

        self.write_name_block();
        self.write_info_block(&gb.cart);
        self.write_core_block(gb, &layout);
        if cgb {
            self.write_xoam_block(gb.ppu.oam().extra());
        }
        self.write_mbc_block(&gb.cart);
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

    fn write_core_block<A: AudioCallback>(&mut self, gb: &Gb<A>, layout: &Layout) {
        self.write_block_header(*b"CORE", CORE_BLOCK_SIZE);
        self.write_all(&MAJOR_VERSION.to_le_bytes());
        self.write_all(&MINOR_VERSION.to_le_bytes());
        self.write_all(&model_id(gb.model));

        let cpu = &gb.cpu;
        for reg in [cpu.pc(), cpu.af(), cpu.bc(), cpu.de(), cpu.hl(), cpu.sp()] {
            self.write_all(&reg.to_le_bytes());
        }
        let execution = if gb.clock.stopped {
            Execution::Stopped
        } else if cpu.is_halted() {
            Execution::Halted
        } else {
            Execution::Running
        };
        self.write_all(&[
            u8::from(cpu.ime()),
            gb.ints.read_ie(),
            execution as u8,
            // Reserved.
            0,
        ]);
        self.write_all(&io_registers(gb));

        for buffer in layout.0 {
            self.write_all(&buffer.size.to_le_bytes());
            self.write_all(&buffer.offset.to_le_bytes());
        }
    }

    fn write_end_block(&mut self) {
        self.write_block_header(*b"END ", 0);
    }

    fn write_footer(&mut self, offset_to_first_block: u32) {
        self.write_all(&offset_to_first_block.to_le_bytes());
        self.write_all(b"BESS");
    }

    fn write_info_block(&mut self, cart: &Cartridge) {
        self.write_block_header(*b"INFO", INFO_BLOCK_SIZE);

        self.write_all(&cart.header_title_area());
        // The checksum as it is in the header.
        self.write_all(&cart.global_checksum_bytes());
    }

    fn write_mbc_block(&mut self, cart: &Cartridge) {
        let writes = cart.bess_mbc_writes();
        if writes.is_empty() {
            return;
        }
        #[expect(clippy::cast_possible_truncation, reason = "a few writes")]
        self.write_block_header(*b"MBC ", (writes.len() * super::MBC_PAIR_SIZE) as u32);
        for (addr, val) in writes {
            self.write_all(&addr.to_le_bytes());
            self.write_all(&[val]);
        }
    }

    fn write_name_block(&mut self) {
        const EMULATOR_NAME: &str = concat!("Ceres v", env!("CARGO_PKG_VERSION"));

        #[expect(clippy::cast_possible_truncation, reason = "a short name")]
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

    fn write_xoam_block(&mut self, extra: &[u8]) {
        self.write_block_header(*b"XOAM", XOAM_BLOCK_SIZE);
        self.write_all(extra);
    }
}

/// The I/O registers as a ROM would write them back: the read values, but
/// for the registers that do not read back what was written.
fn io_registers<A: AudioCallback>(gb: &Gb<A>) -> [u8; IO_SIZE] {
    let mut io: [u8; IO_SIZE] = array::from_fn(|i| {
        #[expect(clippy::cast_possible_truncation, reason = "i < 0x80")]
        gb.read_mem(0xFF00 | i as u16)
    });
    for (reg, value) in io.iter_mut().enumerate().take(APU_LAST + 1).skip(APU_FIRST) {
        *value = gb.apu.bess_register(reg);
    }
    let hdma1 = usize::from(HDMA1);
    io[hdma1..hdma1 + 4].copy_from_slice(&gb.hdma.registers());
    if gb.model.is_cgb_hardware() {
        io[usize::from(KEY0)] = if matches!(gb.cgb_mode, CgbMode::Cgb) {
            KEY0_CGB
        } else {
            KEY0_DMG_MODE
        };
    }
    io[usize::from(BANK)] = u8::from(!gb.bootrom.is_enabled());
    io
}

/// Keeps `ColorPalette::RAW_SIZE` in sync with the CORE block's palettes.
const _: () = assert!(
    ColorPalette::RAW_SIZE == 0x40,
    "BESS stores 64 bytes per palette memory"
);
