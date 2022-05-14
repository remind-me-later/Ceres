use crate::{apu::Apu, interrupts::Interrupts, timer::Timer, FunctionMode, Gb, Model::Cgb};

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
        F: FnOnce(&mut Apu) -> T,
    {
        self.emulate_dma();
        self.emulate_hdma();
        self.tick_ppu();
        self.timer.tick_t_cycle(&mut self.ints);
        self.tick_apu();
        f(&mut self.apu)
    }

    fn timer_tick<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Timer, &mut Interrupts) -> T,
    {
        self.emulate_dma();
        self.emulate_hdma();
        self.tick_ppu();
        let result = f(&mut self.timer, &mut self.ints);
        self.tick_apu();
        result
    }

    pub fn read_mem(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff => self.mem_tick(|gb| {
                gb.boot_rom
                    .read(addr)
                    .unwrap_or_else(|| gb.cart.read_rom(addr))
            }),
            0x0100..=0x01ff => self.mem_tick(|gb| gb.cart.read_rom(addr)),
            0x0200..=0x08ff => self.mem_tick(|gb| {
                gb.boot_rom
                    .read(addr)
                    .unwrap_or_else(|| gb.cart.read_rom(addr))
            }),
            0x0900..=0x7fff => self.mem_tick(|gb| gb.cart.read_rom(addr)),
            0x8000..=0x9fff => self.mem_tick(|gb| gb.ppu.read_vram(addr)),
            0xa000..=0xbfff => self.mem_tick(|gb| gb.cart.read_ram(addr)),
            0xc000..=0xcfff => self.mem_tick(|gb| gb.wram.read_ram(addr)),
            0xd000..=0xdfff => self.mem_tick(|gb| gb.wram.read_bank_ram(addr)),
            // Echo RAM
            0xe000..=0xefff => self.mem_tick(|gb| gb.wram.read_ram(addr)),
            0xf000..=0xfdff => self.mem_tick(|gb| gb.wram.read_bank_ram(addr)),
            0xfe00..=0xfe9f => self.mem_tick(|gb| gb.ppu.read_oam(addr, gb.dma.is_active())),
            0xfea0..=0xfeff => self.mem_tick(|_| 0xff),
            0xff00..=0xffff => self.read_high((addr & 0xff) as u8),
        }
    }

    fn read_high(&mut self, addr: u8) -> u8 {
        match addr {
            0x00 => self.mem_tick(|gb| gb.joypad.read()),
            0x01 => self.mem_tick(|gb| gb.serial.read_sb()),
            0x02 => self.mem_tick(|gb| gb.serial.read_sc()),
            0x04 => self.timer_tick(Timer::read_div),
            0x05 => self.timer_tick(Timer::read_tima),
            0x06 => self.timer_tick(Timer::read_tma),
            0x07 => self.timer_tick(Timer::read_tac),
            0x0f => self.mem_tick(|gb| gb.ints.read_if()),
            0x10 => self.apu_tick(|apu| apu.read_nr10()),
            0x11 => self.apu_tick(|apu| apu.read_nr11()),
            0x12 => self.apu_tick(|apu| apu.read_nr12()),
            0x14 => self.apu_tick(|apu| apu.read_nr14()),
            0x16 => self.apu_tick(|apu| apu.read_nr21()),
            0x17 => self.apu_tick(|apu| apu.read_nr22()),
            0x19 => self.apu_tick(|apu| apu.read_nr24()),
            0x1a => self.apu_tick(|apu| apu.read_nr30()),
            0x1c => self.apu_tick(|apu| apu.read_nr32()),
            0x1e => self.apu_tick(|apu| apu.read_nr34()),
            0x21 => self.apu_tick(|apu| apu.read_nr42()),
            0x22 => self.apu_tick(|apu| apu.read_nr43()),
            0x23 => self.apu_tick(|apu| apu.read_nr44()),
            0x24 => self.apu_tick(|apu| apu.read_nr50()),
            0x25 => self.apu_tick(|apu| apu.read_nr51()),
            0x26 => self.apu_tick(|apu| apu.read_nr52()),
            0x30..=0x3f => self.apu_tick(|apu| apu.read_wave(addr)),
            0x40 => self.mem_tick(|gb| gb.ppu.read_lcdc()),
            0x41 => self.mem_tick(|gb| gb.ppu.read_stat()),
            0x42 => self.mem_tick(|gb| gb.ppu.read_scy()),
            0x43 => self.mem_tick(|gb| gb.ppu.read_scx()),
            0x44 => self.mem_tick(|gb| gb.ppu.read_ly()),
            0x45 => self.mem_tick(|gb| gb.ppu.read_lyc()),
            0x46 => self.mem_tick(|gb| gb.dma.read()),
            0x47 => self.mem_tick(|gb| gb.ppu.read_bgp()),
            0x48 => self.mem_tick(|gb| gb.ppu.read_obp0()),
            0x49 => self.mem_tick(|gb| gb.ppu.read_obp1()),
            0x4a => self.mem_tick(|gb| gb.ppu.read_wy()),
            0x4b => self.mem_tick(|gb| gb.ppu.read_wx()),
            0x4d if self.model == Cgb => self.mem_tick(|gb| 0x7e | gb.key1),
            0x4f if self.model == Cgb => self.mem_tick(|gb| gb.ppu.read_vbk()),
            0x55 if self.model == Cgb => self.mem_tick(|gb| gb.hdma.read_hdma5()),
            0x68 if self.model == Cgb => self.mem_tick(|gb| gb.ppu.read_bcps()),
            0x69 if self.model == Cgb => self.mem_tick(|gb| gb.ppu.read_bcpd()),
            0x6a if self.model == Cgb => self.mem_tick(|gb| gb.ppu.read_ocps()),
            0x6b if self.model == Cgb => self.mem_tick(|gb| gb.ppu.read_ocpd()),
            0x6c if self.model == Cgb => self.mem_tick(|gb| gb.ppu.read_opri()),
            0x70 if self.model == Cgb => self.mem_tick(|gb| gb.wram.read_svbk()),
            0x80..=0xfe => self.mem_tick(|gb| gb.hram.read(addr)),
            0xff => self.mem_tick(|gb| gb.ints.read_ie()),
            _ => self.mem_tick(|_| 0xff),
        }
    }

    pub fn write_mem(&mut self, addr: u16, val: u8) {
        match addr {
            // assume bootrom doesnt write to rom
            0x0000..=0x08ff => self.mem_tick(|gb| gb.cart.write_rom(addr, val)),
            0x0900..=0x7fff => self.mem_tick(|gb| gb.cart.write_rom(addr, val)),
            0x8000..=0x9fff => self.mem_tick(|gb| gb.ppu.write_vram(addr, val)),
            0xa000..=0xbfff => self.mem_tick(|gb| gb.cart.write_ram(addr, val)),
            0xc000..=0xcfff => self.mem_tick(|gb| gb.wram.write_ram(addr, val)),
            0xd000..=0xdfff => self.mem_tick(|gb| gb.wram.write_bank_ram(addr, val)),
            0xe000..=0xefff => self.mem_tick(|gb| gb.wram.write_ram(addr, val)),
            0xf000..=0xfdff => self.mem_tick(|gb| gb.wram.write_bank_ram(addr, val)),
            0xfe00..=0xfe9f => {
                self.mem_tick(|gb| gb.ppu.write_oam(addr, val, gb.dma.is_active()));
            }
            0xfea0..=0xfeff => self.mem_tick(|_| ()),
            0xff00..=0xffff => self.write_high((addr & 0xff) as u8, val),
        }
    }

    fn write_high(&mut self, addr: u8, val: u8) {
        match addr {
            0x00 => self.mem_tick(|gb| gb.joypad.write(val)),
            0x01 => self.mem_tick(|gb| gb.serial.write_sb(val)),
            0x02 => self.mem_tick(|gb| gb.serial.write_sc(val)),
            0x04 => self.timer_tick(Timer::write_div),
            0x05 => self.timer_tick(|timer, ic| timer.write_tima(ic, val)),
            0x06 => self.timer_tick(|timer, ic| timer.write_tma(ic, val)),
            0x07 => self.timer_tick(|timer, ic| timer.write_tac(ic, val)),
            0x0f => self.mem_tick(|gb| gb.ints.write_if(val)),
            0x10 => self.apu_tick(|apu| apu.write_nr10(val)),
            0x11 => self.apu_tick(|apu| apu.write_nr11(val)),
            0x12 => self.apu_tick(|apu| apu.write_nr12(val)),
            0x13 => self.apu_tick(|apu| apu.write_nr13(val)),
            0x14 => self.apu_tick(|apu| apu.write_nr14(val)),
            0x16 => self.apu_tick(|apu| apu.write_nr21(val)),
            0x17 => self.apu_tick(|apu| apu.write_nr22(val)),
            0x18 => self.apu_tick(|apu| apu.write_nr23(val)),
            0x19 => self.apu_tick(|apu| apu.write_nr24(val)),
            0x1a => self.apu_tick(|apu| apu.write_nr30(val)),
            0x1b => self.apu_tick(|apu| apu.write_nr31(val)),
            0x1c => self.apu_tick(|apu| apu.write_nr32(val)),
            0x1d => self.apu_tick(|apu| apu.write_nr33(val)),
            0x1e => self.apu_tick(|apu| apu.write_nr34(val)),
            0x20 => self.apu_tick(|apu| apu.write_nr41(val)),
            0x21 => self.apu_tick(|apu| apu.write_nr42(val)),
            0x22 => self.apu_tick(|apu| apu.write_nr43(val)),
            0x23 => self.apu_tick(|apu| apu.write_nr44(val)),
            0x24 => self.apu_tick(|apu| apu.write_nr50(val)),
            0x25 => self.apu_tick(|apu| apu.write_nr51(val)),
            0x26 => self.apu_tick(|apu| apu.write_nr52(val)),
            0x30..=0x3f => self.apu_tick(|apu| apu.write_wave(addr, val)),
            0x40 => self.mem_tick(|gb| gb.ppu.write_lcdc(val)),
            0x41 => self.mem_tick(|gb| gb.ppu.write_stat(val)),
            0x42 => self.mem_tick(|gb| gb.ppu.write_scy(val)),
            0x43 => self.mem_tick(|gb| gb.ppu.write_scx(val)),
            0x45 => self.mem_tick(|gb| gb.ppu.write_lyc(val)),
            0x46 => self.mem_tick(|gb| gb.dma.write(val)),
            0x47 => self.mem_tick(|gb| gb.ppu.write_bgp(val)),
            0x48 => self.mem_tick(|gb| gb.ppu.write_obp0(val)),
            0x49 => self.mem_tick(|gb| gb.ppu.write_obp1(val)),
            0x4a => self.mem_tick(|gb| gb.ppu.write_wy(val)),
            0x4b => self.mem_tick(|gb| gb.ppu.write_wx(val)),
            0x4c if self.model == Cgb && self.boot_rom.is_mapped() && val == 4 => {
                self.function_mode = FunctionMode::Compat;
            }
            0x4d if self.model == Cgb => self.mem_tick(|gb| {
                gb.key1 = val & 1;
            }),
            0x4f if self.model == Cgb => self.mem_tick(|gb| gb.ppu.write_vbk(val)),
            0x50 => {
                self.tick_t_cycle();
                if val & 1 != 0 {
                    self.boot_rom.unmap();
                }
            }
            0x51 if self.model == Cgb => self.mem_tick(|gb| gb.hdma.write_hdma1(val)),
            0x52 if self.model == Cgb => self.mem_tick(|gb| gb.hdma.write_hdma2(val)),
            0x53 if self.model == Cgb => self.mem_tick(|gb| gb.hdma.write_hdma3(val)),
            0x54 if self.model == Cgb => self.mem_tick(|gb| gb.hdma.write_hdma4(val)),
            0x55 if self.model == Cgb => self.mem_tick(|gb| gb.hdma.write_hdma5(val)),
            0x68 if self.model == Cgb => self.mem_tick(|gb| gb.ppu.write_bcps(val)),
            0x69 if self.model == Cgb => self.mem_tick(|gb| gb.ppu.write_bcpd(val)),
            0x6a if self.model == Cgb => self.mem_tick(|gb| gb.ppu.write_ocps(val)),
            0x6b if self.model == Cgb => self.mem_tick(|gb| gb.ppu.write_ocpd(val)),
            0x6c if self.model == Cgb => self.mem_tick(|gb| gb.ppu.write_opri(val)),
            0x70 if self.model == Cgb => self.mem_tick(|gb| gb.wram.write_svbk(val)),
            0x80..=0xfe => self.mem_tick(|gb| gb.hram.write(addr, val)),
            0xff => self.mem_tick(|gb| gb.ints.write_ie(val)),
            _ => self.tick_t_cycle(),
        }
    }
}
