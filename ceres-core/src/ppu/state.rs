//! PPU State Machine Definitions
//!
//! This module defines the hierarchical state machine for the PPU,
//! closely mirroring SameBoy's display.c implementation.
//!
//! Reference: SameBoy display.c state numbers are noted in comments.

/// Top-level PPU phase - what major operation is happening
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PpuPhase {
    #[default]
    LcdOff,
    /// First line after LCD turns on (special timing)
    Line0Startup(Line0Stage),
    /// Mode 2: OAM Scan (80 T-cycles)
    OamScan(OamScanStage),
    /// Mode 3: Drawing (variable, 172+ T-cycles)
    Drawing,
    /// Mode 0: HBlank (variable, ~204 T-cycles)
    HBlank(HBlankStage),
    /// Mode 1: VBlank (lines 144-152)
    VBlank(VBlankStage),
    /// Line 153: Special handling
    Line153(Line153Stage),
}

/// Line 0 startup after LCD enable
/// SameBoy states: 23, 2, 34, 37, 38
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line0Stage {
    /// State 2: Initial Mode 0 (76 cycles)
    InitialMode0 { remaining: u8 },
    /// State 34: OAM write blocked (2 cycles)
    OamWriteBlock { remaining: u8 },
    /// State 37: STAT = Mode 3 (2 cycles)
    StatMode3 { remaining: u8 },
    /// State 38: CGB palettes blocked (3 cycles)
    PalettesBlock { remaining: u8 },
}

/// Mode 2: OAM Scan stages
/// SameBoy states: 35, 6, 7, 5, 8, 10, 32
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OamScanStage {
    /// State 35: OAM write blocked (2 cycles, CGB only)
    Entry { remaining: u8 },
    /// State 6: LY update, OAM read blocked (1 cycle)
    LyUpdate,
    /// State 7: STAT = Mode 2 (1 cycle)
    StatUpdate,
    /// State 8: Scan loop (80 cycles, 40 entries × 2)
    Scan { entry: u8, sub_cycle: u8 },
    /// State 10: Transition to Mode 3 (3 cycles)
    Transition1 { remaining: u8 },
    /// State 32: CGB palettes blocked (2 cycles)
    Transition2 { remaining: u8 },
}

/// Mode 0: HBlank stages
/// SameBoy states: 22, 33, 36, 11, 31
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HBlankStage {
    /// State 22: STAT = Mode 0 (1 cycle)
    StatUpdate,
    /// State 33: CGB palettes transition (2 cycles)
    PalettesTransition1 { remaining: u8 },
    /// State 36: Final palettes unblock (2 cycles)
    PalettesTransition2 { remaining: u8 },
    /// State 11: HBlank remainder (variable)
    Remainder { remaining: i16 },
    /// State 31: End of HBlank (2 cycles)
    End { remaining: u8 },
}

/// Mode 1: VBlank stages (lines 144-152)
/// SameBoy states: 26, 12, 24, 13
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VBlankStage {
    /// State 26: ly_for_comparison = -1 (2 cycles)
    LycReset { remaining: u8 },
    /// State 12: LY update (2 cycles)
    LyUpdate { remaining: u8 },
    /// State 24: ly_for_comparison update (1 cycle)
    LycUpdate,
    /// State 13: Line remainder
    Remainder { remaining: i16 },
}

/// Line 153 special handling
/// SameBoy states: 19, 14, 15, 16, 29, 17
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line153Stage {
    /// State 19: ly_for_comparison = -1 (2 cycles)
    LycReset { remaining: u8 },
    /// State 14: LY = 153 (2-4 cycles, model dependent)
    Ly153 { remaining: u8 },
    /// State 15: LY = 0 (2-4 cycles, model dependent)
    Ly0 { remaining: u8 },
    /// State 16: ly_for_comparison transition (4 cycles)
    LycTransition { remaining: u8 },
    /// State 29: LYC side effect window (12 cycles)
    LycSideEffect { remaining: u8 },
    /// State 17: Remainder
    Remainder { remaining: i16 },
}
