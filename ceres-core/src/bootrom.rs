use crate::Model;

const DMG_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/dmg.bin");
const MGB_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/mgb.bin");
const CGB_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/cgb.bin");
const CGB_E_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/cgbE.bin");

pub struct Bootrom {
    data: &'static [u8],
    is_enabled: bool,
}

impl Bootrom {
    pub const fn disable(&mut self) {
        self.is_enabled = false;
    }

    pub const fn enable(&mut self) {
        self.is_enabled = true;
    }

    pub const fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub const fn new(model: Model) -> Self {
        let data = match model {
            Model::DmgB => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::CgbE => CGB_E_BOOTROM,
            Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD => CGB_BOOTROM,
        };
        Self {
            data,
            is_enabled: true,
        }
    }

    pub fn read(&self, addr: u16) -> Option<u8> {
        self.is_enabled.then(|| self.data[addr as usize])
    }
}
