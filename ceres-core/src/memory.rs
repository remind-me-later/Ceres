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

    // tick one t cycle
    pub(crate) fn tick(&mut self) {
        self.tick_dma();
        self.tick_hdma();
        self.tick_ppu();
        self.tick_timer();
        self.tick_apu();
    }

    #[must_use]
    pub(crate) fn t_elapsed(&self) -> u8 {
        if self.double_speed { 2 } else { 4 }
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
        self.dma_cycles += 4;

        if !self.dma_on || self.dma_cycles < 4 {
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

    pub(crate) fn tick_hdma(&mut self) {
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

            self.tick_dma();
            self.tick_ppu();
            self.tick_timer();
            self.tick_apu();

            self.write_vram(self.hdma_dst, val);

            self.hdma_dst += 1;
            self.hdma_src += 1;
        }

        if self.hdma_len == 0 {
            self.hdma_state = HdmaState::Sleep;
        }
    }

    fn mem_tick<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.tick();
        f(self)
    }

    fn timer_tick<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.tick_dma();
        self.tick_hdma();
        self.tick_ppu();
        let result = f(self);
        self.tick_apu();
        result
    }

    fn read_rom_or_cart(&mut self, addr: u16) -> u8 {
        if unlikely(self.rom.is_mapped()) {
            return self.rom.read(addr);
        }

        self.cart.read_rom(addr)
    }

    pub(crate) fn read_mem(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff => self.mem_tick(|gb| gb.read_rom_or_cart(addr)),
            0x0100..=0x01ff => self.mem_tick(|gb| gb.cart.read_rom(addr)),
            0x0200..=0x08ff => self.mem_tick(|gb| gb.read_rom_or_cart(addr)),
            0x0900..=0x7fff => self.mem_tick(|gb| gb.cart.read_rom(addr)),
            0x8000..=0x9fff => self.mem_tick(|gb| gb.read_vram(addr)),
            0xa000..=0xbfff => self.mem_tick(|gb| gb.cart.read_ram(addr)),
            0xc000..=0xcfff => self.mem_tick(|gb| gb.read_ram(addr)),
            0xd000..=0xdfff => self.mem_tick(|gb| gb.read_bank_ram(addr)),
            0xe000..=0xefff => self.mem_tick(|gb| gb.read_ram(addr)),
            0xf000..=0xfdff => self.mem_tick(|gb| gb.read_bank_ram(addr)),
            0xfe00..=0xfe9f => self.mem_tick(|gb| gb.read_oam(addr)),
            0xfea0..=0xfeff => self.mem_tick(|_| 0xff),
            0xff00..=0xffff => self.read_high((addr & 0xff) as u8),
        }
    }

    fn read_high(&mut self, addr: u8) -> u8 {
        match addr {
            P1 => self.mem_tick(|gb| gb.read_p1()),
            SB => self.mem_tick(|gb| gb.serial.read_sb()),
            SC => self.mem_tick(|gb| gb.serial.read_sc()),
            DIV => self.timer_tick(Self::read_div),
            TIMA => self.timer_tick(Self::read_tima),
            TMA => self.timer_tick(Self::read_tma),
            TAC => self.timer_tick(Self::read_tac),
            IF => self.mem_tick(|gb| gb.ifr | 0xe0),
            0x10 => self.mem_tick(|gb| gb.apu_ch1.read_nr10()),
            0x11 => self.mem_tick(|gb| gb.apu_ch1.read_nr11()),
            0x12 => self.mem_tick(|gb| gb.apu_ch1.read_nr12()),
            0x14 => self.mem_tick(|gb| gb.apu_ch1.read_nr14()),
            0x16 => self.mem_tick(|gb| gb.apu_ch2.read_nr21()),
            0x17 => self.mem_tick(|gb| gb.apu_ch2.read_nr22()),
            0x19 => self.mem_tick(|gb| gb.apu_ch2.read_nr24()),
            0x1a => self.mem_tick(|gb| gb.apu_ch3.read_nr30()),
            0x1c => self.mem_tick(|gb| gb.apu_ch3.read_nr32()),
            0x1e => self.mem_tick(|gb| gb.apu_ch3.read_nr34()),
            0x21 => self.mem_tick(|gb| gb.apu_ch4.read_nr42()),
            0x22 => self.mem_tick(|gb| gb.apu_ch4.read_nr43()),
            0x23 => self.mem_tick(|gb| gb.apu_ch4.read_nr44()),
            0x24 => self.mem_tick(|gb| gb.read_nr50()),
            NR51 => self.mem_tick(|gb| gb.nr51),
            0x26 => self.mem_tick(|gb| gb.read_nr52()),
            WAV_BEGIN..=WAV_END => self.mem_tick(|gb| gb.apu_ch3.read_wave_ram(addr)),
            LCDC => self.mem_tick(|gb| gb.lcdc),
            STAT => self.mem_tick(|gb| gb.stat | 0x80),
            SCY => self.mem_tick(|gb| gb.scy),
            SCX => self.mem_tick(|gb| gb.scx),
            LY => self.mem_tick(|gb| gb.ly),
            LYC => self.mem_tick(|gb| gb.lyc),
            DMA => self.mem_tick(|gb| gb.dma),
            BGP => self.mem_tick(|gb| gb.bgp),
            OBP0 => self.mem_tick(|gb| gb.obp0),
            OBP1 => self.mem_tick(|gb| gb.obp1),
            WY => self.mem_tick(|gb| gb.wy),
            WX => self.mem_tick(|gb| gb.wx),
            KEY1 if self.model == Cgb => self.mem_tick(|gb| 0x7e | gb.key1),
            0x4f if self.model == Cgb => self.mem_tick(|gb| gb.vbk | 0xfe),
            HDMA5 if self.model == Cgb => self.mem_tick(|gb| gb.read_hdma5()),
            0x68 if self.model == Cgb => self.mem_tick(|gb| gb.bcp.spec()),
            0x69 if self.model == Cgb => self.mem_tick(|gb| gb.bcp.data()),
            0x6a if self.model == Cgb => self.mem_tick(|gb| gb.ocp.spec()),
            0x6b if self.model == Cgb => self.mem_tick(|gb| gb.ocp.data()),
            OPRI if self.model == Cgb => self.mem_tick(|gb| gb.opri),
            0x70 if self.model == Cgb => self.mem_tick(|gb: &mut Gb| gb.svbk | 0xf8),
            0x80..=0xfe => self.mem_tick(|gb| gb.hram[(addr & 0x7f) as usize]),
            IE => self.mem_tick(|gb| gb.ie),
            _ => self.mem_tick(|_| 0xff),
        }
    }

    pub(crate) fn write_mem(&mut self, addr: u16, val: u8) {
        match addr {
            // assume bootrom doesn't write to rom
            0x0000..=0x08ff => self.mem_tick(|gb| gb.cart.write_rom(addr, val)),
            0x0900..=0x7fff => self.mem_tick(|gb| gb.cart.write_rom(addr, val)),
            0x8000..=0x9fff => self.mem_tick(|gb| gb.write_vram(addr, val)),
            0xa000..=0xbfff => self.mem_tick(|gb| gb.cart.write_ram(addr, val)),
            0xc000..=0xcfff => self.mem_tick(|gb| gb.write_ram(addr, val)),
            0xd000..=0xdfff => self.mem_tick(|gb| gb.write_bank_ram(addr, val)),
            0xe000..=0xefff => self.mem_tick(|gb| gb.write_ram(addr, val)),
            0xf000..=0xfdff => self.mem_tick(|gb| gb.write_bank_ram(addr, val)),
            0xfe00..=0xfe9f => {
                self.mem_tick(|gb| gb.write_oam(addr, val, gb.dma_active()));
            }
            0xfea0..=0xfeff => self.mem_tick(|_| ()),
            0xff00..=0xffff => self.write_high((addr & 0xff) as u8, val),
        }
    }

    fn write_high(&mut self, addr: u8, val: u8) {
        match addr {
            P1 => self.mem_tick(|gb| gb.write_joy(val)),
            SB => self.mem_tick(|gb| gb.serial.write_sb(val)),
            SC => self.mem_tick(|gb| gb.serial.write_sc(val)),
            DIV => self.timer_tick(Self::write_div),
            TIMA => self.timer_tick(|gb| gb.write_tima(val)),
            TMA => self.timer_tick(|gb| gb.write_tma(val)),
            TAC => self.timer_tick(|gb| gb.write_tac(val)),
            IF => self.mem_tick(|gb| gb.ifr = val & 0x1f),
            0x10 if self.apu_on => self.mem_tick(|gb| gb.apu_ch1.write_nr10(val)),
            0x11 if self.apu_on => self.mem_tick(|gb| gb.apu_ch1.write_nr11(val)),
            0x12 if self.apu_on => self.mem_tick(|gb| gb.apu_ch1.write_nr12(val)),
            0x13 if self.apu_on => self.mem_tick(|gb| gb.apu_ch1.write_nr13(val)),
            0x14 if self.apu_on => self.mem_tick(|gb| gb.apu_ch1.write_nr14(val)),
            0x16 if self.apu_on => self.mem_tick(|gb| gb.apu_ch2.write_nr21(val)),
            0x17 if self.apu_on => self.mem_tick(|gb| gb.apu_ch2.write_nr22(val)),
            0x18 if self.apu_on => self.mem_tick(|gb| gb.apu_ch2.write_nr23(val)),
            0x19 if self.apu_on => self.mem_tick(|gb| gb.apu_ch2.write_nr24(val)),
            0x1a if self.apu_on => self.mem_tick(|gb| gb.apu_ch3.write_nr30(val)),
            0x1b if self.apu_on => self.mem_tick(|gb| gb.apu_ch3.write_nr31(val)),
            0x1c if self.apu_on => self.mem_tick(|gb| gb.apu_ch3.write_nr32(val)),
            0x1d if self.apu_on => self.mem_tick(|gb| gb.apu_ch3.write_nr33(val)),
            0x1e if self.apu_on => self.mem_tick(|gb| gb.apu_ch3.write_nr34(val)),
            0x20 if self.apu_on => self.mem_tick(|gb| gb.apu_ch4.write_nr41(val)),
            0x21 if self.apu_on => self.mem_tick(|gb| gb.apu_ch4.write_nr42(val)),
            0x22 if self.apu_on => self.mem_tick(|gb| gb.apu_ch4.write_nr43(val)),
            0x23 if self.apu_on => self.mem_tick(|gb| gb.apu_ch4.write_nr44(val)),
            0x24 => self.mem_tick(|gb| gb.write_nr50(val)),
            NR51 => self.mem_tick(|gb| gb.write_nr51(val)),
            0x26 => self.mem_tick(|gb| gb.write_nr52(val)),
            WAV_BEGIN..=WAV_END => self.mem_tick(|gb| gb.write_wave(addr, val)),
            LCDC => self.mem_tick(|gb| gb.write_lcdc(val)),
            STAT => self.mem_tick(|gb| gb.write_stat(val)),
            SCY => self.mem_tick(|gb| gb.scy = val),
            SCX => self.mem_tick(|gb| gb.scx = val),
            LYC => self.mem_tick(|gb| gb.lyc = val),
            DMA => self.mem_tick(|gb| gb.write_dma(val)),
            BGP => self.mem_tick(|gb| gb.bgp = val),
            OBP0 => self.mem_tick(|gb| gb.obp0 = val),
            OBP1 => self.mem_tick(|gb| gb.obp1 = val),
            WY => self.mem_tick(|gb| gb.wy = val),
            WX => self.mem_tick(|gb| gb.wx = val),
            KEY0 if self.model == Cgb && self.rom.is_mapped() && val == 4 => {
                self.function_mode = FunctionMode::Compat;
            }
            KEY1 if self.model == Cgb => self.mem_tick(|gb| {
                gb.key1 &= KEY1_SWITCH_B;
                gb.key1 |= val & KEY1_SWITCH_B;
            }),
            VBK if self.model == Cgb => self.mem_tick(|gb| gb.vbk = val & 1),
            0x50 => self.mem_tick(|gb| {
                if val & 1 != 0 {
                    gb.rom.unmap();
                }
            }),
            HDMA1 if self.model == Cgb => self.mem_tick(|gb| gb.write_hdma1(val)),
            HDMA2 if self.model == Cgb => self.mem_tick(|gb| gb.write_hdma2(val)),
            HDMA3 if self.model == Cgb => self.mem_tick(|gb| gb.write_hdma3(val)),
            HDMA4 if self.model == Cgb => self.mem_tick(|gb| gb.write_hdma4(val)),
            HDMA5 if self.model == Cgb => self.mem_tick(|gb| gb.write_hdma5(val)),
            0x68 if self.model == Cgb => self.mem_tick(|gb| gb.bcp.set_spec(val)),
            0x69 if self.model == Cgb => self.mem_tick(|gb| gb.bcp.set_data(val)),
            0x6a if self.model == Cgb => self.mem_tick(|gb| gb.ocp.set_spec(val)),
            0x6b if self.model == Cgb => self.mem_tick(|gb| gb.ocp.set_data(val)),
            0x6c if self.model == Cgb => self.mem_tick(|gb| gb.opri = val),
            0x70 if self.model == Cgb => self.mem_tick(|gb| {
                let tmp = val & 7;
                gb.svbk = tmp;
                gb.svbk_true = if tmp == 0 { 1 } else { tmp };
            }),
            0x80..=0xfe => self.mem_tick(|gb| gb.hram[(addr & 0x7f) as usize] = val),
            0xff => self.mem_tick(|gb| gb.ie = val),
            _ => self.tick(),
        }
    }
}
