mod addresses;
mod dma_controller;
mod high_ram;
mod speed_switch;
mod work_ram;

use self::dma_controller::DmaController;
use super::{cartridge::Cartridge, interrupts::InterruptController, timer::Timer};
use crate::{
    audio::Apu,
    boot_rom::BootRom,
    joypad::Joypad,
    serial::Serial,
    video::{
        MonochromePaletteColors, PixelData, Ppu,
        PpuIO::{Oam, Vram},
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

pub struct Memory<AR: AudioCallbacks> {
    cartridge: Cartridge,
    interrupt_controller: InterruptController,
    timer: Timer,
    high_ram: HighRam,
    work_ram: WorkRam,
    ppu: Ppu,
    joypad: Joypad,
    apu: Apu<AR>,
    serial: Serial,
    dma_controller: DmaController,
    boot_rom: Option<BootRom>,
    model: Model,
    speed_switch_register: speed_switch::Register,
    in_double_speed: bool,
    function_mode: FunctionMode,
}

impl<'a, AR: AudioCallbacks> Memory<AR> {
    pub fn new(
        model: Model,
        cartridge: Cartridge,
        monochrome_palette_colors: MonochromePaletteColors,
        boot_rom: Option<BootRom>,
        audio_renderer: AR,
    ) -> Self {
        let function_mode = match model {
            Model::Dmg | Model::Mgb => FunctionMode::Monochrome,
            Model::Cgb => FunctionMode::Color,
        };

        Self {
            interrupt_controller: InterruptController::new(),
            timer: Timer::new(),
            cartridge,
            high_ram: HighRam::new(),
            work_ram: WorkRam::new(),
            ppu: Ppu::new(boot_rom.is_some(), monochrome_palette_colors),
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
        log::warn!("speed switch");
        self.in_double_speed = !self.in_double_speed;
    }

    pub fn speed_switch_register(&self) -> &speed_switch::Register {
        &self.speed_switch_register
    }

    pub fn mut_speed_switch_register(&mut self) -> &mut speed_switch::Register {
        &mut self.speed_switch_register
    }

    pub fn audio_callbacks(&self) -> &AR {
        self.apu.callbacks()
    }

    pub fn mut_audio_callbacks(&mut self) -> &mut AR {
        self.apu.mut_callbacks()
    }

    pub fn do_render(&mut self) {
        self.ppu.do_render()
    }

    pub fn dont_render(&mut self) {
        self.ppu.dont_render()
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

    pub fn interrupt_controller(&self) -> &InterruptController {
        &self.interrupt_controller
    }

    pub fn mut_interrupt_controller(&mut self) -> &mut InterruptController {
        &mut self.interrupt_controller
    }

    pub fn tick_t_cycle(&mut self) {
        self.emulate_oam_dma();
        self.emulate_hdma();
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

    fn emulate_hdma(&mut self) {
        let microseconds_elapsed_times_16 = self.t_cycles_to_microseconds_elapsed_times_16();

        if let Some(hdma_transfer) = self
            .dma_controller
            .emulate_hdma(&self.ppu, microseconds_elapsed_times_16)
        {
            for i in 0..hdma_transfer.length {
                let address = hdma_transfer.source_address + i;
                let val = match address >> 8 {
                    0x00..=0x7f => self.cartridge.read_rom(address),
                    // TODO: should copy garbage
                    0x80..=0x9f => self.ppu.read(Vram { address }),
                    0xa0..=0xbf => self.cartridge.read_ram(address),
                    0xc0..=0xcf | 0xe0..=0xef => self.work_ram.read_low(address),
                    0xd0..=0xdf | 0xf0..=0xff => self.work_ram.read_high(address),
                    _ => panic!("Illegal source address for HDMA transfer"),
                };

                self.ppu.write(
                    Vram {
                        address: hdma_transfer.destination_address,
                    },
                    val,
                );
            }
        }
    }

    fn emulate_oam_dma(&mut self) {
        if let Some(dma_source_address) = self.dma_controller.emulate_oam_dma(&self.ppu) {
            let val = match dma_source_address >> 8 {
                0x00..=0x7f => self.cartridge.read_rom(dma_source_address),
                0x80..=0x9f => self.ppu.read(Vram {
                    address: dma_source_address,
                }),
                0xa0..=0xbf => self.cartridge.read_ram(dma_source_address),
                0xc0..=0xcf | 0xe0..=0xef => self.work_ram.read_low(dma_source_address),
                0xd0..=0xdf | 0xf0..=0xff => self.work_ram.read_high(dma_source_address),
                _ => panic!("Illegal source address for OAM DMA transfer"),
            };

            self.ppu.write(
                Oam {
                    address: 0xfe00 | dma_source_address,
                },
                val,
            );
        }
    }
}
