use {
    super::{
        Buffer, CORE_BLOCK_SIZE, Execution, FOOTER_SIZE, HUC3_BLOCK_SIZE, INFO_BLOCK_SIZE, IO_SIZE,
        Layout, MAJOR_VERSION, MBC_PAIR_SIZE, MBC7_BLOCK_SIZE, Memory, RTC_BLOCK_SIZE,
        TPP1_BLOCK_SIZE, XOAM_BLOCK_SIZE, same_family,
    },
    crate::{
        AudioCallback, CgbMode, Gb,
        error::Error,
        memory::{
            BANK, BCPS, BGP, DIV, DMA, HDMA1, HDMA2, HDMA3, HDMA4, IF, KEY0, KEY1, LCDC, LY, LYC,
            NR52, OBP0, OBP1, OCPS, P1, SB, SC, SCX, SCY, STAT, SVBK, TAC, TIMA, TMA, UNDOC_FF72,
            UNDOC_FF73, UNDOC_FF75, VBK, WX, WY, io_addr,
        },
        ppu::ColorPalette,
    },
};

/// The APU's registers before NR52, and the wave RAM.
const APU_FIRST: u8 = 0x10;
const WAV_FIRST: u8 = 0x30;
const WAV_LAST: u8 = 0x3F;
/// NRx4's trigger bit.
const NRX4_TRIGGER: u8 = 0x80;
const NRX4: [u8; 4] = [0x14, 0x19, 0x1E, 0x23];
/// KEY0's bit 2: a DMG cartridge running in compatibility mode.
const KEY0_DMG_MODE: u8 = 0x04;
/// KEY1's bit 7: double speed.
const KEY1_DOUBLE_SPEED: u8 = 0x80;
/// The last line LY can hold.
const LAST_LINE: u8 = 153;
/// A dot of a line where LY reads the line (it changes for reads a few dots
/// into it).
const LY_READ_DOT: i32 = 16;

/// The CORE block's contents.
struct Core {
    family: u8,
    pc: u16,
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    ime: bool,
    ie: u8,
    execution: Execution,
    io: [u8; IO_SIZE],
    layout: Layout,
}

/// The NAME of the states ceres wrote before it restored the whole state:
/// they lack the MBC block and the palettes, so only their save data is
/// usable.
const LEGACY_CERES_NAME: &[u8] = b"Ceres, 0.1.0";

