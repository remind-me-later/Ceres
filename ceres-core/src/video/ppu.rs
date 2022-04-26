pub mod mode;
pub mod register;
mod registers;
mod scanline_renderer;

pub use self::{mode::Mode, register::PpuRegister};
use super::{
    palette::MonochromePaletteColors, pixel_data::PixelData, sprites::ObjectAttributeMemory,
    vram::Vram,
};
use crate::{
    interrupts::{Interrupt, Interrupts},
    memory::FunctionMode,
};
use bitflags::bitflags;
use registers::{Lcdc, Registers, Stat};

bitflags! {
   pub struct BgAttributes: u8{
        const PALETTE_NUMBER   = 0b0000_0111;
        const VRAM_BANK_NUMBER = 0b0000_1000;
        const X_FLIP           = 0b0010_0000;
        const Y_FLIP           = 0b0100_0000;
        const BG_TO_OAM_PR     = 0b1000_0000;
    }
}

#[derive(Clone, Copy)]
pub enum PpuIO {
    PpuRegister(PpuRegister),
    Vram { address: u16 },
    VramBank,
    Oam { address: u16 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PixelPriority {
    SpritesOnTop,
    BackgroundOnTop,
    Normal,
}

pub struct Ppu {
    registers: Registers,
    monochrome_palette_colors: MonochromePaletteColors,
    vram: Vram,
    oam: ObjectAttributeMemory,
    cycles: i16,
    pixel_data: PixelData,
    frame_used_window: bool,
    scanline_used_window: bool,
    window_lines_skipped: u16,
    is_frame_done: bool,
    do_render: bool,
}

impl Ppu {
    pub fn new(monochrome_palette_colors: MonochromePaletteColors) -> Self {
        let registers = Registers::new();
        let cycles = registers.stat().mode().cycles(0);

        Self {
            registers,
            monochrome_palette_colors,
            vram: Vram::new(),
            oam: ObjectAttributeMemory::new(),
            pixel_data: PixelData::new(),
            cycles,
            frame_used_window: false,
            window_lines_skipped: 0,
            scanline_used_window: false,
            is_frame_done: false,
            do_render: true,
        }
    }

    pub fn do_render(&mut self) {
        self.do_render = true
    }

    pub fn dont_render(&mut self) {
        self.do_render = false
    }

    pub fn mut_pixel_data(&mut self) -> &mut PixelData {
        &mut self.pixel_data
    }

    pub fn reset_frame_done(&mut self) {
        self.is_frame_done = false;
    }

    pub fn is_frame_done(&self) -> bool {
        self.is_frame_done
    }

    pub fn read(&mut self, io: PpuIO) -> u8 {
        let mode = self.registers.stat().mode();

        match io {
            PpuIO::PpuRegister(register) => self.registers.read(register),
            PpuIO::Vram { address } => match mode {
                Mode::DrawingPixels => 0xff,
                _ => self.vram.read(address),
            },
            PpuIO::VramBank => self.vram.read_bank_number(),
            PpuIO::Oam { address } => match mode {
                Mode::OamScan | Mode::DrawingPixels => 0xff,
                _ => self.oam.read(address as u8),
            },
        }
    }

    pub fn write(&mut self, io: PpuIO, val: u8) {
        let mode = self.registers.stat().mode();

        match io {
            PpuIO::PpuRegister(register) => self.registers.write(register, val, &mut self.cycles),
            PpuIO::Vram { address } => match mode {
                Mode::DrawingPixels => (),
                _ => self.vram.write(address, val),
            },
            PpuIO::VramBank => self.vram.write_bank_number(val),
            PpuIO::Oam { address } => match mode {
                Mode::OamScan | Mode::DrawingPixels => (),
                _ => self.oam.write(address as u8, val),
            },
        }
    }

    pub fn hdma_write(&mut self, address: u16, val: u8) {
        let mode = self.registers.stat().mode();

        match mode {
            Mode::DrawingPixels => (),
            _ => self.vram.write(address, val),
        }
    }

    pub fn dma_write(&mut self, address: u8, val: u8) {
        self.oam.write(address, val)
    }

    fn switch_mode(&mut self, mode: Mode, interrupt_controller: &mut Interrupts) {
        self.registers.mut_stat().set_mode(mode);
        let scx = self.registers.scx();
        self.cycles += mode.cycles(scx);
        let stat = self.registers.stat();

        match mode {
            Mode::OamScan => {
                if stat.contains(Stat::OAM_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                self.scanline_used_window = false;
            }
            Mode::VBlank => {
                interrupt_controller.request(Interrupt::VBLANK);

                if stat.contains(Stat::VBLANK_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                if stat.contains(Stat::OAM_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }

                self.window_lines_skipped = 0;
                self.frame_used_window = false;
            }
            Mode::DrawingPixels => (),
            Mode::HBlank => {
                if stat.contains(Stat::HBLANK_INTERRUPT) {
                    interrupt_controller.request(Interrupt::LCD_STAT);
                }
            }
        }
    }

    pub fn mode(&self) -> Mode {
        self.registers.stat().mode()
    }

    pub fn tick(
        &mut self,
        interrupt_controller: &mut Interrupts,
        function_mode: FunctionMode,
        mus_elapsed: u8,
    ) {
        if !self.registers.lcdc().contains(Lcdc::LCD_ENABLE) {
            return;
        }

        self.cycles -= i16::from(mus_elapsed);
        let stat = self.registers.stat();

        if self.cycles > 0 {
            return;
        }

        match stat.mode() {
            Mode::OamScan => self.switch_mode(Mode::DrawingPixels, interrupt_controller),
            Mode::DrawingPixels => {
                if self.do_render {
                    self.draw_scanline(function_mode);
                }
                self.switch_mode(Mode::HBlank, interrupt_controller);
            }
            Mode::HBlank => {
                let ly = self.registers.mut_ly();
                *ly += 1;
                if *ly < 144 {
                    self.switch_mode(Mode::OamScan, interrupt_controller);
                } else {
                    self.switch_mode(Mode::VBlank, interrupt_controller);
                }
                self.check_compare_interrupt(interrupt_controller);
            }
            Mode::VBlank => {
                let ly = self.registers.mut_ly();
                *ly += 1;
                if *ly > 153 {
                    *ly = 0;
                    self.switch_mode(Mode::OamScan, interrupt_controller);
                    self.is_frame_done = true;
                } else {
                    let scx = self.registers.scx();
                    self.cycles += self.registers.stat().mode().cycles(scx);
                }
                self.check_compare_interrupt(interrupt_controller);
            }
        };
    }

    fn check_compare_interrupt(&mut self, interrupt_controller: &mut Interrupts) {
        if self.registers.is_on_coincidence_scanline() {
            self.registers.mut_stat().insert(Stat::LY_EQUALS_LYC);
            if self
                .registers
                .stat()
                .contains(Stat::LY_EQUALS_LYC_INTERRUPT)
            {
                interrupt_controller.request(Interrupt::LCD_STAT);
            }
        } else {
            self.registers.mut_stat().remove(Stat::LY_EQUALS_LYC);
        }
    }
}
