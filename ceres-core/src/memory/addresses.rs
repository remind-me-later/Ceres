use super::{
    dma_controller::DmaRegister::{Dma, HDMA1, HDMA2, HDMA3, HDMA4, HDMA5},
    FunctionMode, Memory,
};
use crate::{
    audio::{
        Apu,
        ApuIO::{ApuRegister, WavePatternRam},
        ApuRegister::{
            NR10, NR11, NR12, NR13, NR14, NR21, NR22, NR23, NR24, NR30, NR31, NR32, NR33, NR34,
            NR41, NR42, NR43, NR44, NR50, NR51, NR52,
        },
    },
    cartridge::RumbleCallbacks,
    interrupts::{
        ICRegister::{Ie, If},
        InterruptController,
    },
    serial::Register::{SB, SC},
    timer::{
        Register::{Div, Tac, Tima, Tma},
        Timer,
    },
    video::ppu::{
        PpuIO::{Oam, PpuRegister, Vram, VramBank},
        PpuRegister::{
            Bcpd, Bcps, Bgp, Lcdc, Ly, Lyc, Obp0, Obp1, Ocpd, Ocps, Opri, Scx, Scy, Stat, Wx, Wy,
        },
    },
    AudioCallbacks, Model,
};

impl<'a, A: AudioCallbacks, R: RumbleCallbacks> Memory<A, R> {
    fn generic_mem_cycle<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.tick_t_cycle();
        f(self)
    }

    fn apu_mem_cycle<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Apu<A>) -> T,
    {
        self.emulate_oam_dma();
        self.emulate_hdma();
        self.tick_ppu();
        self.timer.tick_t_cycle(&mut self.interrupt_controller);
        self.tick_apu();
        f(&mut self.apu)
    }

    fn timer_mem_cycle<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Timer, &mut InterruptController) -> T,
    {
        self.emulate_oam_dma();
        self.emulate_hdma();
        self.tick_ppu();
        let result = f(&mut self.timer, &mut self.interrupt_controller);
        self.tick_apu();
        result
    }

    pub fn read(&mut self, address: u16) -> u8 {
        match address {
            0x00..=0xff => self.generic_mem_cycle(|mem| {
                if mem.boot_rom.is_active() {
                    mem.boot_rom.read(address)
                } else {
                    mem.cartridge.read_rom(address)
                }
            }),
            0x0100..=0x1ff => self.generic_mem_cycle(|mem| mem.cartridge.read_rom(address)),
            0x200..=0x8ff => self.generic_mem_cycle(|mem| {
                if mem.boot_rom.is_active() {
                    mem.boot_rom.read(address)
                } else {
                    mem.cartridge.read_rom(address)
                }
            }),
            0x0900..=0x7fff => self.generic_mem_cycle(|mem| mem.cartridge.read_rom(address)),
            0x8000..=0x9fff => self.generic_mem_cycle(|mem| mem.ppu.read(Vram { address })),
            0xa000..=0xbfff => self.generic_mem_cycle(|mem| mem.cartridge.read_ram(address)),
            0xc000..=0xcfff => self.generic_mem_cycle(|mem| mem.work_ram.read_low(address)),
            0xd000..=0xdfff => self.generic_mem_cycle(|mem| mem.work_ram.read_high(address)),
            // Echo RAM
            0xe000..=0xefff => self.generic_mem_cycle(|mem| mem.work_ram.read_low(address)),
            0xf000..=0xfdff => self.generic_mem_cycle(|mem| mem.work_ram.read_high(address)),
            0xfe00..=0xfe9f => self.generic_mem_cycle(|mem| {
                if mem.dma_controller.is_dma_active() {
                    0xff
                } else {
                    mem.ppu.read(Oam { address })
                }
            }),
            0xfea0..=0xfeff => {
                log::warn!("Read from unusable RAM");
                0xff
            }
            0xff00..=0xffff => self.read_high((address & 0xff) as u8),
        }
    }

    fn read_high(&mut self, address: u8) -> u8 {
        match address {
            0x00 => self.generic_mem_cycle(|mem| mem.joypad.read()),
            0x01 => self.generic_mem_cycle(|mem| mem.serial.read(SB)),
            0x02 => self.generic_mem_cycle(|mem| mem.serial.read(SC)),
            0x04 => self.timer_mem_cycle(|timer, ic| timer.read(ic, Div)),
            0x05 => self.timer_mem_cycle(|timer, ic| timer.read(ic, Tima)),
            0x06 => self.timer_mem_cycle(|timer, ic| timer.read(ic, Tma)),
            0x07 => self.timer_mem_cycle(|timer, ic| timer.read(ic, Tac)),
            0x0f => self.generic_mem_cycle(|mem| mem.interrupt_controller.read(If)),
            0x10 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR10))),
            0x11 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR11))),
            0x12 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR12))),
            0x13 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR13))),
            0x14 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR14))),
            0x16 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR21))),
            0x17 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR22))),
            0x18 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR23))),
            0x19 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR24))),
            0x1a => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR30))),
            0x1b => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR31))),
            0x1c => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR32))),
            0x1d => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR33))),
            0x1e => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR34))),
            0x20 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR41))),
            0x21 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR42))),
            0x22 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR43))),
            0x23 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR44))),
            0x24 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR50))),
            0x25 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR51))),
            0x26 => self.apu_mem_cycle(|apu| apu.read(ApuRegister(NR52))),
            0x30..=0x3f => self.apu_mem_cycle(|apu| apu.read(WavePatternRam { address })),
            0x40 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Lcdc))),
            0x41 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Stat))),
            0x42 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Scy))),
            0x43 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Scx))),
            0x44 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Ly))),
            0x45 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Lyc))),
            0x46 => self.generic_mem_cycle(|mem| mem.dma_controller.read(Dma)),
            0x47 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Bgp))),
            0x48 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Obp0))),
            0x49 => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Obp1))),
            0x4a => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Wy))),
            0x4b => self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Wx))),
            0x4d if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.speed_switch_register.read())
            }
            0x4f if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.read(VramBank))
            }
            0x51 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.read(HDMA1))
            }
            0x52 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.read(HDMA2))
            }
            0x53 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.read(HDMA3))
            }
            0x54 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.read(HDMA4))
            }
            0x55 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.read(HDMA5))
            }
            0x68 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Bcps)))
            }
            0x69 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Bcpd)))
            }
            0x6a if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Ocps)))
            }
            0x6b if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Ocpd)))
            }
            0x6c if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.read(PpuRegister(Opri)))
            }
            0x70 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.work_ram.read_bank())
            }
            0x80..=0xfe => self.generic_mem_cycle(|mem| mem.high_ram.read(address)),
            0xff => self.generic_mem_cycle(|mem| mem.interrupt_controller.read(Ie)),
            _ => self.generic_mem_cycle(|_| 0xff),
        }
    }

    pub fn write(&mut self, address: u16, val: u8) {
        match address {
            0x00..=0x8ff => self.generic_mem_cycle(|mem| {
                if !mem.boot_rom.is_active() {
                    mem.cartridge.write_rom(address, val)
                }
            }),
            0x0900..=0x7fff => self.generic_mem_cycle(|mem| mem.cartridge.write_rom(address, val)),
            0x8000..=0x9fff => self.generic_mem_cycle(|mem| mem.ppu.write(Vram { address }, val)),
            0xa000..=0xbfff => self.generic_mem_cycle(|mem| mem.cartridge.write_ram(address, val)),
            0xc000..=0xcfff => self.generic_mem_cycle(|mem| mem.work_ram.write_low(address, val)),
            0xd000..=0xdfff => self.generic_mem_cycle(|mem| mem.work_ram.write_high(address, val)),
            // Echo RAM
            0xe000..=0xefff => self.generic_mem_cycle(|mem| mem.work_ram.write_low(address, val)),
            0xf000..=0xfdff => self.generic_mem_cycle(|mem| mem.work_ram.write_high(address, val)),
            0xfe00..=0xfe9f => self.generic_mem_cycle(|mem| {
                if !mem.dma_controller.is_dma_active() {
                    mem.ppu.write(Oam { address }, val)
                }
            }),
            0xfea0..=0xfeff => {
                log::warn!("Write to unusable RAM");
            }
            0xff00..=0xffff => self.write_high((address & 0xff) as u8, val),
        }
    }

    #[allow(clippy::semicolon_if_nothing_returned)]
    fn write_high(&mut self, address: u8, val: u8) {
        match address {
            0x00 => self.generic_mem_cycle(|mem| mem.joypad.write(val)),
            0x01 => self.generic_mem_cycle(|mem| mem.serial.write(SB, val)),
            0x02 => self.generic_mem_cycle(|mem| mem.serial.write(SC, val)),
            0x04 => self.timer_mem_cycle(|timer, ic| timer.write(ic, Div, val)),
            0x05 => self.timer_mem_cycle(|timer, ic| timer.write(ic, Tima, val)),
            0x06 => self.timer_mem_cycle(|timer, ic| timer.write(ic, Tma, val)),
            0x07 => self.timer_mem_cycle(|timer, ic| timer.write(ic, Tac, val)),
            0x0f => self.generic_mem_cycle(|mem| mem.interrupt_controller.write(If, val)),
            0x10 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR10), val)),
            0x11 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR11), val)),
            0x12 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR12), val)),
            0x13 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR13), val)),
            0x14 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR14), val)),
            0x16 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR21), val)),
            0x17 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR22), val)),
            0x18 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR23), val)),
            0x19 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR24), val)),
            0x1a => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR30), val)),
            0x1b => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR31), val)),
            0x1c => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR32), val)),
            0x1d => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR33), val)),
            0x1e => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR34), val)),
            0x20 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR41), val)),
            0x21 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR42), val)),
            0x22 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR43), val)),
            0x23 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR44), val)),
            0x24 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR50), val)),
            0x25 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR51), val)),
            0x26 => self.apu_mem_cycle(|apu| apu.write(ApuRegister(NR52), val)),
            0x30..=0x3f => self.apu_mem_cycle(|apu| apu.write(WavePatternRam { address }, val)),
            0x40 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Lcdc), val)),
            0x41 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Stat), val)),
            0x42 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Scy), val)),
            0x43 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Scx), val)),
            0x44 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Ly), val)),
            0x45 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Lyc), val)),
            0x46 => self.generic_mem_cycle(|mem| mem.dma_controller.write(Dma, val)),
            0x47 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Bgp), val)),
            0x48 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Obp0), val)),
            0x49 => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Obp1), val)),
            0x4a => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Wy), val)),
            0x4b => self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Wx), val)),
            0x4c if self.model == Model::Cgb && self.boot_rom.is_active() && val == 0x4 => {
                self.function_mode = FunctionMode::Compatibility;
            }
            0x4d if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.speed_switch_register.write(val))
            }
            0x4f if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.write(VramBank, val))
            }
            0x50 => {
                self.tick_t_cycle();
                if val & 0b1 != 0 {
                    self.boot_rom.deactivate();
                }
            }
            0x51 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.write(HDMA1, val))
            }
            0x52 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.write(HDMA2, val))
            }
            0x53 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.write(HDMA3, val))
            }
            0x54 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.write(HDMA4, val))
            }
            0x55 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.dma_controller.write(HDMA5, val))
            }
            0x68 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Bcps), val))
            }
            0x69 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Bcpd), val))
            }
            0x6a if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Ocps), val))
            }
            0x6b if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Ocpd), val))
            }
            0x6c if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.ppu.write(PpuRegister(Opri), val))
            }
            0x70 if self.model == Model::Cgb => {
                self.generic_mem_cycle(|mem| mem.work_ram.write_bank(val))
            }
            0x80..=0xfe => self.generic_mem_cycle(|mem| mem.high_ram.write(address, val)),
            0xff => self.generic_mem_cycle(|mem| mem.interrupt_controller.write(Ie, val)),
            _ => self.tick_t_cycle(),
        }
    }
}