/// A parsed and checked state: loading it cannot fail anymore.
struct State<'a> {
    core: Core,
    /// Written by an old ceres (`LEGACY_CERES_NAME`).
    legacy: bool,
    /// The MBC block's (address, value) writes, in order.
    mbc: &'a [u8],
    rtc: Option<&'a [u8]>,
    xoam: Option<&'a [u8]>,
}

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    /// Loads a state. A state that is not valid BESS, or one from another
    /// family of models, leaves `gb` as it was.
    pub(crate) fn load_state<A: AudioCallback>(
        &mut self,
        gb: &mut Gb<A>,
        secs_since_unix_epoch: u64,
    ) -> Result<(), Error> {
        let state = self.parse(gb, true)?;
        if state.legacy {
            return Err(Error::InvalidSaveState);
        }
        gb.load_bess(&state, self.data, secs_since_unix_epoch);
        Ok(())
    }

    /// Loads only the cartridge's RAM and clock from a state (what a
    /// battery keeps), from any model.
    pub(crate) fn load_save_data<A: AudioCallback>(
        &mut self,
        gb: &mut Gb<A>,
        secs_since_unix_epoch: u64,
    ) -> Result<(), Error> {
        let state = self.parse(gb, false)?;
        let ram = state.core.layout.get(Memory::MbcRam);
        copy_memory(self.data, ram, gb.cart.ram_mut());
        if let Some(rtc) = state.rtc {
            gb.load_rtc(rtc, secs_since_unix_epoch);
        }
        Ok(())
    }

    /// Reads the blocks, with the checks BESS asks for (and those SameBoy
    /// treats as fatal); `same_model` also requires the state to be from
    /// the family of `gb`'s model.
    fn parse<A: AudioCallback>(
        &mut self,
        gb: &Gb<A>,
        same_model: bool,
    ) -> Result<State<'a>, Error> {
        let offset_to_first_block = self.read_footer()?;
        self.seek(offset_to_first_block as usize)?;

        let mut core = None;
        let mut mbc: &[u8] = &[];
        let mut rtc = None;
        let mut xoam = None;
        let mut legacy = false;
        loop {
            let name = self.take(4)?;
            let size = self.read_u32()?;
            let block = self.take(size as usize)?;
            // Every known block but NAME and INFO comes after CORE.
            let known_after_core = matches!(
                name,
                b"XOAM" | b"MBC " | b"RTC " | b"HUC3" | b"TPP1" | b"MBC7" | b"SGB " | b"END "
            );
            if known_after_core && core.is_none() {
                return Err(Error::InvalidSaveState);
            }
            let exact = |expected: u32| {
                if size == expected {
                    Ok(())
                } else {
                    Err(Error::InvalidSaveState)
                }
            };
            match name {
                b"CORE" => {
                    if core.is_some() {
                        return Err(Error::InvalidSaveState);
                    }
                    core = Some(parse_core(block)?);
                }
                b"INFO" => exact(INFO_BLOCK_SIZE)?,
                b"XOAM" => {
                    exact(XOAM_BLOCK_SIZE)?;
                    xoam = Some(block);
                }
                b"MBC " => {
                    if block.len() % MBC_PAIR_SIZE != 0 {
                        return Err(Error::InvalidSaveState);
                    }
                    for &[lo, hi, _] in block.as_chunks::<MBC_PAIR_SIZE>().0 {
                        let addr = u16::from_le_bytes([lo, hi]);
                        if !matches!(addr, 0x0000..=0x7FFF | 0xA000..=0xBFFF) {
                            return Err(Error::InvalidSaveState);
                        }
                    }
                    mbc = block;
                }
                b"RTC " => {
                    exact(RTC_BLOCK_SIZE)?;
                    rtc = Some(block);
                }
                // Cartridges ceres does not emulate: checked, not used.
                b"HUC3" => exact(HUC3_BLOCK_SIZE)?,
                b"TPP1" => exact(TPP1_BLOCK_SIZE)?,
                b"MBC7" => exact(MBC7_BLOCK_SIZE)?,
                // The SGB's state is not restored (as if its commands were
                // disabled), but only an SGB state may carry it.
                b"SGB " => {
                    if !matches!(core.as_ref().map(|c| c.family), Some(b'S')) {
                        return Err(Error::InvalidSaveState);
                    }
                }
                b"END " => {
                    exact(0)?;
                    break;
                }
                b"NAME" => legacy = block == LEGACY_CERES_NAME,
                // The blocks of later versions: ignored.
                _ => {}
            }
        }

        let core = core.ok_or(Error::InvalidSaveState)?;
        if same_model && !same_family(core.family, gb.model) {
            return Err(Error::InvalidSaveState);
        }
        for buffer in core.layout.0 {
            let end = buffer.offset.checked_add(buffer.size);
            if end.is_none_or(|end| end as usize > self.data.len()) {
                return Err(Error::InvalidSaveState);
            }
        }
        Ok(State {
            core,
            legacy,
            mbc,
            rtc,
            xoam,
        })
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let bytes = self
            .data
            .get(self.position..)
            .and_then(|rest| rest.get(..n))
            .ok_or(Error::InvalidSaveState)?;
        self.position += n;
        Ok(bytes)
    }

    fn read_u32(&mut self) -> Result<u32, Error> {
        let bytes = self.take(4)?;
        let bytes = bytes.first_chunk().ok_or(Error::InvalidSaveState)?;
        Ok(u32::from_le_bytes(*bytes))
    }

    fn read_footer(&mut self) -> Result<u32, Error> {
        let start = self
            .data
            .len()
            .checked_sub(FOOTER_SIZE)
            .ok_or(Error::InvalidSaveState)?;
        self.seek(start)?;
        let offset_to_first_block = self.read_u32()?;
        if self.take(4)? != b"BESS" {
            return Err(Error::InvalidSaveState);
        }
        Ok(offset_to_first_block)
    }

    const fn seek(&mut self, position: usize) -> Result<(), Error> {
        if position > self.data.len() {
            return Err(Error::InvalidSaveState);
        }
        self.position = position;
        Ok(())
    }
}

