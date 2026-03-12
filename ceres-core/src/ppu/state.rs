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
/// Total duration: 176 ticks (88 cycles).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OamScanStage {
    /// OAM scan and setup.
    /// Duration: 168 ticks (State 35 + 6 + 7 + 8).
    Running { tick: u16 },
    /// State 10: Mode 3 transition part 1.
    /// Duration: 4 ticks (2 cycles).
    Transition1 { remaining: u8 },
    /// State 32: Mode 3 transition part 2 (CGB palettes blocked).
    /// Duration: 4 ticks (2 cycles).
    Transition2 { remaining: u8 },
}

impl Default for OamScanStage {
    fn default() -> Self {
        Self::Running { tick: 0 }
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
    /// Duration: 2 ticks (1 cycle).
    StatUpdate { remaining: u8 },
    /// State 33: CGB palettes blocked (non-double-speed only).
    /// Duration: 4 ticks (2 cycles).
    PalettesBlock { remaining: u8 },
    /// State 36: CGB palettes unblocked, HDMA trigger check.
    /// Duration: 4 ticks (2 cycles).
    PalettesUnblock { remaining: u8 },
    /// State 11: Main HBlank wait period.
    /// Duration: Variable.
    Remainder,
    /// State 31: Pre-end, set mode_for_interrupt = 2 for next line.
    /// Duration: 4 ticks (2 cycles).
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
