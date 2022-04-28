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
    pub fn read(&self, address: u16) -> Option<u8> {
        if self.is_mapped {
            Some(self.boot_rom[address as usize])
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_mapped(&self) -> bool {
        self.is_mapped
    }

    pub fn unmap(&mut self) {
        self.is_mapped = false;
    }
}
