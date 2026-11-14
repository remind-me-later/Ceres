use {crate::Model, alloc::borrow::Cow};

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
    data: Cow<'static, [u8]>,
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
            data: Cow::Borrowed(data),
            is_enabled: true,
        }
    }

    /// A mapped boot ROM with `data`, for the proofs.
    #[cfg(kani)]
    pub(crate) const fn from_image(data: Cow<'static, [u8]>) -> Self {
        Self {
            data,
            is_enabled: true,
        }
    }

    /// A boot ROM other than the model's own (such as one that skips the
    /// logo), for tests.
    #[cfg(feature = "debug")]
    pub(crate) fn custom(data: Box<[u8]>) -> Self {
        Self {
            data: Cow::Owned(data.into_vec()),
            is_enabled: true,
        }
    }

    /// FF50: bit 0 reads whether the boot ROM is unmapped; the other bits
    /// are driven by nothing and read 1, as the data bus is precharged.
    // @hw die: YULA YAZA, ZUFA ZADO (the netlist's `boot_rom.vhd`)
    #[must_use]
    pub(crate) const fn read_bank(&self) -> u8 {
        0xFE | !self.is_enabled as u8
    }

    /// A write of FF50 with bit 0 set unmaps the boot ROM until the next
    /// reset; a write with it clear does nothing.
    // @hw die: TEPU (dffr), clocked by TUGE; SATO
    pub(crate) const fn write_bank(&mut self, val: u8) {
        if val & 1 != 0 {
            self.disable();
        }
    }

    pub(crate) fn read(&self, addr: u16) -> Option<u8> {
        self.is_enabled
            .then(|| self.data.get(addr as usize).copied().unwrap_or(0xFF))
    }
}
