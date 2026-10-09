mod dma;
mod hdma;
mod hram;
mod key1;
mod svbk;
mod wram;

use crate::{AudioCallback, Model, ppu};
use crate::{CgbMode, Gb};
pub use dma::Dma;
pub use hdma::{Hdma, SwitchHdma};
pub use hram::Hram;
pub use key1::{Key1, SpeedSwitch};
pub use wram::Wram;

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
const NR14: u8 = 0x14;
const NR21: u8 = 0x16;
const NR24: u8 = 0x19;
const NR30: u8 = 0x1A;
const NR34: u8 = 0x1E;
const NR41: u8 = 0x20;
const NR44: u8 = 0x23;
const NR50: u8 = 0x24;
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
// Undocumented CGB registers
const UNDOC_FF72: u8 = 0x72;
const UNDOC_FF73: u8 = 0x73;
const UNDOC_FF75: u8 = 0x75;
// HRAM
const HRAM_BEG: u8 = 0x80;
const HRAM_END: u8 = 0xFE;
// IE
const IE: u8 = 0xFF;

impl<A: AudioCallback> Gb<A> {
    #[must_use]
    const fn are_cgb_regs_available(&self) -> bool {
        // The bootrom writes the color palettes for compatibility mode, so we must allow it to write to those registers,
        // since it's not modifiable by the user there should be no issues.
        matches!(self.cgb_mode, CgbMode::Cgb) || self.bootrom.is_enabled()
    }

    #[must_use]
    fn read_boot_or_cart(&self, addr: u16) -> u8 {
        self.bootrom.read(addr).unwrap_or_else(|| {
            #[cfg(feature = "game_genie")]
            {
                let data = self.cart.read_rom(addr);
                self.game_genie.query(addr, data).unwrap_or(data)
            }

            #[cfg(not(feature = "game_genie"))]
            {
                self.cart.read_rom(addr)
            }
        })
    }

    #[must_use]
    fn read_high(&self, addr: u8) -> u8 {
        match addr {
            P1 => self.joy.read_p1(),
            SB => self.serial.read_sb(),
            SC => self.serial.read_sc(),
            DIV => self.read_div(),
            TIMA => self.clock.tima(),
            TMA => self.clock.tma(),
            TAC => self.read_tac(),
            IF => self.ints.read_if(),
            NR10..=NR14
            | NR21..=NR24
            | NR30..=NR34
            | NR41..=NR44
            | NR50..=NR52
            | WAV_BEG..=WAV_END => self.apu.read(usize::from(addr)),
            LCDC => self.ppu.read_lcdc(),
            STAT => self.ppu.cpu_read_stat(),
            SCY => self.ppu.read_scy(),
            SCX => self.ppu.read_scx(),
            LY => self.ppu.cpu_read_ly(),
            LYC => self.ppu.read_lyc(),
            DMA => self.dma.read(),
            BGP => self.ppu.read_bgp(),
            OBP0 => self.ppu.read_obp0(),
            OBP1 => self.ppu.read_obp1(),
            WY => self.ppu.read_wy(),
            WX => self.ppu.read_wx(),
            KEY1 if matches!(self.cgb_mode, CgbMode::Cgb) => self.key1.read(),
            VBK if self.is_cgb() => self.ppu.vram().read_vbk(),
            HDMA5 if matches!(self.cgb_mode, CgbMode::Cgb) => self.hdma.read_hdma5(),
            BCPS if self.is_cgb() => self.ppu.bcp().spec(),
            BCPD if matches!(self.cgb_mode, CgbMode::Cgb) => {
                if self.ppu.is_cgb_palettes_accessible() {
                    self.ppu.bcp().data()
                } else {
                    0xFF
                }
            }
            OCPS if self.is_cgb() => self.ppu.ocp().spec(),
            OCPD if matches!(self.cgb_mode, CgbMode::Cgb) => {
                if self.ppu.is_cgb_palettes_accessible() {
                    self.ppu.ocp().data()
                } else {
                    0xFF
                }
            }
            OPRI if self.bootrom.is_enabled() => self.ppu.read_opri(),
            SVBK if matches!(self.cgb_mode, CgbMode::Cgb) => self.wram.svbk().read(),
            PCM12 if self.is_cgb() => self.apu.pcm12(),
            PCM34 if self.is_cgb() => self.apu.pcm34(),
            // Undocumented CGB registers. Per Pan Docs "FF72-FF73 — Bits 0-7
            // (CGB Mode only)": full R/W, init $00. "FF75 — Bits 4-6
            // (CGB Mode only)": bits 0-3 and 7 read as 1, bits 4-6 are R/W.
            UNDOC_FF72 if self.is_cgb() => self.undoc_ff72,
            UNDOC_FF73 if self.is_cgb() => self.undoc_ff73,
            UNDOC_FF75 if self.is_cgb() => (self.undoc_ff75 & 0x70) | 0x8F,
            HRAM_BEG..=HRAM_END => self.hram.read(addr),
            IE => self.ints.read_ie(),
            _ => 0xFF,
        }
    }

