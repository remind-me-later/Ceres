use crate::Model;

const DMG_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/dmg.bin");
const MGB_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/mgb.bin");
const CGB_BOOTROM: &[u8] = include_bytes!("../../gb-bootroms/bin/cgb.bin");

#[derive(Debug)]
pub struct Bootrom {
    data: &'static [u8],
    is_enabled: bool,
}

impl Bootrom {
    pub const fn new(model: Model) -> Self {
        let data = match model {
            Model::Dmg => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::Cgb => CGB_BOOTROM,
        };
        Self {
            data,
            is_enabled: true,
        }
    }

    pub const fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub const fn disable(&mut self) {
        self.is_enabled = false;
    }

    pub const fn enable(&mut self) {
        self.is_enabled = true;
    }

    pub fn read(&self, addr: u16) -> Option<u8> {
        self.is_enabled.then(|| self.data[addr as usize])
    }
}
