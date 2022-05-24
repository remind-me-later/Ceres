use {
    crate::{ppu::Mode, FunctionMode, Gb, Model::Cgb, KEY1_SWITCH_B},
    core::intrinsics::unlikely,
};

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

const WAV_BEGIN: u8 = 0x30;
const WAV_END: u8 = 0x3f;

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
const WY: u8 = 0x4a;
const WX: u8 = 0x4b;

const KEY0: u8 = 0x4c;
const KEY1: u8 = 0x4d;
const VBK: u8 = 0x4f;

const OPRI: u8 = 0x6c;

const IF: u8 = 0x0f;
const IE: u8 = 0xff;

const HDMA1: u8 = 0x51;
const HDMA2: u8 = 0x52;
const HDMA3: u8 = 0x53;
const HDMA4: u8 = 0x54;
const HDMA5: u8 = 0x55;

const NR51: u8 = 0x25;

impl Gb {
    #[must_use]
    pub(crate) fn read_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xfff) as usize]
    }

    pub(crate) fn write_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xfff) as usize] = val;
    }

    #[must_use]
    pub(crate) fn read_bank_ram(&self, addr: u16) -> u8 {
        self.wram[(addr & 0xfff | (self.svbk_true as u16 * 0x1000)) as usize]
    }

    pub(crate) fn write_bank_ram(&mut self, addr: u16, val: u8) {
        self.wram[(addr & 0xfff | (self.svbk_true as u16 * 0x1000)) as usize] = val;
    }

    pub(crate) fn write_dma(&mut self, val: u8) {
        if self.dma_on {
            self.dma_restarting = true;
        }

        self.dma_cycles = -8; // two m-cycles delay
        self.dma = val;
        self.dma_addr = u16::from(val) << 8;
        self.dma_on = true;
    }

    // FIXME: sprites are not displayed during OAM DMA
    pub(crate) fn tick_dma(&mut self) {
        if !self.dma_on {
            return;
        }

        self.dma_cycles += 4;

        if self.dma_cycles < 4 {
            return;
        }

        self.dma_cycles -= 4;
        let src = self.dma_addr;

        let val = match src >> 8 {
            0x00..=0x7f => self.cart.read_rom(src),
            0x80..=0x9f => self.read_vram(src),
            0xa0..=0xbf => self.cart.read_ram(src),
            0xc0..=0xcf | 0xe0..=0xef => self.read_ram(src),
            0xd0..=0xdf | 0xf0..=0xff => self.read_bank_ram(src),
            _ => panic!("Illegal source addr for OAM DMA transfer"),
        };

        // mode doesn't matter writes from DMA can always access OAM
        self.oam[((src & 0xff) as u8) as usize] = val;

        self.dma_addr = self.dma_addr.wrapping_add(1);
        if self.dma_addr & 0xff >= 0xa0 {
            self.dma_on = false;
            self.dma_restarting = false;
        }
    }

    pub(crate) fn dma_active(&self) -> bool {
        self.dma_on && (self.dma_cycles > 0 || self.dma_restarting)
    }

    pub(crate) fn write_hdma1(&mut self, val: u8) {
        self.hdma_src = u16::from(val) << 8 | self.hdma_src & 0xf0;
    }

    pub(crate) fn write_hdma2(&mut self, val: u8) {
        self.hdma_src = self.hdma_src & 0xff00 | u16::from(val) & 0xf0;
    }

    pub(crate) fn write_hdma3(&mut self, val: u8) {
        self.hdma_dst = u16::from(val & 0x1f) << 8 | self.hdma_dst & 0xf0;
    }

    pub(crate) fn write_hdma4(&mut self, val: u8) {
        self.hdma_dst = self.hdma_dst & 0x1f00 | u16::from(val) & 0xf0;
    }

    fn hdma_on(&self) -> bool {
        !matches!(self.hdma_state, HdmaState::Sleep)
    }

    pub(crate) fn read_hdma5(&self) -> u8 {
        // active on low
        ((!self.hdma_on()) as u8) << 7 | self.hdma5
    }

    pub(crate) fn write_hdma5(&mut self, val: u8) {
        // stop current transfer
        if self.hdma_on() && val & 0x80 == 0 {
            self.hdma_state = HdmaState::Sleep;
            return;
        }

        self.hdma5 = val & !0x80;
        let transfer_blocks = val & 0x7f;
        self.hdma_len = (u16::from(transfer_blocks) + 1) * 0x10;
        self.hdma_state = if val & 0x80 == 0 {
            HdmaState::General
        } else {
            HdmaState::HBlank
        };
    }

    pub(crate) fn run_hdma(&mut self) {
        match self.hdma_state {
            HdmaState::General => (),
            HdmaState::HBlank if self.ppu_mode() == Mode::HBlank => (),
            HdmaState::HBlankDone if self.ppu_mode() != Mode::HBlank => {
                self.hdma_state = HdmaState::HBlank;
                return;
            }
            _ => return,
        }

        let len = if self.hdma_state == HdmaState::HBlank {
            self.hdma_state = HdmaState::HBlankDone;
            self.hdma5 = (self.hdma_len / 0x10).wrapping_sub(1) as u8;
            self.hdma_len -= 0x10;
            0x10
        } else {
            self.hdma_state = HdmaState::Sleep;
            self.hdma5 = 0xff;
            let len = self.hdma_len;
            self.hdma_len = 0;
            len
        };

        for _ in 0..len {
            let val = match self.hdma_src >> 8 {
                0x00..=0x7f => self.cart.read_rom(self.hdma_src),
                // TODO: should copy garbage
                0x80..=0x9f => 0xff,
                0xa0..=0xbf => self.cart.read_ram(self.hdma_src),
                0xc0..=0xcf => self.read_ram(self.hdma_src),
                0xd0..=0xdf => self.read_bank_ram(self.hdma_src),
                _ => panic!("Illegal source addr for HDMA transfer"),
            };

            self.advance_cycle();

            self.write_vram(self.hdma_dst, val);

            self.hdma_dst += 1;
            self.hdma_src += 1;
        }

        if self.hdma_len == 0 {
            self.hdma_state = HdmaState::Sleep;
        }
    }

    fn read_rom_or_cart(&mut self, addr: u16) -> u8 {
        if unlikely(self.rom.is_mapped) {
            return self.rom.read(addr);
        }

        self.cart.read_rom(addr)
    }

    pub(crate) fn read_mem(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff | 0x0200..=0x08ff => self.read_rom_or_cart(addr),
            0x0100..=0x01ff | 0x0900..=0x7fff => self.cart.read_rom(addr),
            0x8000..=0x9fff => self.read_vram(addr),
            0xa000..=0xbfff => self.cart.read_ram(addr),
            0xc000..=0xcfff | 0xe000..=0xefff => self.read_ram(addr),
            0xd000..=0xdfff | 0xf000..=0xfdff => self.read_bank_ram(addr),
            0xfe00..=0xfe9f => self.read_oam(addr),
            0xfea0..=0xfeff => 0xff,
            0xff00..=0xffff => self.read_high((addr & 0xff) as u8),
        }
    }

    fn read_high(&mut self, addr: u8) -> u8 {
        match addr {
            P1 => self.read_p1(),
            SB => self.serial.read_sb(),
            SC => self.serial.read_sc(),
            DIV => ((self.clk_wide >> 6) & 0xff) as u8,
            TIMA => self.tima,
            TMA => self.tma,
            TAC => 0xf8 | self.tac,
            IF => self.ifr | 0xe0,
            0x10 => self.apu_ch1.read_nr10(),
            0x11 => self.apu_ch1.read_nr11(),
            0x12 => self.apu_ch1.read_nr12(),
            0x14 => self.apu_ch1.read_nr14(),
            0x16 => self.apu_ch2.read_nr21(),
            0x17 => self.apu_ch2.read_nr22(),
            0x19 => self.apu_ch2.read_nr24(),
            0x1a => self.apu_ch3.read_nr30(),
            0x1c => self.apu_ch3.read_nr32(),
            0x1e => self.apu_ch3.read_nr34(),
            0x21 => self.apu_ch4.read_nr42(),
            0x22 => self.apu_ch4.read_nr43(),
            0x23 => self.apu_ch4.read_nr44(),
            0x24 => self.read_nr50(),
            NR51 => self.nr51,
            0x26 => self.read_nr52(),
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
            KEY1 if self.model == Cgb => 0x7e | self.key1,
            0x4f if self.model == Cgb => self.vbk | 0xfe,
            HDMA5 if self.model == Cgb => self.read_hdma5(),
            0x68 if self.model == Cgb => self.bcp.spec(),
            0x69 if self.model == Cgb => self.bcp.data(),
            0x6a if self.model == Cgb => self.ocp.spec(),
            0x6b if self.model == Cgb => self.ocp.data(),
            OPRI if self.model == Cgb => self.opri,
            0x70 if self.model == Cgb => self.svbk | 0xf8,
            0x80..=0xfe => self.hram[(addr & 0x7f) as usize],
            IE => self.ie,
            _ => 0xff,
        }
    }

    pub(crate) fn write_mem(&mut self, addr: u16, val: u8) {
        match addr {
            // assume bootrom doesn't write to rom
            0x0000..=0x08ff | 0x0900..=0x7fff => self.cart.write_rom(addr, val),
            0x8000..=0x9fff => self.write_vram(addr, val),
            0xa000..=0xbfff => self.cart.write_ram(addr, val),
            0xc000..=0xcfff | 0xe000..=0xefff => self.write_ram(addr, val),
            0xd000..=0xdfff | 0xf000..=0xfdff => self.write_bank_ram(addr, val),
            0xfe00..=0xfe9f => {
                self.write_oam(addr, val, self.dma_active());
            }
            0xfea0..=0xfeff => (),
            0xff00..=0xffff => self.write_high((addr & 0xff) as u8, val),
        }
    }

    fn write_high(&mut self, addr: u8, val: u8) {
        match addr {
            P1 => self.write_joy(val),
            SB => self.serial.write_sb(val),
            SC => self.serial.write_sc(val),
            DIV => self.write_div(),
            TIMA => self.write_tima(val),
            TMA => self.write_tma(val),
            TAC => self.write_tac(val),
            IF => self.ifr = val & 0x1f,
            0x10 if self.apu_on => self.apu_ch1.write_nr10(val),
            0x11 if self.apu_on => self.apu_ch1.write_nr11(val),
            0x12 if self.apu_on => self.apu_ch1.write_nr12(val),
            0x13 if self.apu_on => self.apu_ch1.write_nr13(val),
            0x14 if self.apu_on => self.apu_ch1.write_nr14(val),
            0x16 if self.apu_on => self.apu_ch2.write_nr21(val),
            0x17 if self.apu_on => self.apu_ch2.write_nr22(val),
            0x18 if self.apu_on => self.apu_ch2.write_nr23(val),
            0x19 if self.apu_on => self.apu_ch2.write_nr24(val),
            0x1a if self.apu_on => self.apu_ch3.write_nr30(val),
            0x1b if self.apu_on => self.apu_ch3.write_nr31(val),
            0x1c if self.apu_on => self.apu_ch3.write_nr32(val),
            0x1d if self.apu_on => self.apu_ch3.write_nr33(val),
            0x1e if self.apu_on => self.apu_ch3.write_nr34(val),
            0x20 if self.apu_on => self.apu_ch4.write_nr41(val),
            0x21 if self.apu_on => self.apu_ch4.write_nr42(val),
            0x22 if self.apu_on => self.apu_ch4.write_nr43(val),
            0x23 if self.apu_on => self.apu_ch4.write_nr44(val),
            0x24 => self.write_nr50(val),
            NR51 => self.write_nr51(val),
            0x26 => self.write_nr52(val),
            WAV_BEGIN..=WAV_END => self.write_wave(addr, val),
            LCDC => self.write_lcdc(val),
            STAT => self.write_stat(val),
            SCY => self.scy = val,
            SCX => self.scx = val,
            LYC => self.lyc = val,
            DMA => self.write_dma(val),
            BGP => self.bgp = val,
            OBP0 => self.obp0 = val,
            OBP1 => self.obp1 = val,
            WY => self.wy = val,
            WX => self.wx = val,
            KEY0 if self.model == Cgb && self.rom.is_mapped && val == 4 => {
                self.function_mode = FunctionMode::Compat;
            }
            KEY1 if self.model == Cgb => {
                self.key1 &= KEY1_SWITCH_B;
                self.key1 |= val & KEY1_SWITCH_B;
            }
            VBK if self.model == Cgb => self.vbk = val & 1,
            0x50 => {
                if val & 1 != 0 {
                    self.rom.is_mapped = false;
                }
            }
            HDMA1 if self.model == Cgb => self.write_hdma1(val),
            HDMA2 if self.model == Cgb => self.write_hdma2(val),
            HDMA3 if self.model == Cgb => self.write_hdma3(val),
            HDMA4 if self.model == Cgb => self.write_hdma4(val),
            HDMA5 if self.model == Cgb => self.write_hdma5(val),
            0x68 if self.model == Cgb => self.bcp.set_spec(val),
            0x69 if self.model == Cgb => self.bcp.set_data(val),
            0x6a if self.model == Cgb => self.ocp.set_spec(val),
            0x6b if self.model == Cgb => self.ocp.set_data(val),
            0x6c if self.model == Cgb => self.opri = val,
            0x70 if self.model == Cgb => {
                let tmp = val & 7;
                self.svbk = tmp;
                self.svbk_true = if tmp == 0 { 1 } else { tmp };
            }
            0x80..=0xfe => self.hram[(addr & 0x7f) as usize] = val,
            0xff => self.ie = val,
            _ => self.advance_cycle(),
        }
    }
}
