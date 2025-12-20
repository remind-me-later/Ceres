#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PpuPhase {
    #[default]
    LcdOff,
    Line0Startup(Line0Stage),
    OamScan(OamScanStage),
    Drawing,
    HBlank(HBlankStage),
    VBlank(VBlankStage),
    Line153(Line153Stage),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line0Stage {
    InitialMode0 { remaining: u16 },
    OamWriteBlock { remaining: u8 },
    StatMode3 { remaining: u8 },
    PalettesBlock { remaining: u8 },
}

impl Default for Line0Stage {
    fn default() -> Self {
        Self::InitialMode0 { remaining: 152 }
    }
}

/// OAM Scan (Mode 2) state machine.
/// SameBoy timing (all in 8MHz half-cycles, which equal ticks in Ceres):
/// - Entry (State 35): 4 ticks - OAM write blocked on CGB (non-double-speed)
/// - LyUpdate (State 6): 2 ticks - LY update, OAM read blocked
/// - StatUpdate (State 7): 2 ticks - STAT = Mode 2, OAM fully blocked
/// - Scan (State 8): 160 ticks - 40 OAM entries × 4 ticks each
/// - Transition1 (State 10): 6 ticks - Mode 3 transition, VRAM blocked
/// - Transition2 (State 32): 4 ticks - CGB palettes blocked
///
/// Total: 4 + 2 + 2 + 160 + 6 + 4 = 178 ticks (but overlaps with Mode 3 setup)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OamScanStage {
    /// State 35: OAM write blocked on CGB (non-double-speed).
    /// Duration: 4 ticks (2 8MHz cycles).
    Entry { remaining: u8 },
    /// State 6: LY update, OAM read blocked.
    /// Duration: 2 ticks (1 8MHz cycle).
    LyUpdate { remaining: u8 },
    /// State 7: STAT = Mode 2, OAM fully blocked.
    /// Duration: 2 ticks (1 8MHz cycle).
    StatUpdate { remaining: u8 },
    /// State 8: OAM scan loop.
    /// Each entry takes 4 ticks (2 8MHz cycles).
    /// On CGB: scan happens on first 2 ticks.
    /// On DMG: scan happens on last 2 ticks.
    Scan { entry: u8, sub_tick: u8 },
    /// State 10: Mode 3 transition.
    /// Duration: 6 ticks (3 8MHz cycles).
    Transition1 { remaining: u8 },
    /// State 32: CGB palettes blocked.
    /// Duration: 4 ticks (2 8MHz cycles).
    Transition2 { remaining: u8 },
}

impl Default for OamScanStage {
    fn default() -> Self {
        Self::Entry { remaining: 4 }
    }
}

/// HBlank (Mode 0) state machine.
/// SameBoy timing (in 8MHz half-cycles = ticks):
/// - StatUpdate (State 22): 2 ticks - STAT = Mode 0, memory unblocked
/// - PalettesBlock (State 33): 4 ticks - CGB palettes blocked (non-double-speed)
/// - PalettesUnblock (State 36): 4 ticks - CGB palettes unblocked, HDMA trigger
/// - Remainder (State 11): Variable - Wait for line end (912 total - cycles_for_line - 4)
/// - PreEnd (State 31): 4 ticks - Set mode_for_interrupt = 2 for next line
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HBlankStage {
    /// State 22: STAT = Mode 0, memory unblocked.
    /// Duration: 2 ticks (1 8MHz cycle).
    StatUpdate { remaining: u8 },
    /// State 33: CGB palettes blocked (non-double-speed only).
    /// Duration: 4 ticks (2 8MHz cycles).
    PalettesBlock { remaining: u8 },
    /// State 36: CGB palettes unblocked, HDMA trigger check.
    /// Duration: 4 ticks (2 8MHz cycles).
    PalettesUnblock { remaining: u8 },
    /// State 11: Main HBlank wait period.
    /// Duration: Variable (LINE_LENGTH - cycles_for_line - 4 ticks).
    Remainder,
    /// State 31: Pre-end, set mode_for_interrupt = 2 for next line.
    /// Duration: 4 ticks (2 8MHz cycles).
    PreEnd { remaining: u8 },
}

impl Default for HBlankStage {
    fn default() -> Self {
        Self::StatUpdate { remaining: 2 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VBlankStage {
    LycReset { remaining: u8 },
    LyUpdate { remaining: u8 },
    LycUpdate { remaining: u8 },
    Remainder { remaining: u16 },
}

impl Default for VBlankStage {
    fn default() -> Self {
        Self::LycReset { remaining: 4 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line153Stage {
    LycReset { remaining: u8 },
    Ly153 { remaining: u8 },
    Ly0 { remaining: u8 },
    LycTransition { remaining: u8 },
    LycSideEffect { remaining: u8 },
    Remainder { remaining: u16 },
}

impl Default for Line153Stage {
    fn default() -> Self {
        Self::LycReset { remaining: 4 }
    }
}