/// The CORE block. A shorter block reads as zeros past its end, a longer one
/// is cut (later versions may add to it).
fn parse_core(block: &[u8]) -> Result<Core, Error> {
    let mut core = [0; CORE_BLOCK_SIZE as usize];
    let len = block.len().min(core.len());
    core[..len].copy_from_slice(&block[..len]);
    let u16_at = |i: usize| u16::from_le_bytes([core[i], core[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([core[i], core[i + 1], core[i + 2], core[i + 3]]);

    // A different major version is not compatible; later minor ones are.
    if u16_at(0x00) != MAJOR_VERSION {
        return Err(Error::InvalidSaveState);
    }
    let execution = match core[0x16] {
        0 => Execution::Running,
        1 => Execution::Halted,
        2 => Execution::Stopped,
        _ => return Err(Error::InvalidSaveState),
    };
    let mut io = [0; IO_SIZE];
    io.copy_from_slice(&core[0x18..0x18 + IO_SIZE]);
    let mut layout = Layout::default();
    for (i, buffer) in layout.0.iter_mut().enumerate() {
        let at = 0x98 + 8 * i;
        *buffer = Buffer {
            size: u32_at(at),
            offset: u32_at(at + 4),
        };
    }
    Ok(Core {
        family: core[0x04],
        pc: u16_at(0x08),
        af: u16_at(0x0A),
        bc: u16_at(0x0C),
        de: u16_at(0x0E),
        hl: u16_at(0x10),
        sp: u16_at(0x12),
        ime: core[0x14] != 0,
        ie: core[0x15],
        execution,
        io,
        layout,
    })
}

/// Copies a memory of the state into `dest`: data past `dest` is dropped,
/// and what the state lacks (a CGB's second VRAM bank, say) is zero.
fn copy_memory(data: &[u8], buffer: Buffer, dest: &mut [u8]) {
    dest.fill(0);
    let start = buffer.offset as usize;
    let src = &data[start..start + buffer.size as usize];
    let len = src.len().min(dest.len());
    dest[..len].copy_from_slice(&src[..len]);
}

impl<A: AudioCallback> Gb<A> {
    /// Restores a parsed state: a reset, then the registers written as a
    /// ROM would write them (in an order that keeps them from interfering,
    /// and without starting transfers or sounds), then the memories.
    fn load_bess(&mut self, state: &State<'_>, data: &[u8], secs_since_unix_epoch: u64) {
        let core = &state.core;
        self.soft_reset();
        self.load_io_registers(&core.io);

        // The cartridge's registers, in order.
        for &[lo, hi, val] in state.mbc.as_chunks::<MBC_PAIR_SIZE>().0 {
            self.write_mem(u16::from_le_bytes([lo, hi]), val);
        }

        self.load_memories(&core.layout, data);
        if let Some(xoam) = state.xoam {
            self.ppu.oam_mut().extra_mut().copy_from_slice(xoam);
        }
        if let Some(rtc) = state.rtc {
            self.load_rtc(rtc, secs_since_unix_epoch);
        }

        // The interrupts, then the CPU.
        self.write_mem(io_addr(IF), core.io[usize::from(IF)]);
        self.ints.write_ie(core.ie);
        let cpu = &mut self.cpu;
        cpu.set_pc(core.pc);
        cpu.set_af(core.af);
        cpu.set_bc(core.bc);
        cpu.set_de(core.de);
        cpu.set_hl(core.hl);
        cpu.set_sp(core.sp);
        cpu.set_ime(core.ime);
        cpu.set_halted(core.execution == Execution::Halted);
        if core.execution == Execution::Stopped {
            self.ppu.enter_stop_mode();
            self.clock.stopped = true;
            self.ppu.set_cpu_idle(true);
        }
    }

    /// Writes the I/O registers of a state as a ROM would, in an order that
    /// keeps them from interfering, without starting transfers or sounds.
    fn load_io_registers(&mut self, io: &[u8; IO_SIZE]) {
        let io = |reg: u8| io[usize::from(reg)];

        // KEY0 decides the CGB's mode, BANK whether the boot ROM is mapped.
        if self.model.is_cgb_hardware() {
            let dmg_mode = io(KEY0) & KEY0_DMG_MODE != 0;
            self.cgb_mode = if dmg_mode {
                CgbMode::Compat
            } else {
                CgbMode::Cgb
            };
            // Objects are drawn in coordinate order in DMG mode.
            self.ppu.write_opri(u8::from(dmg_mode));
        }
        if io(BANK) != 0 {
            self.bootrom.disable();
        }
        if matches!(self.cgb_mode, CgbMode::Cgb) {
            self.write_mem(io_addr(SVBK), io(SVBK));
            self.key1.write(io(KEY1));
            self.key1
                .set_double_speed(io(KEY1) & KEY1_DOUBLE_SPEED != 0);
        }

        // Timers, joypad and serial: DIV sets the system counter, SC does
        // not start a transfer.
        self.write_mem(io_addr(P1), io(P1));
        self.write_mem(io_addr(SB), io(SB));
        self.serial.set_sc(io(SC), self.cgb_mode);
        self.clock.div = u16::from(io(DIV)) << 8;
        // The system counter is running (as after the boot ROM).
        self.clock.div_state = 2;
        self.serial.set_master_clock(self.clock.div & 0x100 != 0);
        self.write_mem(io_addr(TIMA), io(TIMA));
        self.write_mem(io_addr(TMA), io(TMA));
        self.write_mem(io_addr(TAC), io(TAC));

        // The APU: NR52 first, as it powers the others; no channel is
        // triggered.
        self.write_mem(io_addr(NR52), io(NR52));
        for reg in APU_FIRST..NR52 {
            let value = if NRX4.contains(&reg) {
                io(reg) & !NRX4_TRIGGER
            } else {
                io(reg)
            };
            self.write_mem(io_addr(reg), value);
        }
        for reg in WAV_FIRST..=WAV_LAST {
            self.write_mem(io_addr(reg), io(reg));
        }

        // The PPU, restarted at the line LY was on. DMA does not start a
        // transfer.
        self.write_mem(io_addr(LCDC), io(LCDC));
        if io(LCDC) & 0x80 != 0 {
            self.ppu.set_position(io(LY).min(LAST_LINE), LY_READ_DOT);
        }
        for reg in [STAT, SCY, SCX, LYC, BGP, OBP0, OBP1, WX, WY] {
            self.write_mem(io_addr(reg), io(reg));
        }
        self.dma.set_reg(io(DMA));

        // The CGB's other registers (HDMA5 is left alone: it would start a
        // transfer).
        if self.model.is_cgb_hardware() {
            // (RP, the infrared port, is not emulated.)
            for reg in [VBK, HDMA1, HDMA2, HDMA3, HDMA4, BCPS, OCPS] {
                self.write_mem(io_addr(reg), io(reg));
            }
            for reg in [UNDOC_FF72, UNDOC_FF73, UNDOC_FF75] {
                self.write_mem(io_addr(reg), io(reg));
            }
        }
    }

    /// Copies a state's memories.
    fn load_memories(&mut self, layout: &Layout, data: &[u8]) {
        copy_memory(data, layout.get(Memory::Ram), self.wram.wram_mut());
        copy_memory(
            data,
            layout.get(Memory::Vram),
            self.ppu.vram_mut().bytes_mut(),
        );
        copy_memory(data, layout.get(Memory::MbcRam), self.cart.ram_mut());
        copy_memory(
            data,
            layout.get(Memory::Oam),
            self.ppu.oam_mut().bytes_mut(),
        );
        copy_memory(data, layout.get(Memory::Hram), self.hram.hram_mut());
        if self.model.is_cgb_hardware() {
            let palette = |memory| {
                let mut raw = [0; ColorPalette::RAW_SIZE];
                copy_memory(data, layout.get(memory), &mut raw);
                raw
            };
            self.ppu.bcp_mut().set_raw(&palette(Memory::BgPalette));
            self.ppu.ocp_mut().set_raw(&palette(Memory::ObjPalette));
        }
    }

    /// The RTC block: the real and latched registers (a byte and 3 bytes of
    /// padding each), then when the state was saved.
    fn load_rtc(&mut self, block: &[u8], secs_since_unix_epoch: u64) {
        let Some(rtc) = self.cart.rtc_mut() else {
            return;
        };
        let mut regs = [[0; 5]; 2];
        for (reg, &[value, ..]) in regs
            .as_flattened_mut()
            .iter_mut()
            .zip(block.as_chunks::<4>().0)
        {
            *reg = value;
        }
        let [real, latched] = regs;
        rtc.set_real(real);
        rtc.set_latched(latched);

        let mut timestamp = [0; 8];
        timestamp.copy_from_slice(&block[0x28..0x30]);
        // A timestamp from the future (a clock that went back) counts as none.
        let elapsed = secs_since_unix_epoch.saturating_sub(u64::from_le_bytes(timestamp));
        rtc.add_seconds(elapsed);
    }
}
