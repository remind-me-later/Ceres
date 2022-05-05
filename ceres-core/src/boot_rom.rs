use alloc::boxed::Box;

pub struct BootRom {
    boot_rom: Box<[u8]>,
    is_mapped: bool,
}

impl BootRom {
    #[must_use]
    pub fn new(data: Box<[u8]>) -> Self {
        Self {
            boot_rom: data,
            is_mapped: true,
        }
    }

    #[must_use]
    pub fn read(&self, addr: u16) -> Option<u8> {
        self.is_mapped.then(|| self.boot_rom[addr as usize])
    }

    #[must_use]
    pub fn is_mapped(&self) -> bool {
        self.is_mapped
    }

    pub fn unmap(&mut self) {
        self.is_mapped = false;
    }
}
