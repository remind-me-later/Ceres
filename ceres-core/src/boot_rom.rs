use alloc::boxed::Box;

pub struct BootRom {
    boot_rom: Box<[u8]>,
    is_mapped: bool,
}

impl BootRom {
    #[must_use]
    pub fn new(data: Box<[u8]>) -> Self {
        // TODO: check it has the right size
        Self {
            boot_rom: data,
            is_mapped: true,
        }
    }

    #[must_use]
    pub fn read(&self, address: u16) -> u8 {
        self.boot_rom[address as usize]
    }

    #[must_use]
    pub fn is_mapped(&self) -> bool {
        self.is_mapped
    }

    pub fn unmap(&mut self) {
        self.is_mapped = false;
    }
}
