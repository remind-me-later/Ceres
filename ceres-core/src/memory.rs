extern crate alloc;

mod addresses;
mod dma_controller;
mod high_ram;
mod speed_switch;
mod work_ram;

use alloc::rc::Rc;
use core::cell::RefCell;

use self::dma_controller::DmaController;
use super::{cartridge::Cartridge, interrupts::Interrupts, timer::Timer};
use crate::{
    audio::Apu,
    boot_rom::BootRom,
    joypad::Joypad,
    serial::Serial,
    video::{
        ppu::{Ppu, PpuIO::Vram},
        MonochromePaletteColors, PixelData,
    },
    AudioCallbacks, Button, Model,
};
use high_ram::HighRam;
use work_ram::WorkRam;

#[derive(Clone, Copy)]
pub enum FunctionMode {
    Monochrome,
    Compatibility,
    Color,
}

pub struct Memory {
    cartridge: Cartridge,
    interrupt_controller: Interrupts,
    timer: Timer,
    high_ram: HighRam,
    work_ram: WorkRam,
    ppu: Ppu,
    joypad: Joypad,
    apu: Apu,
    serial: Serial,
    dma_controller: DmaController,
    boot_rom: BootRom,
    model: Model,
    speed_switch_register: speed_switch::Register,
    in_double_speed: bool,
    function_mode: FunctionMode,
}

impl Memory {
    pub fn new(
        model: Model,
        cartridge: Cartridge,
        monochrome_palette_colors: MonochromePaletteColors,
        boot_rom: BootRom,
        audio_renderer: Rc<RefCell<dyn AudioCallbacks>>,
    ) -> Self {
        let function_mode = match model {
            Model::Dmg | Model::Mgb => FunctionMode::Monochrome,
            Model::Cgb => FunctionMode::Color,
        };

        Self {
            interrupt_controller: Interrupts::new(),
            timer: Timer::new(),
            cartridge,
            high_ram: HighRam::new(),
            work_ram: WorkRam::new(),
            ppu: Ppu::new(monochrome_palette_colors),
            joypad: Joypad::new(),
            apu: Apu::new(audio_renderer),
            serial: Serial::new(),
            dma_controller: DmaController::new(),
            boot_rom,
            model,
            in_double_speed: false,
            speed_switch_register: speed_switch::Register::empty(),
            function_mode,
        }
    }

    pub fn cartridge(&self) -> &Cartridge {
        &self.cartridge
    }

    pub fn reset_frame_done(&mut self) {
        self.ppu.reset_frame_done();
    }

    pub fn is_frame_done(&self) -> bool {
        self.ppu.is_frame_done()
    }

    pub fn switch_speed(&mut self) {
        self.in_double_speed = !self.in_double_speed;
    }

    pub fn speed_switch_register(&self) -> &speed_switch::Register {
        &self.speed_switch_register
    }

    pub fn mut_speed_switch_register(&mut self) -> &mut speed_switch::Register {
        &mut self.speed_switch_register
    }

    pub fn do_render(&mut self) {
        self.ppu.do_render();
    }

    pub fn dont_render(&mut self) {
        self.ppu.dont_render();
    }

    pub fn mut_pixel_data(&mut self) -> &mut PixelData {
        self.ppu.mut_pixel_data()
    }

    pub fn press(&mut self, button: Button) {
        self.joypad.press(&mut self.interrupt_controller, button);
    }

    pub fn release(&mut self, button: Button) {
        self.joypad.release(button);
    }

    pub fn interrupt_controller(&self) -> &Interrupts {
        &self.interrupt_controller
    }

    pub fn mut_interrupt_controller(&mut self) -> &mut Interrupts {
        &mut self.interrupt_controller
    }

    pub fn tick_t_cycle(&mut self) {
        self.emulate_oam_dma();
        self.emulate_vram_dma();
        self.tick_ppu();
        self.timer.tick_t_cycle(&mut self.interrupt_controller);
        self.tick_apu();
    }

    pub fn tick_ppu(&mut self) {
        let microseconds_elapsed_times_16 = self.t_cycles_to_microseconds_elapsed_times_16();
        self.ppu.tick(
            &mut self.interrupt_controller,
            self.function_mode,
            microseconds_elapsed_times_16,
        );
    }

    fn t_cycles_to_microseconds_elapsed_times_16(&self) -> u8 {
        if self.in_double_speed {
            2
        } else {
            4
        }
    }

    pub fn tick_apu(&mut self) {
        let microseconds_elapsed_times_16 = self.t_cycles_to_microseconds_elapsed_times_16();
        self.apu.tick(microseconds_elapsed_times_16);
    }

    fn emulate_vram_dma(&mut self) {
        if self.dma_controller.start_transfer(&self.ppu) {
            while !self.dma_controller.vram_dma_is_transfer_done() {
                let hdma_transfer = self.dma_controller.do_vram_transfer();
                let address = hdma_transfer.source_address;
                let val = match address >> 8 {
                    0x00..=0x7f => self.cartridge.read_rom(address),
                    // TODO: should copy garbage
                    0x80..=0x9f => 0xff,
                    0xa0..=0xbf => self.cartridge.read_ram(address),
                    0xc0..=0xcf => self.work_ram.read_low(address),
                    0xd0..=0xdf => self.work_ram.read_high(address),
                    _ => panic!("Illegal source address for HDMA transfer"),
                };
                self.ppu
                    .vram_dma_write(hdma_transfer.destination_address, val);

                // tick
                self.emulate_oam_dma();
                self.tick_ppu();
                self.timer.tick_t_cycle(&mut self.interrupt_controller);
                self.tick_apu();
            }
        }
    }

    // FIXME: sprites are not displayed during OAM DMA
    fn emulate_oam_dma(&mut self) {
        if let Some(dma_source_address) = self.dma_controller.emulate_oam_dma(&self.ppu) {
            let val = match dma_source_address >> 8 {
                0x00..=0x7f => self.cartridge.read_rom(dma_source_address),
                0x80..=0x9f => self.ppu.read(Vram {
                    // TODO: should be able to read vram at any moment?
                    address: dma_source_address,
                }),
                0xa0..=0xbf => self.cartridge.read_ram(dma_source_address),
                0xc0..=0xcf | 0xe0..=0xef => self.work_ram.read_low(dma_source_address),
                0xd0..=0xdf | 0xf0..=0xff => self.work_ram.read_high(dma_source_address),
                _ => panic!("Illegal source address for OAM DMA transfer"),
            };

            self.ppu
                .oam_dma_write((dma_source_address & 0xff) as u8, val);
        }
    }
}
