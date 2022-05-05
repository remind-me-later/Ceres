use super::{FunctionMode, Memory};
use crate::{audio::Apu, interrupts::Interrupts, timer::Timer, Model::Cgb};

impl Memory {
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

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00ff => self.mem_tick(|mem| {
                mem.boot_rom
                    .read(addr)
                    .unwrap_or_else(|| mem.cart.borrow_mut().read_rom(addr))
            }),
            0x0100..=0x01ff => self.mem_tick(|mem| mem.cart.borrow_mut().read_rom(addr)),
            0x0200..=0x08ff => self.mem_tick(|mem| {
                mem.boot_rom
                    .read(addr)
                    .unwrap_or_else(|| mem.cart.borrow_mut().read_rom(addr))
            }),
            0x0900..=0x7fff => self.mem_tick(|mem| mem.cart.borrow_mut().read_rom(addr)),
            0x8000..=0x9fff => self.mem_tick(|mem| mem.ppu.read_vram(addr)),
            0xa000..=0xbfff => self.mem_tick(|mem| mem.cart.borrow_mut().read_ram(addr)),
            0xc000..=0xcfff => self.mem_tick(|mem| mem.wram.read_ram(addr)),
            0xd000..=0xdfff => self.mem_tick(|mem| mem.wram.read_bank_ram(addr)),
            // Echo RAM
            0xe000..=0xefff => self.mem_tick(|mem| mem.wram.read_ram(addr)),
            0xf000..=0xfdff => self.mem_tick(|mem| mem.wram.read_bank_ram(addr)),
            0xfe00..=0xfe9f => self.mem_tick(|mem| mem.ppu.read_oam(addr, mem.dma.is_active())),
            0xfea0..=0xfeff => {
                log::warn!("Read from unusable RAM");
                self.mem_tick(|_| 0xff)
            }
            0xff00..=0xffff => self.read_high((addr & 0xff) as u8),
        }
    }

    fn read_high(&mut self, addr: u8) -> u8 {
        match addr {
            0x00 => self.mem_tick(|mem| mem.joypad.read()),
            0x01 => self.mem_tick(|mem| mem.serial.read_sb()),
            0x02 => self.mem_tick(|mem| mem.serial.read_sc()),
            0x04 => self.timer_tick(|timer, ic| timer.read_div(ic)),
            0x05 => self.timer_tick(|timer, ic| timer.read_tima(ic)),
            0x06 => self.timer_tick(|timer, ic| timer.read_tma(ic)),
            0x07 => self.timer_tick(|timer, ic| timer.read_tac(ic)),
            0x0f => self.mem_tick(|mem| mem.ints.read_if()),
            0x10 => self.apu_tick(|apu| apu.read_nr10()),
            0x11 => self.apu_tick(|apu| apu.read_nr11()),
            0x12 => self.apu_tick(|apu| apu.read_nr12()),
            0x13 => self.apu_tick(|apu| apu.read_nr13()),
            0x14 => self.apu_tick(|apu| apu.read_nr14()),
            0x16 => self.apu_tick(|apu| apu.read_nr21()),
            0x17 => self.apu_tick(|apu| apu.read_nr22()),
            0x18 => self.apu_tick(|apu| apu.read_nr23()),
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
            0x40 => self.mem_tick(|mem| mem.ppu.read_lcdc()),
            0x41 => self.mem_tick(|mem| mem.ppu.read_stat()),
            0x42 => self.mem_tick(|mem| mem.ppu.read_scy()),
            0x43 => self.mem_tick(|mem| mem.ppu.read_scx()),
            0x44 => self.mem_tick(|mem| mem.ppu.read_ly()),
            0x45 => self.mem_tick(|mem| mem.ppu.read_lyc()),
            0x46 => self.mem_tick(|mem| mem.dma.read()),
            0x47 => self.mem_tick(|mem| mem.ppu.read_bgp()),
            0x48 => self.mem_tick(|mem| mem.ppu.read_obp0()),
            0x49 => self.mem_tick(|mem| mem.ppu.read_obp1()),
            0x4a => self.mem_tick(|mem| mem.ppu.read_wy()),
            0x4b => self.mem_tick(|mem| mem.ppu.read_wx()),
            0x4d if self.model == Cgb => self.mem_tick(|mem| mem.key1.read()),
            0x4f if self.model == Cgb => self.mem_tick(|mem| mem.ppu.read_vbk()),
            0x55 if self.model == Cgb => self.mem_tick(|mem| mem.hdma.read_hdma5()),
            0x68 if self.model == Cgb => self.mem_tick(|mem| mem.ppu.read_bcps()),
            0x69 if self.model == Cgb => self.mem_tick(|mem| mem.ppu.read_bcpd()),
            0x6a if self.model == Cgb => self.mem_tick(|mem| mem.ppu.read_ocps()),
            0x6b if self.model == Cgb => self.mem_tick(|mem| mem.ppu.read_ocpd()),
            0x6c if self.model == Cgb => self.mem_tick(|mem| mem.ppu.read_opri()),
            0x70 if self.model == Cgb => self.mem_tick(|mem| mem.wram.read_svbk()),
            0x80..=0xfe => self.mem_tick(|mem| mem.hram.read(addr)),
            0xff => self.mem_tick(|mem| mem.ints.read_ie()),
            _ => self.mem_tick(|_| 0xff),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // assume bootrom doesnt write to rom
            0x0000..=0x08ff => self.mem_tick(|mem| mem.cart.borrow_mut().write_rom(addr, val)),
            0x0900..=0x7fff => self.mem_tick(|mem| mem.cart.borrow_mut().write_rom(addr, val)),
            0x8000..=0x9fff => self.mem_tick(|mem| mem.ppu.write_vram(addr, val)),
            0xa000..=0xbfff => self.mem_tick(|mem| mem.cart.borrow_mut().write_ram(addr, val)),
            0xc000..=0xcfff => self.mem_tick(|mem| mem.wram.write_ram(addr, val)),
            0xd000..=0xdfff => self.mem_tick(|mem| mem.wram.write_bank_ram(addr, val)),
            0xe000..=0xefff => self.mem_tick(|mem| mem.wram.write_ram(addr, val)),
            0xf000..=0xfdff => self.mem_tick(|mem| mem.wram.write_bank_ram(addr, val)),
            0xfe00..=0xfe9f => {
                self.mem_tick(|mem| mem.ppu.write_oam(addr, val, mem.dma.is_active()))
            }
            0xfea0..=0xfeff => log::warn!("Write to unusable RAM"),
            0xff00..=0xffff => self.write_high((addr & 0xff) as u8, val),
        }
    }

    fn write_high(&mut self, addr: u8, val: u8) {
        match addr {
            0x00 => self.mem_tick(|mem| mem.joypad.write(val)),
            0x01 => self.mem_tick(|mem| mem.serial.write_sb(val)),
            0x02 => self.mem_tick(|mem| mem.serial.write_sc(val)),
            0x04 => self.timer_tick(|timer, ic| timer.write_div(ic)),
            0x05 => self.timer_tick(|timer, ic| timer.write_tac(ic, val)),
            0x06 => self.timer_tick(|timer, ic| timer.write_tma(ic, val)),
            0x07 => self.timer_tick(|timer, ic| timer.write_tima(ic, val)),
            0x0f => self.mem_tick(|mem| mem.ints.write_if(val)),
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
            0x40 => self.mem_tick(|mem| mem.ppu.write_lcdc(val)),
            0x41 => self.mem_tick(|mem| mem.ppu.write_stat(val)),
            0x42 => self.mem_tick(|mem| mem.ppu.write_scy(val)),
            0x43 => self.mem_tick(|mem| mem.ppu.write_scx(val)),
            0x44 => self.mem_tick(|mem| mem.ppu.write_ly(val)),
            0x45 => self.mem_tick(|mem| mem.ppu.write_lyc(val)),
            0x46 => self.mem_tick(|mem| mem.dma.write(val)),
            0x47 => self.mem_tick(|mem| mem.ppu.write_bgp(val)),
            0x48 => self.mem_tick(|mem| mem.ppu.write_obp0(val)),
            0x49 => self.mem_tick(|mem| mem.ppu.write_obp1(val)),
            0x4a => self.mem_tick(|mem| mem.ppu.write_wy(val)),
            0x4b => self.mem_tick(|mem| mem.ppu.write_wx(val)),
            0x4c if self.model == Cgb && self.boot_rom.is_mapped() && val == 4 => {
                self.function_mode = FunctionMode::Compatibility;
            }
            0x4d if self.model == Cgb => self.mem_tick(|mem| mem.key1.write(val)),
            0x4f if self.model == Cgb => self.mem_tick(|mem| mem.ppu.write_vbk(val)),
            0x50 => {
                self.tick_t_cycle();
                if val & 1 != 0 {
                    self.boot_rom.unmap();
                }
            }
            0x51 if self.model == Cgb => self.mem_tick(|mem| mem.hdma.write_hdma1(val)),
            0x52 if self.model == Cgb => self.mem_tick(|mem| mem.hdma.write_hdma2(val)),
            0x53 if self.model == Cgb => self.mem_tick(|mem| mem.hdma.write_hdma3(val)),
            0x54 if self.model == Cgb => self.mem_tick(|mem| mem.hdma.write_hdma4(val)),
            0x55 if self.model == Cgb => self.mem_tick(|mem| mem.hdma.write_hdma5(val)),
            0x68 if self.model == Cgb => self.mem_tick(|mem| mem.ppu.write_bcps(val)),
            0x69 if self.model == Cgb => self.mem_tick(|mem| mem.ppu.write_bcpd(val)),
            0x6a if self.model == Cgb => self.mem_tick(|mem| mem.ppu.write_ocps(val)),
            0x6b if self.model == Cgb => self.mem_tick(|mem| mem.ppu.write_ocpd(val)),
            0x6c if self.model == Cgb => self.mem_tick(|mem| mem.ppu.write_opri(val)),
            0x70 if self.model == Cgb => self.mem_tick(|mem| mem.wram.write_svbk(val)),
            0x80..=0xfe => self.mem_tick(|mem| mem.hram.write(addr, val)),
            0xff => self.mem_tick(|mem| mem.ints.write_ie(val)),
            _ => self.tick_t_cycle(),
        }
    }
}
