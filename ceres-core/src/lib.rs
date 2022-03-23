#![no_std]
#![forbid(unsafe_code)]
#![warn(clippy::all)]

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

use core::time::Duration;
use cpu::Cpu;
use memory::Memory;

pub use audio::{AudioCallbacks, Frame, Sample};
pub use boot_rom::BootRom;
pub use cartridge::{Cartridge, HeaderInfo};
pub use error::Error;
pub use joypad::Button;
pub use video::PixelData;
pub use video::{SCANLINES_PER_FRAME, SCREEN_HEIGHT, SCREEN_WIDTH};

// 59.7 fps
pub const NANOSECONDS_PER_FRAME: u64 = 16_750_418;
pub const FRAME_DURATION: Duration = Duration::from_nanos(NANOSECONDS_PER_FRAME);
pub const T_CYCLES_PER_SECOND: u32 = 0x0040_0000; // 2^22
pub const M_CYCLES_PER_SECOND: u32 = T_CYCLES_PER_SECOND / 4;
pub const T_CYCLES_PER_FRAME: u32 = 70224;
pub const M_CYCLES_PER_FRAME: u32 = T_CYCLES_PER_FRAME / 4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Dmg, // Game Boy
    Mgb, // Game Boy Pocket
    Cgb,
}

pub struct Gameboy<AR: AudioCallbacks> {
    cpu: Cpu<AR>,
}

impl<AR: AudioCallbacks> Gameboy<AR> {
    pub fn new(
        model: Model,
        cartridge: Cartridge,
        boot_rom: Option<BootRom>,
        audio_renderer: AR,
    ) -> Self {
        let some_boot_rom = boot_rom.is_some();
        let memory = Memory::new(model, cartridge, boot_rom, audio_renderer);
        let cpu = Cpu::new(model, some_boot_rom, memory);

        Self { cpu }
    }

    pub fn press(&mut self, button: Button) {
        self.cpu.mut_memory().press(button);
    }

    pub fn release(&mut self, button: Button) {
        self.cpu.mut_memory().release(button);
    }

    pub fn audio_callbacks(&self) -> &AR {
        self.cpu.memory().audio_callbacks()
    }

    pub fn mut_audio_callbacks(&mut self) -> &mut AR {
        self.cpu.mut_memory().mut_audio_callbacks()
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

    #[must_use]
    pub fn cartridge_header_info(&self) -> &HeaderInfo {
        self.cpu.memory().cartridge().header_info()
    }

    pub fn cartridge(&self) -> &Cartridge {
        self.cpu.cartridge()
    }
}
