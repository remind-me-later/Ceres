//! Access-conflict classes for writes to PPU/CPU-visible I/O registers.
//!
//! These mirror SameBoy's `GB_CONFLICT_*` handling: a register write does not
//! always land at the end of its M-cycle, and some registers are written in
//! two steps. The maps are selected by hardware (`GB_is_cgb`), not by the
//! mode a ROM runs in, exactly like SameBoy.

use crate::{
    Model,
    memory::{BGP, HRAM_START, IF, IO_START, LCDC, LYC, NR10, OBP0, OBP1, SCX, SCY, STAT, WX, WY},
};

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
    map[IF as usize] = ConflictType::WriteCpu;
    map[LCDC as usize] = ConflictType::DmgLcdc;
    map[STAT as usize] = ConflictType::StatDmg;
    map[SCY as usize] = ConflictType::ScyDmg;
    map[SCX as usize] = ConflictType::ScxDmgAndCgbDouble;
    map[LYC as usize] = ConflictType::ReadOld;
    map[BGP as usize] = ConflictType::PaletteDmg;
    map[OBP0 as usize] = ConflictType::PaletteDmg;
    map[OBP1 as usize] = ConflictType::PaletteDmg;
    map[WY as usize] = ConflictType::ReadOld;
    map[WX as usize] = ConflictType::WxDmg;
    map
};

pub const SGB_CONFLICT_MAP: [ConflictType; 128] = {
    let mut map = [ConflictType::ReadOld; 128];
    map[IF as usize] = ConflictType::WriteCpu;
    map[LCDC as usize] = ConflictType::SgbLcdc;
    map[STAT as usize] = ConflictType::StatDmg;
    map[SCY as usize] = ConflictType::ScyDmg;
    map[SCX as usize] = ConflictType::ScxDmgAndCgbDouble;
    map[LYC as usize] = ConflictType::ReadOld;
    map[BGP as usize] = ConflictType::ReadNew;
    map[OBP0 as usize] = ConflictType::ReadNew;
    map[OBP1 as usize] = ConflictType::ReadNew;
    map[WY as usize] = ConflictType::ReadOld;
    map[WX as usize] = ConflictType::WxDmg;
    map
};

pub const CGB_CONFLICT_MAP: [ConflictType; 128] = {
    let mut map = [ConflictType::ReadOld; 128];
    map[IF as usize] = ConflictType::WriteCpu;
    map[LCDC as usize] = ConflictType::LcdcCgb;
    map[STAT as usize] = ConflictType::StatCgb;
    map[SCX as usize] = ConflictType::ReadOld;
    map[LYC as usize] = ConflictType::WriteCpu;
    map[BGP as usize] = ConflictType::PaletteCgb;
    map[OBP0 as usize] = ConflictType::PaletteCgb;
    map[OBP1 as usize] = ConflictType::PaletteCgb;
    map[WY as usize] = ConflictType::ReadOld;
    map[WX as usize] = ConflictType::WriteCpu;
    map
};

pub const CGB_DOUBLE_CONFLICT_MAP: [ConflictType; 128] = {
    let mut map = [ConflictType::ReadOld; 128];
    map[IF as usize] = ConflictType::WriteCpu;
    map[NR10 as usize] = ConflictType::Nr10CgbDouble;
    map[LCDC as usize] = ConflictType::LcdcCgbDouble;
    map[STAT as usize] = ConflictType::StatCgbDouble;
    map[SCX as usize] = ConflictType::ScxDmgAndCgbDouble;
    map[LYC as usize] = ConflictType::ReadOld;
    map[WY as usize] = ConflictType::ReadOld;
    map[WX as usize] = ConflictType::ReadOld;
    map
};

#[must_use]
#[expect(
    clippy::module_name_repetitions,
    reason = "Reads better at the call sites than a bare `get`"
)]
pub const fn get_conflict(model: Model, double_speed: bool, addr: u16) -> ConflictType {
    if addr < IO_START || addr >= HRAM_START {
        return ConflictType::ReadOld;
    }

    let offset = (addr - IO_START) as usize;

    // Up to the CGB-C the STAT and LYC writes land at once: their timing is
    // in the PPU's STAT interrupt events.
    if matches!(model, Model::Cgb0 | Model::CgbA | Model::CgbB | Model::CgbC)
        && (offset == STAT as usize || offset == LYC as usize)
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
