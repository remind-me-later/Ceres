use crate::{CompatMode, Gb, Model::Cgb, KEY1_SWITCH_B};

#[derive(PartialEq, Eq)]
pub enum HdmaState {
    Sleep,
    HBlank,
    HBlankDone,
    General,
}

// IO addresses
const P1: u8 = 0x00;
const SB: u8 = 0x01;
const SC: u8 = 0x02;
const DIV: u8 = 0x04;
const TIMA: u8 = 0x05;
const TMA: u8 = 0x06;
const TAC: u8 = 0x07;
const IF: u8 = 0x0F;
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
const WAV_BEGIN: u8 = 0x30;
const WAV_END: u8 = 0x3F;
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
const WY: u8 = 0x4A;
const WX: u8 = 0x4B;
const KEY0: u8 = 0x4C;
const KEY1: u8 = 0x4D;
const VBK: u8 = 0x4F;
const HDMA1: u8 = 0x51;
const HDMA2: u8 = 0x52;
const HDMA3: u8 = 0x53;
const HDMA4: u8 = 0x54;
const HDMA5: u8 = 0x55;
const BCPS: u8 = 0x68;
const BCPD: u8 = 0x69;
const OCPS: u8 = 0x6A;
const OCPD: u8 = 0x6B;
const OPRI: u8 = 0x6C;
const SVBK: u8 = 0x70;
const HRAM_BEG: u8 = 0x80;
const HRAM_END: u8 = 0xFE;
const IE: u8 = 0xFF;

