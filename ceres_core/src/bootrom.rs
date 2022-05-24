use crate::Model;

const DMG_BOOTROM: &[u8] = include_bytes!("../bootroms/bin/dmg_boot.bin");
const MGB_BOOTROM: &[u8] = include_bytes!("../bootroms/bin/mgb_boot.bin");
const CGB_BOOTROM: &[u8] = include_bytes!("../bootroms/bin/cgb_boot_fast.bin");

pub struct BootRom {
    boot_rom: &'static [u8],
    pub is_mapped: bool,
}

impl BootRom {
    #[must_use]
    pub fn new(model: Model) -> Self {
        let buf = match model {
            Model::Dmg => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::Cgb => CGB_BOOTROM,
        };

        Self {
            boot_rom: buf,
            is_mapped: true,
        }
    }

    #[must_use]
    pub fn read(&self, addr: u16) -> u8 {
        self.boot_rom[addr as usize]
    }
}
