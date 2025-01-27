use crate::{ppu::Mode, CgbMode, Gb, Model::Cgb};
use crate::{AudioCallback, Model};

#[derive(Default, Debug)]
pub enum HdmaState {
    #[default]
    Sleep,
    WaitHBlank,
    HBlankDone,
    General,
}

// IO addresses
// JoyP
const P1: u8 = 0x00;
// Serial
const SB: u8 = 0x01;
const SC: u8 = 0x02;
// Timer
const DIV: u8 = 0x04;
const TIMA: u8 = 0x05;
const TMA: u8 = 0x06;
const TAC: u8 = 0x07;
// IF
const IF: u8 = 0x0F;
// APU
const NR10: u8 = 0x10;
const NR11: u8 = 0x11;
const NR12: u8 = 0x12;
const NR13: u8 = 0x13;
const NR14: u8 = 0x14;
const NR21: u8 = 0x16;
const NR22: u8 = 0x17;
const NR23: u8 = 0x18;
const NR24: u8 = 0x19;
const NR30: u8 = 0x1A;
const NR31: u8 = 0x1B;
const NR32: u8 = 0x1C;
const NR33: u8 = 0x1D;
const NR34: u8 = 0x1E;
const NR41: u8 = 0x20;
const NR42: u8 = 0x21;
const NR43: u8 = 0x22;
const NR44: u8 = 0x23;
const NR50: u8 = 0x24;
const NR51: u8 = 0x25;
const NR52: u8 = 0x26;
const WAV_BEG: u8 = 0x30;
const WAV_END: u8 = 0x3F;
// PPU
const LCDC: u8 = 0x40;
const STAT: u8 = 0x41;
const SCY: u8 = 0x42;
const SCX: u8 = 0x43;
const LY: u8 = 0x44;
const LYC: u8 = 0x45;
const DMA: u8 = 0x46;
const BGP: u8 = 0x47;
const OBP0: u8 = 0x48;
const OBP1: u8 = 0x49;
const BANK: u8 = 0x50;
const WY: u8 = 0x4A;
const WX: u8 = 0x4B;
const KEY0: u8 = 0x4C;
const KEY1: u8 = 0x4D;
const VBK: u8 = 0x4F;
// HDMA
const HDMA1: u8 = 0x51;
const HDMA2: u8 = 0x52;
const HDMA3: u8 = 0x53;
const HDMA4: u8 = 0x54;
const HDMA5: u8 = 0x55;
// Palettes
const BCPS: u8 = 0x68;
const BCPD: u8 = 0x69;
const OCPS: u8 = 0x6A;
const OCPD: u8 = 0x6B;
const OPRI: u8 = 0x6C;
// WRAM select
const SVBK: u8 = 0x70;
// APU digital out
const PCM12: u8 = 0x76;
const PCM34: u8 = 0x77;
// HRAM
const HRAM_BEG: u8 = 0x80;
const HRAM_END: u8 = 0xFE;
// IE
const IE: u8 = 0xFF;