    /// CPU read: like `read_mem`, but OAM accesses can corrupt OAM (DMG).
    #[inline]
    pub fn cpu_read_mem(&mut self, addr: u16) -> u8 {
        if (0xFE00..=0xFEFF).contains(&addr) {
            let dma_blocked = self.dma.blocks_oam_read();
            return self.ppu.cpu_read_oam_area(addr, dma_blocked);
        }
        let value = self
            .dma_read_redirect(addr)
            .map_or(0xFF, |addr| self.read_mem(addr));
        self.dma_after_read(addr);
        value
    }

    #[must_use]
    #[inline]
    pub fn read_mem(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF => self.read_boot_or_cart(addr),
            0x0200..=0x08FF => {
                if self.model.is_cgb_hardware() {
                    self.read_boot_or_cart(addr)
                } else {
                    #[cfg(feature = "game_genie")]
                    {
                        let data = self.cart.read_rom(addr);
                        self.game_genie.query(addr, data).unwrap_or(data)
                    }

                    #[cfg(not(feature = "game_genie"))]
                    {
                        self.cart.read_rom(addr)
                    }
                }
            }
            0x0100..=0x01FF | 0x0900..=0x7FFF => {
                #[cfg(feature = "game_genie")]
                {
                    let data = self.cart.read_rom(addr);
                    self.game_genie.query(addr, data).unwrap_or(data)
                }

                #[cfg(not(feature = "game_genie"))]
                {
                    self.cart.read_rom(addr)
                }
            }
            0x8000..=0x9FFF => self.ppu.read_vram(addr),
            0xA000..=0xBFFF => self.cart.read_ram(addr),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.wram.read_wram_lo(addr),
            0xD000..=0xDFFF | 0xF000..=0xFDFF => self.wram.read_wram_hi(addr),
            0xFE00..=0xFE9F => {
                if self.dma.blocks_oam_read() {
                    0xFF
                } else {
                    self.ppu.read_oam(addr)
                }
            }
            0xFEA0..=0xFEFF => self.ppu.peek_unusable(addr),
            0xFF00..=0xFFFF => self.read_high((addr & 0xFF) as u8),
        }
    }

    #[expect(clippy::cognitive_complexity)]
    fn write_high(&mut self, addr: u8, val: u8) {
        match addr {
            P1 => self.joy.write_joy(val),
            SB => self.serial.write_sb(val),
            SC => self.serial.write_sc(val, &mut self.ints, self.cgb_mode),
            DIV => self.write_div(),
            TIMA => self.write_tima(val),
            TMA => self.write_tma(val),
            TAC => self.write_tac(val),
            IF => self.ints.write_if(val),
            NR10..=NR14
            | NR21..=NR24
            | NR30..=NR34
            | NR41..=NR44
            | NR50..=NR52
            | WAV_BEG..=WAV_END => {
                if addr == NR52 && val & 0x80 != 0 && !self.apu.is_enabled() {
                    self.apu_powered_on();
                }
                let ctx = self.apu_ctx();
                self.apu.write(&ctx, usize::from(addr), val);
            }
            LCDC => {
                let is_cgb = self.is_cgb();
                self.ppu.write_lcdc(val, &mut self.ints, is_cgb);
                if self.ppu.take_lcd_off_hdma_edge() {
                    self.hdma.lcd_off_edge();
                }
            }
            STAT => {
                let is_cgb = self.is_cgb();
                self.ppu.write_stat(val, &mut self.ints, is_cgb);
            }
            SCY => self.ppu.write_scy(val),
            SCX => self.ppu.write_scx(val),
            LYC => self.ppu.write_lyc(val, &mut self.ints),
            DMA => {
                self.dma.write(val);
                self.ppu.set_dma_state(0xFF, u16::from(val) << 8, true);
                self.ppu.refresh_stat(&mut self.ints);
            }
            BGP => self.ppu.write_bgp(val),
            OBP0 => self.ppu.write_obp0(val),
            OBP1 => self.ppu.write_obp1(val),
            WY => self.ppu.write_wy(val),
            WX => self.ppu.write_wx(val),
            KEY0 if matches!(
                self.model,
                Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD | Model::CgbE
            ) =>
            {
                // FIXME: causes broken palettes on GB games played on CGB
                // should we allow all cgb functions to be observable from a GB rom?
                if self.bootrom.is_enabled() && val == 4 {
                    self.cgb_mode = CgbMode::Compat;
                }
            }
            BANK => {
                if val & 1 != 0 {
                    self.bootrom.disable();
                }
            }
            VBK if matches!(self.cgb_mode, CgbMode::Cgb) => self.ppu.vram_mut().write_vbk(val),
            KEY1 if matches!(self.cgb_mode, CgbMode::Cgb) => self.key1.write(val),
            HDMA1 if matches!(self.cgb_mode, CgbMode::Cgb) => self.hdma.write_hdma1(val),
            HDMA2 if matches!(self.cgb_mode, CgbMode::Cgb) => self.hdma.write_hdma2(val),
            HDMA3 if matches!(self.cgb_mode, CgbMode::Cgb) => self.hdma.write_hdma3(val),
            HDMA4 if matches!(self.cgb_mode, CgbMode::Cgb) => self.hdma.write_hdma4(val),
            HDMA5 if matches!(self.cgb_mode, CgbMode::Cgb) => {
                let in_hblank = self.ppu.gstat_hdma_enable_in_hblank().unwrap_or_else(|| {
                    matches!(self.ppu.mode(), ppu::Mode::HBlank) && !self.ppu.at_oam_scan_edge()
                });
                self.hdma.write_hdma5(val, in_hblank);
            }
            BCPS if self.is_cgb() => self.ppu.bcp_mut().set_spec(val),
            BCPD if self.are_cgb_regs_available() => {
                if self.ppu.is_cgb_palettes_accessible() {
                    self.ppu.bcp_mut().set_data(val);
                } else {
                    self.ppu.bcp_mut().auto_increment();
                }
            }
            OCPS if self.is_cgb() => self.ppu.ocp_mut().set_spec(val),
            OCPD if self.are_cgb_regs_available() => {
                if self.ppu.is_cgb_palettes_accessible() {
                    self.ppu.ocp_mut().set_data(val);
                } else {
                    self.ppu.ocp_mut().auto_increment();
                }
            }
            OPRI if self.is_cgb() => {
                // FIXME: understand behaviour outside of bootrom
                if self.bootrom.is_enabled() {
                    self.ppu.write_opri(val);
                }
            }
            SVBK if matches!(self.cgb_mode, CgbMode::Cgb) => self.wram.svbk_mut().write(val),
            // Undocumented CGB registers. Per Pan Docs: FF72/FF73 are full
            // R/W (any bit can be written), FF75 only bits 4-6 are writable
            // (bits 0-3, 7 always read as 1).
            UNDOC_FF72 if self.is_cgb() => self.undoc_ff72 = val,
            UNDOC_FF73 if self.is_cgb() => self.undoc_ff73 = val,
            UNDOC_FF75 if self.is_cgb() => self.undoc_ff75 = val & 0x70,
            HRAM_BEG..=HRAM_END => self.hram.write(addr, val),
            IE => self.ints.write_ie(val),
            _ => (),
        }
    }

    #[inline]
    pub fn write_mem(&mut self, addr: u16, val: u8) {
        let Some(addr) = (if addr < 0xFE00 {
            self.dma_write_redirect(addr, val)
        } else {
            Some(addr)
        }) else {
            return;
        };
        match addr {
            // FIXME: we assume bootrom doesn't write to rom
            0x0000..=0x7FFF => self.cart.write_rom(addr, val),
            0x8000..=0x9FFF => self.ppu.write_vram(addr, val),
            0xA000..=0xBFFF => self.cart.write_ram(addr, val),
            0xC000..=0xCFFF | 0xE000..=0xEFFF => self.wram.write_wram_lo(addr, val),
            0xD000..=0xDFFF | 0xF000..=0xFDFF => self.wram.write_wram_hi(addr, val),
            0xFE00..=0xFEFF => {
                let dma_blocked = self.dma.blocks_oam_write();
                self.ppu.cpu_write_oam_area(addr, val, dma_blocked);
            }
            0xFF00..=0xFFFF => self.write_high((addr & 0xFF) as u8, val),
        }
    }
}
