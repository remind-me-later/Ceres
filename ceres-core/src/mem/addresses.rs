use {
    super::KEY1_SWITCH,
    crate::{ppu::LCDC_ENA_B, FunctionMode, Gb, Model::Cgb},
    core::intrinsics::unlikely,
};

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
    fn mem_tick<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.tick_t_cycle();
        f(self)
    }

    fn apu_tick<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.emulate_dma();
        self.emulate_hdma();
        self.tick_ppu();
        self.tick_timer();
        self.tick_apu();
        f(self)
    }

    fn timer_tick<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.emulate_dma();
        self.emulate_hdma();
        self.tick_ppu();
        let result = f(self);
        self.tick_apu();
        result
    }

    fn brom_or_rom(&mut self, addr: u16) -> u8 {
        if unlikely(self.brom.is_mapped()) {
            return self.brom.read(addr);
        }

        self.cart.read_rom(addr)
    }

    pub(crate) fn read_mem(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff => self.mem_tick(|gb| gb.brom_or_rom(addr)),
            0x0100..=0x01ff => self.mem_tick(|gb| gb.cart.read_rom(addr)),
            0x0200..=0x08ff => self.mem_tick(|gb| gb.brom_or_rom(addr)),
            0x0900..=0x7fff => self.mem_tick(|gb| gb.cart.read_rom(addr)),
            0x8000..=0x9fff => self.mem_tick(|gb| gb.read_vram(addr)),
            0xa000..=0xbfff => self.mem_tick(|gb| gb.cart.read_ram(addr)),
            0xc000..=0xcfff => self.mem_tick(|gb| gb.read_ram(addr)),
            0xd000..=0xdfff => self.mem_tick(|gb| gb.read_bank_ram(addr)),
            // Echo RAM
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
            0x10 => self.apu_tick(|gb| gb.apu_ch1.read_nr10()),
            0x11 => self.apu_tick(|gb| gb.apu_ch1.read_nr11()),
            0x12 => self.apu_tick(|gb| gb.apu_ch1.read_nr12()),
            0x14 => self.apu_tick(|gb| gb.apu_ch1.read_nr14()),
            0x16 => self.apu_tick(|gb| gb.apu_ch2.read_nr21()),
            0x17 => self.apu_tick(|gb| gb.apu_ch2.read_nr22()),
            0x19 => self.apu_tick(|gb| gb.apu_ch2.read_nr24()),
            0x1a => self.apu_tick(|gb| gb.apu_ch3.read_nr30()),
            0x1c => self.apu_tick(|gb| gb.apu_ch3.read_nr32()),
            0x1e => self.apu_tick(|gb| gb.apu_ch3.read_nr34()),
            0x21 => self.apu_tick(|gb| gb.apu_ch4.read_nr42()),
            0x22 => self.apu_tick(|gb| gb.apu_ch4.read_nr43()),
            0x23 => self.apu_tick(|gb| gb.apu_ch4.read_nr44()),
            0x24 => self.apu_tick(|gb| gb.read_nr50()),
            NR51 => self.apu_tick(|gb| gb.nr51),
            0x26 => self.apu_tick(|gb| gb.read_nr52()),
            WAV_BEGIN..=WAV_END => self.apu_tick(|gb| gb.apu_ch3.read_wave_ram(addr)),
            LCDC => self.mem_tick(|gb| gb.lcdc),
            STAT => self.mem_tick(|gb| {
                if gb.lcdc & LCDC_ENA_B == 0 {
                    0x80
                } else {
                    gb.stat | 0x80
                }
            }),
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
            // assume bootrom doesnt write to rom
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

    #[allow(clippy::too_many_lines)]
    fn write_high(&mut self, addr: u8, val: u8) {
        match addr {
            0x00 => self.mem_tick(|gb| gb.write_joy(val)),
            0x01 => self.mem_tick(|gb| gb.serial.write_sb(val)),
            0x02 => self.mem_tick(|gb| gb.serial.write_sc(val)),
            0x04 => self.timer_tick(Self::write_div),
            0x05 => self.timer_tick(|gb| gb.write_tima(val)),
            0x06 => self.timer_tick(|gb| gb.write_tma(val)),
            0x07 => self.timer_tick(|gb| gb.write_tac(val)),
            0x0f => self.mem_tick(|gb| gb.ifr = val & 0x1f),
            0x10 if self.apu_on => self.apu_tick(|gb| gb.apu_ch1.write_nr10(val)),
            0x11 if self.apu_on => self.apu_tick(|gb| gb.apu_ch1.write_nr11(val)),
            0x12 if self.apu_on => self.apu_tick(|gb| gb.apu_ch1.write_nr12(val)),
            0x13 if self.apu_on => self.apu_tick(|gb| gb.apu_ch1.write_nr13(val)),
            0x14 if self.apu_on => self.apu_tick(|gb| gb.apu_ch1.write_nr14(val)),
            0x16 if self.apu_on => self.apu_tick(|gb| gb.apu_ch2.write_nr21(val)),
            0x17 if self.apu_on => self.apu_tick(|gb| gb.apu_ch2.write_nr22(val)),
            0x18 if self.apu_on => self.apu_tick(|gb| gb.apu_ch2.write_nr23(val)),
            0x19 if self.apu_on => self.apu_tick(|gb| gb.apu_ch2.write_nr24(val)),
            0x1a if self.apu_on => self.apu_tick(|gb| gb.apu_ch3.write_nr30(val)),
            0x1b if self.apu_on => self.apu_tick(|gb| gb.apu_ch3.write_nr31(val)),
            0x1c if self.apu_on => self.apu_tick(|gb| gb.apu_ch3.write_nr32(val)),
            0x1d if self.apu_on => self.apu_tick(|gb| gb.apu_ch3.write_nr33(val)),
            0x1e if self.apu_on => self.apu_tick(|gb| gb.apu_ch3.write_nr34(val)),
            0x20 if self.apu_on => self.apu_tick(|gb| gb.apu_ch4.write_nr41(val)),
            0x21 if self.apu_on => self.apu_tick(|gb| gb.apu_ch4.write_nr42(val)),
            0x22 if self.apu_on => self.apu_tick(|gb| gb.apu_ch4.write_nr43(val)),
            0x23 if self.apu_on => self.apu_tick(|gb| gb.apu_ch4.write_nr44(val)),
            0x24 => self.apu_tick(|gb| gb.write_nr50(val)),
            0x25 => self.apu_tick(|gb| gb.write_nr51(val)),
            0x26 => self.apu_tick(|gb| gb.write_nr52(val)),
            0x30..=0x3f => self.apu_tick(|gb| gb.write_wave(addr, val)),
            0x40 => self.mem_tick(|gb| gb.write_lcdc(val)),
            0x41 => self.mem_tick(|gb| gb.write_stat(val)),
            0x42 => self.mem_tick(|gb| gb.scy = val),
            0x43 => self.mem_tick(|gb| gb.scx = val),
            0x45 => self.mem_tick(|gb| gb.lyc = val),
            0x46 => self.mem_tick(|gb| gb.write_dma(val)),
            0x47 => self.mem_tick(|gb| gb.bgp = val),
            0x48 => self.mem_tick(|gb| gb.obp0 = val),
            0x49 => self.mem_tick(|gb| gb.obp1 = val),
            0x4a => self.mem_tick(|gb| gb.wy = val),
            0x4b => self.mem_tick(|gb| gb.wx = val),
            KEY0 if self.model == Cgb && self.brom.is_mapped() && val == 4 => {
                self.function_mode = FunctionMode::Compat;
            }
            KEY1 if self.model == Cgb => self.mem_tick(|gb| {
                gb.key1 &= KEY1_SWITCH;
                gb.key1 |= val & KEY1_SWITCH;
            }),
            VBK if self.model == Cgb => self.mem_tick(|gb| gb.vbk = val & 1),
            0x50 => self.mem_tick(|gb| {
                if val & 1 != 0 {
                    gb.brom.unmap();
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
            _ => self.tick_t_cycle(),
        }
    }
}
