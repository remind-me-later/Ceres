#[derive(PartialEq, Eq, Clone, Copy)]
pub enum PpuMode {
    AccessOam,
    AccessVram,
    HBlank,
    VBlank,
}

impl PpuMode {
    pub const fn cycles(self, scroll_x: u8) -> i16 {
        let scroll_adjust = match scroll_x % 0x08 {
            5..=7 => 2,
            1..=4 => 1,
            _ => 0,
        };
        match self {
            PpuMode::AccessOam => super::ACCESS_OAM_CYCLES,
            PpuMode::AccessVram => super::ACCESS_VRAM_CYCLES + scroll_adjust * 4,
            PpuMode::HBlank => super::HBLANK_CYCLES - scroll_adjust * 4,
            PpuMode::VBlank => super::VBLANK_LINE_CYCLES,
        }
    }
}

impl From<u8> for PpuMode {
    fn from(val: u8) -> Self {
        match val & 0x3 {
            0 => Self::HBlank,
            1 => Self::VBlank,
            2 => Self::AccessOam,
            _ => Self::AccessVram,
        }
    }
}

impl From<PpuMode> for u8 {
    fn from(val: PpuMode) -> Self {
        match val {
            PpuMode::HBlank => 0,
            PpuMode::VBlank => 1,
            PpuMode::AccessOam => 2,
            PpuMode::AccessVram => 3,
        }
    }
}
