#![no_std]

extern crate alloc;

mod audio;
mod boot_rom;
mod cartridge;
mod cpu;
mod error;
mod interrupts;
mod joypad;
mod memory;
mod serial;
mod timer;
mod video;

pub use audio::{AudioCallbacks, Frame, Sample};
pub use boot_rom::BootRom;
pub use cartridge::{Cartridge, Header};
pub use error::Error;
pub use joypad::Button;
pub use video::{
    MonochromePaletteColors, PixelData, SCANLINES_PER_FRAME, SCREEN_HEIGHT, SCREEN_WIDTH,
};

use alloc::rc::Rc;
use core::{cell::RefCell, time::Duration};
use cpu::Cpu;
use memory::Memory;

// 59.7 fps
pub const NANOSECONDS_PER_FRAME: u64 = 16_750_418;
pub const FRAME_DURATION: Duration = Duration::from_nanos(NANOSECONDS_PER_FRAME);
pub const T_CYCLES_PER_SECOND: u32 = 0x0040_0000; // 2^22
pub const M_CYCLES_PER_SECOND: u32 = T_CYCLES_PER_SECOND / 4;
pub const T_CYCLES_PER_FRAME: u32 = 70224;
pub const M_CYCLES_PER_FRAME: u32 = T_CYCLES_PER_FRAME / 4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Dmg,
    Mgb,
    Cgb,
}

pub struct Gameboy {
    cpu: Cpu,
}

impl Gameboy {
    pub fn new(
        model: Model,
        cartridge: Rc<RefCell<Cartridge>>,
        boot_rom: BootRom,
        audio_callbacks: Rc<RefCell<dyn AudioCallbacks>>,
        monochrome_palette_colors: MonochromePaletteColors,
    ) -> Self {
        Self {
            cpu: Cpu::new(Memory::new(
                model,
                cartridge,
                monochrome_palette_colors,
                boot_rom,
                audio_callbacks,
            )),
        }
    }

    pub fn press(&mut self, button: Button) {
        self.cpu.mut_memory().press(button);
    }

    pub fn release(&mut self, button: Button) {
        self.cpu.mut_memory().release(button);
    }

    pub fn mut_pixel_data(&mut self) -> &mut PixelData {
        self.cpu.mut_memory().mut_pixel_data()
    }

    pub fn run_frame(&mut self) {
        self.cpu.mut_memory().do_render();

        while !self.cpu.memory().is_frame_done() {
            self.cpu.execute_instruction();
        }

        self.cpu.mut_memory().reset_frame_done();
    }

    pub fn run_frame_but_dont_render(&mut self) {
        self.cpu.mut_memory().dont_render();

        while !self.cpu.memory().is_frame_done() {
            self.cpu.execute_instruction();
        }

        self.cpu.mut_memory().reset_frame_done();
    }
}