impl<A: AudioCallback> Gb<A> {
    #[must_use]
    #[inline]
    pub(crate) const fn read_wram_lo(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xFFF) as usize]
    }

    #[inline]
    fn write_wram_lo(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xFFF) as usize] = val;
    }

    #[must_use]
    #[inline]
    pub(crate) const fn read_wram_hi(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xFFF | self.svbk.bank_offset()) as usize]
    }

    #[inline]
    fn write_wram_hi(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xFFF | self.svbk.bank_offset()) as usize] = val;
    }

    #[must_use]
    #[inline]
    const fn dma_active(&self) -> bool {
        self.dma_on && (self.dma_cycles > 0 || self.dma_restarting)
    }

    #[must_use]
    #[inline]
    const fn hdma_on(&self) -> bool {
        !matches!(self.hdma_state, HdmaState::Sleep)
    }

    #[must_use]
    #[inline]
    const fn read_boot_or_cart(&self, addr: u16) -> u8 {
        if let Some(bootrom) = self.bootrom {
            // TODO: as long as the bootrom is correct should be in bounds
            bootrom[addr as usize]
        } else {
            self.cart.read_rom(addr)
        }
    }

    // **************
    // * Memory map *
    // **************

    #[must_use]
    pub(crate) fn read_mem(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF => self.read_boot_or_cart(addr),
            0x0200..=0x08FF => {
                if matches!(self.model, Model::Cgb) {
                    self.read_boot_or_cart(addr)
                } else {
                    self.cart.read_rom(addr)
                }
            }
            0x0100..=0x01FF | 0x0900..=0x7FFF => self.cart.read_rom(addr),
            0x8000..=0x9FFF => self.ppu.read_vram(addr),
            0xA000..=0xBFFF => self.cart.read_ram(addr),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.read_wram_lo(addr),
            0xD000..=0xDFFF | 0xF000..=0xFDFF => self.read_wram_hi(addr),
            0xFE00..=0xFE9F => self.ppu.read_oam(addr, self.dma_on),
            0xFEA0..=0xFEFF => 0xFF,
            0xFF00..=0xFFFF => self.read_high((addr & 0xFF) as u8),
        }
    }

    #[inline]
    pub(crate) fn write_mem(&mut self, addr: u16, val: u8) {
        match addr {
            // FIXME: we assume bootrom doesn't write to rom
            0x0000..=0x7FFF => self.cart.write_rom(addr, val),
            0x8000..=0x9FFF => self.ppu.write_vram(addr, val),
            0xA000..=0xBFFF => self.cart.write_ram(addr, val),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.write_wram_lo(addr, val),
            0xD000..=0xDFFF | 0xF000..=0xFDFF => self.write_wram_hi(addr, val),
            0xFE00..=0xFE9F => self.ppu.write_oam(addr, val, self.dma_active()),
            0xFEA0..=0xFEFF => (),
            0xFF00..=0xFFFF => self.write_high((addr & 0xFF) as u8, val),
        }
    }

    #[must_use]
    #[inline]
    fn read_high(&self, addr: u8) -> u8 {
        match addr {
            P1 => self.joy.read_p1(),
            SB => self.serial.read_sb(),
            SC => self.serial.read_sc(),
            DIV => self.read_div(),
            TIMA => self.tima,
            TMA => self.tma,
            TAC => self.read_tac(),
            IF => self.ints.read_if(),
            NR10 => self.apu.read_nr10(),
            NR11 => self.apu.read_nr11(),
            NR12 => self.apu.read_nr12(),
            NR14 => self.apu.read_nr14(),
            NR21 => self.apu.read_nr21(),
            NR22 => self.apu.read_nr22(),
            NR24 => self.apu.read_nr24(),
            NR30 => self.apu.read_nr30(),
            NR32 => self.apu.read_nr32(),
            NR34 => self.apu.read_nr34(),
            NR42 => self.apu.read_nr42(),
            NR43 => self.apu.read_nr43(),
            NR44 => self.apu.read_nr44(),
            NR50 => self.apu.read_nr50(),
            NR51 => self.apu.read_nr51(),
            NR52 => self.apu.read_nr52(),
            WAV_BEG..=WAV_END => self.apu.read_wave_ram(addr),
            LCDC => self.ppu.read_lcdc(),
            STAT => self.ppu.read_stat(),
            SCY => self.ppu.read_scy(),
            SCX => self.ppu.read_scx(),
            LY => self.ppu.read_ly(),
            LYC => self.ppu.read_lyc(),
            DMA => self.dma,
            BGP => self.ppu.read_bgp(),
            OBP0 => self.ppu.read_obp0(),
            OBP1 => self.ppu.read_obp1(),
            WY => self.ppu.read_wy(),
            WX => self.ppu.read_wx(),
            KEY1 if matches!(self.cgb_mode, CgbMode::Cgb) => self.key1.read(),
            VBK if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.read_vbk(),
            HDMA5 if matches!(self.cgb_mode, CgbMode::Cgb) => self.read_hdma5(),
            BCPS if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.bcp().spec(),
            BCPD if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.bcp().data(),
            OCPS if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.ocp().spec(),
            OCPD if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.ocp().data(),
            OPRI if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.read_opri(),
            SVBK if matches!(self.cgb_mode, CgbMode::Cgb) => self.svbk.read(),
            PCM12 if matches!(self.cgb_mode, CgbMode::Cgb) => self.apu.pcm12(),
            PCM34 if matches!(self.cgb_mode, CgbMode::Cgb) => self.apu.pcm34(),
            HRAM_BEG..=HRAM_END => self.hram[(addr & 0x7F) as usize],
            IE => self.ints.read_ie(),
            _ => 0xFF,
        }
    }

    #[allow(clippy::cognitive_complexity)]
    #[inline]
    fn write_high(&mut self, addr: u8, val: u8) {
        match addr {
            P1 => self.joy.write_joy(val),
            SB => self.serial.write_sb(val),
            SC => self.serial.write_sc(val, &mut self.ints, &self.cgb_mode),
            DIV => self.write_div(),
            TIMA => self.write_tima(val),
            TMA => self.write_tma(val),
            TAC => self.write_tac(val),
            IF => self.ints.write_if(val),
            NR10 if self.apu.enabled() => self.apu.write_nr10(val),
            NR11 if self.apu.enabled() => self.apu.write_nr11(val),
            NR12 if self.apu.enabled() => self.apu.write_nr12(val),
            NR13 if self.apu.enabled() => self.apu.write_nr13(val),
            NR14 if self.apu.enabled() => self.apu.write_nr14(val),
            NR21 if self.apu.enabled() => self.apu.write_nr21(val),
            NR22 if self.apu.enabled() => self.apu.write_nr22(val),
            NR23 if self.apu.enabled() => self.apu.write_nr23(val),
            NR24 if self.apu.enabled() => self.apu.write_nr24(val),
            NR30 if self.apu.enabled() => self.apu.write_nr30(val),
            NR31 if self.apu.enabled() => self.apu.write_nr31(val),
            NR32 if self.apu.enabled() => self.apu.write_nr32(val),
            NR33 if self.apu.enabled() => self.apu.write_nr33(val),
            NR34 if self.apu.enabled() => self.apu.write_nr34(val),
            NR41 if self.apu.enabled() => self.apu.write_nr41(val),
            NR42 if self.apu.enabled() => self.apu.write_nr42(val),
            NR43 if self.apu.enabled() => self.apu.write_nr43(val),
            NR44 if self.apu.enabled() => self.apu.write_nr44(val),
            NR50 => self.apu.write_nr50(val),
            NR51 => self.apu.write_nr51(val),
            NR52 => self.apu.write_nr52(val),
            WAV_BEG..=WAV_END => self.apu.write_wave_ram(addr, val),
            LCDC => self.ppu.write_lcdc(val, &mut self.ints),
            STAT => self.ppu.write_stat(val),
            SCY => self.ppu.write_scy(val),
            SCX => self.ppu.write_scx(val),
            LYC => self.ppu.write_lyc(val),
            DMA => self.write_dma(val),
            BGP => self.ppu.write_bgp(val),
            OBP0 => self.ppu.write_obp0(val),
            OBP1 => self.ppu.write_obp1(val),
            WY => self.ppu.write_wy(val),
            WX => self.ppu.write_wx(val),
            KEY0 if matches!(self.model, Cgb) => {
                if self.bootrom.is_some() && val == 4 {
                    self.cgb_mode = CgbMode::Compat;
                }
            }
            KEY1 if matches!(self.cgb_mode, CgbMode::Cgb) => self.key1.write(val),
            VBK if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.write_vbk(val),
            BANK => {
                if val & 1 != 0 {
                    self.bootrom = None;
                }
            }
            HDMA1 if matches!(self.cgb_mode, CgbMode::Cgb) => self.write_hdma1(val),
            HDMA2 if matches!(self.cgb_mode, CgbMode::Cgb) => self.write_hdma2(val),
            HDMA3 if matches!(self.cgb_mode, CgbMode::Cgb) => self.write_hdma3(val),
            HDMA4 if matches!(self.cgb_mode, CgbMode::Cgb) => self.write_hdma4(val),
            HDMA5 if matches!(self.cgb_mode, CgbMode::Cgb) => self.write_hdma5(val),
            BCPS if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.bcp_mut().set_spec(val),
            BCPD if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.bcp_mut().set_data(val),
            OCPS if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.ocp_mut().set_spec(val),
            OCPD if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.ocp_mut().set_data(val),
            OPRI if matches!(self.model, Model::Cgb) => {
                // FIXME: understand behaviour outside of bootrom
                if self.bootrom.is_some() {
                    self.ppu.write_opri(val);
                }
            }
            SVBK if matches!(self.cgb_mode, CgbMode::Cgb) => self.svbk.write(val),
            HRAM_BEG..=HRAM_END => self.hram[(addr & 0x7F) as usize] = val,
            IE => self.ints.write_ie(val),
            _ => (),
        }
    }

    // *******
    // * DMA *
    // *******

    #[inline]
    fn write_dma(&mut self, val: u8) {
        if self.dma_on {
            self.dma_restarting = true;
        }

        self.dma_cycles = -8; // two m-cycles delay
        self.dma = val;
        self.dma_addr = u16::from(val) << 8;
        self.dma_on = true;
    }

    #[inline]
    pub(crate) fn run_dma(&mut self) {
        if !self.dma_on {
            return;
        }

        while self.dma_cycles >= 4 {
            self.dma_cycles -= 4;

            // TODO: reading some ranges should cause problems, $DF is
            // the maximum value accesible to OAM DMA (probably reads
            // from echo RAM should work too, RESEARCH).
            // what happens if reading from IO range? (garbage? 0xff?)
            let val = self.read_mem(self.dma_addr);

            // TODO: writes from DMA can access OAM on modes 2 and 3
            // with some glitches (RESEARCH) and without trouble during
            // VBLANK (what happens in HBLANK?)
            self.ppu.write_oam_by_dma(self.dma_addr, val);

            self.dma_addr = self.dma_addr.wrapping_add(1);
            if self.dma_addr & 0xFF > 0x9F {
                self.dma_on = false;
                self.dma_restarting = false;
            }
        }
    }

    #[inline]
    fn write_hdma1(&mut self, val: u8) {
        self.hdma_src = (u16::from(val) << 8) | (self.hdma_src & 0xF0);
    }

    #[inline]
    fn write_hdma2(&mut self, val: u8) {
        self.hdma_src = (self.hdma_src & 0xFF00) | u16::from(val & 0xF0);
    }

    #[inline]
    fn write_hdma3(&mut self, val: u8) {
        self.hdma_dst = (u16::from(val & 0x1F) << 8) | (self.hdma_dst & 0xF0);
    }

    #[inline]
    fn write_hdma4(&mut self, val: u8) {
        self.hdma_dst = (self.hdma_dst & 0x1F00) | u16::from(val & 0xF0);
    }

    #[must_use]
    #[inline]
    const fn read_hdma5(&self) -> u8 {
        // active on low
        (!self.hdma_on() as u8) << 7 | self.hdma5
    }

    #[inline]
    fn write_hdma5(&mut self, val: u8) {
        use HdmaState::{General, Sleep, WaitHBlank};

        debug_assert!(!matches!(self.hdma_state, HdmaState::General));

        // stop current transfer
        if self.hdma_on() && val & 0x80 == 0 {
            self.hdma_state = Sleep;
            return;
        }

        self.hdma5 = val & 0x7F;
        self.hdma_len = (u16::from(self.hdma5) + 1) * 0x10;
        self.hdma_state = if val & 0x80 == 0 { General } else { WaitHBlank };
    }

    #[inline]
    pub(crate) fn run_hdma(&mut self) {
        use HdmaState::{General, HBlankDone, Sleep, WaitHBlank};

        match self.hdma_state {
            General => (),
            WaitHBlank if matches!(self.ppu.mode(), Mode::HBlank) => (),
            HBlankDone if !matches!(self.ppu.mode(), Mode::HBlank) => {
                self.hdma_state = WaitHBlank;
                return;
            }
            _ => return,
        }

        let len = if matches!(self.hdma_state, WaitHBlank) {
            self.hdma_len -= 0x10;
            self.hdma_state = if self.hdma_len == 0 {
                Sleep
            } else {
                HBlankDone
            };
            self.hdma5 = ((self.hdma_len / 0x10).wrapping_sub(1) & 0xFF) as u8;
            0x10
        } else {
            self.hdma_state = Sleep;
            self.hdma5 = 0xFF;
            let len = self.hdma_len;
            self.hdma_len = 0;
            len
        };

        for _ in 0..len {
            // TODO: the same problems as normal DMA plus reading from
            // VRAM should copy garbage
            let val = self.read_mem(self.hdma_src);
            self.ppu.write_vram(self.hdma_dst, val);
            self.hdma_dst += 1;
            self.hdma_src += 1;
        }

        // can be outside of loop because HDMA should not
        // access IO range (clk registers, ifr,
        // etc..). If the PPU reads VRAM during an HDMA transfer it
        // should be glitchy anyways
        // FIXME: timings
        if self.key1.enabled() {
            self.advance_t_cycles(i32::from(len) * 2 * 2);
        } else {
            self.advance_t_cycles(i32::from(len) * 2);
        }
    }
}

