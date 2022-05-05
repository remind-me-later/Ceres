mod addresses;
mod dma;
mod hdma;
mod hram;
mod key1;
mod wram;

use self::{dma::Dma, hdma::Hdma, key1::Key1};
use super::{cartridge::Cartridge, interrupts::Interrupts, timer::Timer};
use crate::{
    audio::Apu,
    boot_rom::BootRom,
    joypad::Joypad,
    serial::Serial,
    video::{ppu::Ppu, PixelData},
    AudioCallbacks, Button, Model,
};
use alloc::rc::Rc;
use core::cell::RefCell;
use hram::Hram;
use wram::Wram;

#[derive(Clone, Copy)]
pub enum FunctionMode {
    Monochrome,
    Compatibility,
    Color,
}

pub struct Memory {
    cart: Rc<RefCell<Cartridge>>,
    ints: Interrupts,
    timer: Timer,
    hram: Hram,
    wram: Wram,
    ppu: Ppu,
    joypad: Joypad,
    apu: Apu,
    serial: Serial,
    dma: Dma,
    hdma: Hdma,
    boot_rom: BootRom,
    model: Model,
    pub key1: Key1,
    in_double_speed: bool,
    function_mode: FunctionMode,
}

impl Memory {
    pub fn new(
        model: Model,
        cartridge: Rc<RefCell<Cartridge>>,
        boot_rom: BootRom,
        audio_renderer: Rc<RefCell<dyn AudioCallbacks>>,
    ) -> Self {
        let function_mode = match model {
            Model::Dmg | Model::Mgb => FunctionMode::Monochrome,
            Model::Cgb => FunctionMode::Color,
        };

        Self {
            ints: Interrupts::new(),
            timer: Timer::new(),
            cart: cartridge,
            hram: Hram::new(),
            wram: Wram::new(),
            ppu: Ppu::new(),
            joypad: Joypad::new(),
            apu: Apu::new(audio_renderer),
            serial: Serial::new(),
            dma: Dma::new(),
            hdma: Hdma::new(),
            boot_rom,
            model,
            in_double_speed: false,
            key1: Key1::empty(),
            function_mode,
        }
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

    pub fn mut_pixel_data(&mut self) -> &mut PixelData {
        self.ppu.mut_pixel_data()
    }

    pub fn press(&mut self, button: Button) {
        self.joypad.press(&mut self.ints, button);
    }

    pub fn release(&mut self, button: Button) {
        self.joypad.release(button);
    }

    pub fn interrupt_controller(&self) -> &Interrupts {
        &self.ints
    }

    pub fn mut_interrupt_controller(&mut self) -> &mut Interrupts {
        &mut self.ints
    }

    pub fn tick_t_cycle(&mut self) {
        self.emulate_dma();
        self.emulate_hdma();
        self.tick_ppu();
        self.timer.tick_t_cycle(&mut self.ints);
        self.tick_apu();
    }

    fn tick_ppu(&mut self) {
        let mus_elapsed = self.mus_since_last_tick();
        self.ppu
            .tick(&mut self.ints, self.function_mode, mus_elapsed);
    }

    fn mus_since_last_tick(&self) -> u8 {
        if self.in_double_speed {
            2
        } else {
            4
        }
    }

    fn tick_apu(&mut self) {
        let mus_elapsed = self.mus_since_last_tick();
        self.apu.tick(mus_elapsed);
    }

    fn emulate_hdma(&mut self) {
        if self.hdma.start(&self.ppu) {
            while !self.hdma.is_transfer_done() {
                let transfer = self.hdma.transfer();
                let addr = transfer.src;
                let val = match addr >> 8 {
                    0x00..=0x7f => self.cart.borrow_mut().read_rom(addr),
                    // TODO: should copy garbage
                    0x80..=0x9f => 0xff,
                    0xa0..=0xbf => self.cart.borrow_mut().read_ram(addr),
                    0xc0..=0xcf => self.wram.read_ram(addr),
                    0xd0..=0xdf => self.wram.read_bank_ram(addr),
                    _ => panic!("Illegal source addr for HDMA transfer"),
                };
                self.ppu.hdma_write(transfer.dst, val);

                // tick
                self.emulate_dma();
                self.tick_ppu();
                self.timer.tick_t_cycle(&mut self.ints);
                self.tick_apu();
            }
        }
    }

    // FIXME: sprites are not displayed during OAM DMA
    fn emulate_dma(&mut self) {
        if let Some(src) = self.dma.emulate() {
            let val = match src >> 8 {
                0x00..=0x7f => self.cart.borrow_mut().read_rom(src),
                0x80..=0x9f => self.ppu.read_vram(src),
                0xa0..=0xbf => self.cart.borrow_mut().read_ram(src),
                0xc0..=0xcf | 0xe0..=0xef => self.wram.read_ram(src),
                0xd0..=0xdf | 0xf0..=0xff => self.wram.read_bank_ram(src),
                _ => panic!("Illegal source addr for OAM DMA transfer"),
            };

            self.ppu.dma_write((src & 0xff) as u8, val);
        }
    }
}