impl Gb {
    #[inline]
    #[must_use]
    pub(crate) fn read_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xFFF) as usize]
    }

    #[inline]
    #[must_use]
    pub(crate) fn read_bank_ram(&self, addr: u16) -> u8 {
        let bank = self.svbk_true as u16 * 0x1000;
        self.wram[(addr & 0xFFF | bank) as usize]
    }

    #[inline]
    fn write_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xFFF) as usize] = val;
    }

    #[inline]
    fn write_bank_ram(&mut self, addr: u16, val: u8) {
        let bank = self.svbk_true as u16 * 0x1000;
        self.wram[(addr & 0xFFF | bank) as usize] = val;
    }

    #[inline]
    fn dma_active(&self) -> bool {
        self.dma_on && (self.dma_cycles > 0 || self.dma_restarting)
    }

    #[inline]
    fn hdma_on(&self) -> bool {
        !matches!(self.hdma_state, HdmaState::Sleep)
    }

    #[inline]
    fn read_rom_or_cart(&mut self, addr: u16) -> u8 {
        if self.boot_rom_mapped {
            return self.boot_rom[addr as usize];
        }
        self.cart.read_rom(addr)
    }

    pub(crate) fn read_mem(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF | 0x0200..=0x08FF => self.read_rom_or_cart(addr),
            0x0100..=0x01FF | 0x0900..=0x7FFF => self.cart.read_rom(addr),
            0x8000..=0x9FFF => self.read_vram(addr),
            0xA000..=0xBFFF => self.cart.read_ram(addr),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.read_ram(addr),
            0xD000..=0xDFFF | 0xF000..=0xFDFF => self.read_bank_ram(addr),
            0xFE00..=0xFE9F => self.read_oam(addr),
            0xFEA0..=0xFEFF => 0xFF,
            0xFF00..=0xFFFF => self.read_high((addr & 0xFF) as u8),
        }
    }

    #[inline]
    fn read_high(&mut self, addr: u8) -> u8 {
        match addr {
            P1 => self.read_p1(),
            SB => self.sb,
            SC => self.sc | 0x7E,
            DIV => ((self.clk_wide >> 6) & 0xFF) as u8,
            TIMA => self.tima,
            TMA => self.tma,
            TAC => 0xF8 | self.tac,
            IF => self.ifr | 0xE0,
            NR10 => self.apu_ch1.read_nr10(),
            NR11 => self.apu_ch1.read_nr11(),
            NR12 => self.apu_ch1.read_nr12(),
            NR14 => self.apu_ch1.read_nr14(),
            NR21 => self.apu_ch2.read_nr21(),
            NR22 => self.apu_ch2.read_nr22(),
            NR24 => self.apu_ch2.read_nr24(),
            NR30 => self.apu_ch3.read_nr30(),
            NR32 => self.apu_ch3.read_nr32(),
            NR34 => self.apu_ch3.read_nr34(),
            NR42 => self.apu_ch4.read_nr42(),
            NR43 => self.apu_ch4.read_nr43(),
            NR44 => self.apu_ch4.read_nr44(),
            NR50 => self.read_nr50(),
            NR51 => self.nr51,
            NR52 => self.read_nr52(),
            WAV_BEGIN..=WAV_END => self.apu_ch3.read_wave_ram(addr),
            LCDC => self.lcdc,
            STAT => self.stat | 0x80,
            SCY => self.scy,
            SCX => self.scx,
            LY => self.ly,
            LYC => self.lyc,
            DMA => self.dma,
            BGP => self.bgp,
            OBP0 => self.obp0,
            OBP1 => self.obp1,
            WY => self.wy,
            WX => self.wx,
            KEY1 if self.model == Cgb => 0x7E | self.key1,
            VBK if self.model == Cgb => self.vbk | 0xFE,
            HDMA5 if self.model == Cgb => {
                // active on low
                ((!self.hdma_on()) as u8) << 7 | self.hdma5
            }
            BCPS if self.model == Cgb => self.bcp.spec(),
            BCPD if self.model == Cgb => self.bcp.data(),
            OCPS if self.model == Cgb => self.ocp.spec(),
            OCPD if self.model == Cgb => self.ocp.data(),
            OPRI if self.model == Cgb => self.opri,
            SVBK if self.model == Cgb => self.svbk | 0xF8,
            HRAM_BEG..=HRAM_END => self.hram[(addr & 0x7F) as usize],
            IE => self.ie,
            _ => 0xFF,
        }
    }

    pub(crate) fn write_mem(&mut self, addr: u16, val: u8) {
        match addr {
            // assume bootrom doesn't write to rom
            0x0000..=0x08FF | 0x0900..=0x7FFF => self.cart.write_rom(addr, val),
            0x8000..=0x9FFF => self.write_vram(addr, val),
            0xA000..=0xBFFF => self.cart.write_ram(addr, val),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.write_ram(addr, val),
            0xD000..=0xDFFF | 0xF000..=0xFDFF => self.write_bank_ram(addr, val),
            0xFE00..=0xFE9F => {
                self.write_oam(addr, val, self.dma_active());
            }
            0xFEA0..=0xFEFF => (),
            0xFF00..=0xFFFF => self.write_high((addr & 0xFF) as u8, val),
        }
    }

    #[inline]
    fn write_high(&mut self, addr: u8, val: u8) {
        match addr {
            P1 => self.write_joy(val),
            SB => self.sb = val,
            SC => self.sc = val,
            DIV => self.write_div(),
            TIMA => self.write_tima(val),
            TMA => self.write_tma(val),
            TAC => self.write_tac(val),
            IF => self.ifr = val & 0x1F,
            NR10 if self.apu_on => self.apu_ch1.write_nr10(val),
            NR11 if self.apu_on => self.apu_ch1.write_nr11(val),
            NR12 if self.apu_on => self.apu_ch1.write_nr12(val),
            NR13 if self.apu_on => self.apu_ch1.write_nr13(val),
            NR14 if self.apu_on => self.apu_ch1.write_nr14(val),
            NR21 if self.apu_on => self.apu_ch2.write_nr21(val),
            NR22 if self.apu_on => self.apu_ch2.write_nr22(val),
            NR23 if self.apu_on => self.apu_ch2.write_nr23(val),
            NR24 if self.apu_on => self.apu_ch2.write_nr24(val),
            NR30 if self.apu_on => self.apu_ch3.write_nr30(val),
            NR31 if self.apu_on => self.apu_ch3.write_nr31(val),
            NR32 if self.apu_on => self.apu_ch3.write_nr32(val),
            NR33 if self.apu_on => self.apu_ch3.write_nr33(val),
            NR34 if self.apu_on => self.apu_ch3.write_nr34(val),
            NR41 if self.apu_on => self.apu_ch4.write_nr41(val),
            NR42 if self.apu_on => self.apu_ch4.write_nr42(val),
            NR43 if self.apu_on => self.apu_ch4.write_nr43(val),
            NR44 if self.apu_on => self.apu_ch4.write_nr44(val),
            NR50 => self.write_nr50(val),
            NR51 => self.write_nr51(val),
            NR52 => self.write_nr52(val),
            WAV_BEGIN..=WAV_END => self.apu_ch3.write_wave_ram(addr, val),
            LCDC => self.write_lcdc(val),
            STAT => self.write_stat(val),
            SCY => self.scy = val,
            SCX => self.scx = val,
            LYC => self.lyc = val,
            DMA => {
                if self.dma_on {
                    self.dma_restarting = true;
                }

                self.dma_cycles = -8; // two m-cycles delay
                self.dma = val;
                self.dma_addr = (val as u16) << 8;
                self.dma_on = true;
            }
            BGP => self.bgp = val,
            OBP0 => self.obp0 = val,
            OBP1 => self.obp1 = val,
            WY => self.wy = val,
            WX => self.wx = val,
            KEY0 if self.model == Cgb && self.boot_rom_mapped && val == 4 => {
                self.compat_mode = CompatMode::Compat;
            }
            KEY1 if self.model == Cgb => {
                self.key1 &= KEY1_SWITCH_B;
                self.key1 |= val & KEY1_SWITCH_B;
            }
            VBK if self.model == Cgb => self.vbk = val & 1,
            0x50 => {
                if val & 1 != 0 {
                    self.boot_rom_mapped = false;
                }
            }
            HDMA1 if self.model == Cgb => {
                self.hdma_src = (val as u16) << 8 | self.hdma_src & 0xF0;
            }
            HDMA2 if self.model == Cgb => {
                self.hdma_src = self.hdma_src & 0xFF00 | (val as u16) & 0xF0;
            }
            HDMA3 if self.model == Cgb => {
                self.hdma_dst = ((val & 0x1F) as u16) << 8 | self.hdma_dst & 0xF0;
            }
            HDMA4 if self.model == Cgb => {
                self.hdma_dst = self.hdma_dst & 0x1F00 | (val as u16) & 0xF0;
            }
            HDMA5 if self.model == Cgb => {
                // stop current transfer
                if self.hdma_on() && val & 0x80 == 0 {
                    self.hdma_state = HdmaState::Sleep;
                    return;
                }

                self.hdma5 = val & !0x80;
                let transfer_blocks = val & 0x7F;
                self.hdma_len = (transfer_blocks as u16 + 1) * 0x10;
                self.hdma_state = if val & 0x80 == 0 {
                    HdmaState::General
                } else {
                    HdmaState::HBlank
                };
            }
            BCPS if self.model == Cgb => self.bcp.set_spec(val),
            BCPD if self.model == Cgb => self.bcp.set_data(val),
            OCPS if self.model == Cgb => self.ocp.set_spec(val),
            OCPD if self.model == Cgb => self.ocp.set_data(val),
            OPRI if self.model == Cgb => self.opri = val,
            SVBK if self.model == Cgb => {
                let tmp = val & 7;
                self.svbk = tmp;
                self.svbk_true = if tmp == 0 { 1 } else { tmp };
            }
            HRAM_BEG..=HRAM_END => self.hram[(addr & 0x7F) as usize] = val,
            IE => self.ie = val,
            _ => (),
        }
    }
}
