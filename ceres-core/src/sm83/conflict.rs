//! Access-conflict classes for writes to PPU/CPU-visible I/O registers.
//!
//! These mirror SameBoy's `GB_CONFLICT_*` handling: a register write does not
//! always land at the end of its M-cycle, and some registers are written in
//! two steps. The maps are selected by hardware (`GB_is_cgb`), not by the
//! mode a ROM runs in, exactly like SameBoy.

use crate::{CgbMode, Model};

#[expect(
    clippy::module_name_repetitions,
    reason = "Named after SameBoy's `conflict_t`; it is used outside this module"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConflictType {
    #[default]
    ReadOld,
    ReadNew,
    WriteCpu,
    StatCgb,
    StatCgbDouble,
    StatDmg,
    PaletteDmg,
    PaletteCgb,
    DmgLcdc,
    SgbLcdc,
    WxDmg,
    LcdcCgb,
    LcdcCgbDouble,
    Nr10CgbDouble,
    ScxDmgAndCgbDouble,
    ScyDmg,
}

pub const DMG_CONFLICT_MAP: [ConflictType; 128] = {
    let mut map = [ConflictType::ReadOld; 128];
    map[0x0F] = ConflictType::WriteCpu;
    map[0x40] = ConflictType::DmgLcdc;
    map[0x41] = ConflictType::StatDmg;
    map[0x42] = ConflictType::ScyDmg;
    map[0x43] = ConflictType::ScxDmgAndCgbDouble;
    map[0x45] = ConflictType::ReadOld;
    map[0x47] = ConflictType::PaletteDmg;
    map[0x48] = ConflictType::PaletteDmg;
    map[0x49] = ConflictType::PaletteDmg;
    map[0x4A] = ConflictType::ReadOld;
    map[0x4B] = ConflictType::WxDmg;
    map
};

pub const SGB_CONFLICT_MAP: [ConflictType; 128] = {
    let mut map = [ConflictType::ReadOld; 128];
    map[0x0F] = ConflictType::WriteCpu;
    map[0x40] = ConflictType::SgbLcdc;
    map[0x41] = ConflictType::StatDmg;
    map[0x42] = ConflictType::ScyDmg;
    map[0x43] = ConflictType::ScxDmgAndCgbDouble;
    map[0x45] = ConflictType::ReadOld;
    map[0x47] = ConflictType::ReadNew;
    map[0x48] = ConflictType::ReadNew;
    map[0x49] = ConflictType::ReadNew;
    map[0x4A] = ConflictType::ReadOld;
    map[0x4B] = ConflictType::WxDmg;
    map
};

pub const CGB_CONFLICT_MAP: [ConflictType; 128] = {
    let mut map = [ConflictType::ReadOld; 128];
    map[0x0F] = ConflictType::WriteCpu;
    map[0x40] = ConflictType::LcdcCgb;
    map[0x41] = ConflictType::StatCgb;
    map[0x43] = ConflictType::ReadOld;
    map[0x45] = ConflictType::WriteCpu;
    map[0x47] = ConflictType::PaletteCgb;
    map[0x48] = ConflictType::PaletteCgb;
    map[0x49] = ConflictType::PaletteCgb;
    map[0x4A] = ConflictType::ReadOld;
    map[0x4B] = ConflictType::WriteCpu;
    map
};

pub const CGB_DOUBLE_CONFLICT_MAP: [ConflictType; 128] = {
    let mut map = [ConflictType::ReadOld; 128];
    map[0x0F] = ConflictType::WriteCpu;
    map[0x10] = ConflictType::Nr10CgbDouble;
    map[0x40] = ConflictType::LcdcCgbDouble;
    map[0x41] = ConflictType::StatCgbDouble;
    map[0x43] = ConflictType::ScxDmgAndCgbDouble;
    map[0x45] = ConflictType::ReadOld;
    map[0x4A] = ConflictType::ReadOld;
    map[0x4B] = ConflictType::ReadOld;
    map
};

#[must_use]
#[expect(
    clippy::module_name_repetitions,
    reason = "Reads better at the call sites than a bare `get`"
)]
pub const fn get_conflict(
    model: Model,
    _cgb_mode: CgbMode,
    double_speed: bool,
    addr: u16,
) -> ConflictType {
    if (addr & 0xFF80) != 0xFF00 {
        return ConflictType::ReadOld;
    }

    let offset = (addr & 0x7F) as usize;

    // Up to the CGB-C the STAT and LYC writes land at once: their timing is
    // in the PPU's STAT interrupt events.
    if matches!(model, Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC)
        && matches!(offset, 0x41 | 0x45)
    {
        return ConflictType::ReadOld;
    }

    if model.is_cgb_hardware() {
        if double_speed {
            CGB_DOUBLE_CONFLICT_MAP[offset]
        } else {
            CGB_CONFLICT_MAP[offset]
        }
    } else if matches!(model, Model::Sgb | Model::Sgb2) {
        SGB_CONFLICT_MAP[offset]
    } else {
        DMG_CONFLICT_MAP[offset]
    }
}
