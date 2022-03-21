extern crate alloc;

use alloc::boxed::Box;

pub struct BootRom {
    boot_rom: Box<[u8]>,
    is_active: bool,
}

impl BootRom {
    pub fn new(data: Box<[u8]>) -> Self {
        // TODO: check it has the right size
        Self {
            boot_rom: data,
            is_active: true,
        }
    }

    pub fn read(&self, address: u8) -> u8 {
        self.boot_rom[address as usize]
    }

    pub const fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
    }
}
