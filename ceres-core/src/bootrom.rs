use crate::Model;

const DMG0_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/dmg0.bin");
const DMG_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/dmg.bin");
const MGB_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/mgb.bin");
const SGB_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/sgb.bin");
const SGB2_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/sgb2.bin");
const CGB_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/cgb.bin");
const CGB0_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/cgb0.bin");
const CGB_E_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/cgbE.bin");
const AGB_BOOTROM: &[u8] = include_bytes!("../../external/gb-bootroms/bin/agb.bin");

pub(crate) struct Bootrom {
    data: &'static [u8],
    is_enabled: bool,
}

impl Bootrom {
    pub(crate) const fn disable(&mut self) {
        self.is_enabled = false;
    }

    pub(crate) const fn enable(&mut self) {
        self.is_enabled = true;
    }

    pub(crate) const fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub(crate) const fn new(model: Model) -> Self {
        let data = match model {
            Model::Dmg0 => DMG0_BOOTROM,
            Model::DmgB => DMG_BOOTROM,
            Model::Mgb => MGB_BOOTROM,
            Model::Sgb => SGB_BOOTROM,
            Model::Sgb2 => SGB2_BOOTROM,
            Model::CgbE => CGB_E_BOOTROM,
            Model::Cgb0 => CGB0_BOOTROM,
            Model::CgbA | Model::CgbB | Model::CgbC | Model::CgbD => CGB_BOOTROM,
            Model::Agb => AGB_BOOTROM,
        };
        Self {
            data,
            is_enabled: true,
        }
    }

    pub(crate) fn read(&self, addr: u16) -> Option<u8> {
        self.is_enabled.then(|| self.data[addr as usize])
    }
}