#[derive(Default, Debug)]
pub struct Svbk {
    svbk: u8,
}

impl Svbk {
    #[must_use]
    #[inline]
    pub const fn read(&self) -> u8 {
        self.svbk | 0xF8
    }

    #[inline]
    pub fn write(&mut self, val: u8) {
        self.svbk = val & 7;
    }

    #[must_use]
    #[inline]
    pub const fn bank_offset(&self) -> u16 {
        // Return value between 0x1000 and 0x7000
        (if self.svbk == 0 { 1 } else { self.svbk } as u16) * 0x1000
    }
}

#[derive(Default, Debug)]
pub struct Key1 {
    key1: u8,
}

impl Key1 {
    #[must_use]
    #[inline]
    pub const fn read(&self) -> u8 {
        self.key1 | 0x7E
    }

    #[inline]
    pub fn write(&mut self, val: u8) {
        self.key1 = self.key1 & 0x80 | val & 1;
    }

    #[must_use]
    #[inline]
    pub const fn enabled(&self) -> bool {
        self.key1 & 0x80 != 0
    }

    #[must_use]
    #[inline]
    pub const fn requested(&self) -> bool {
        self.key1 & 1 != 0
    }

    #[inline]
    pub fn change_speed(&mut self) {
        debug_assert!(self.requested());
        self.key1 = !self.key1;
    }
}
