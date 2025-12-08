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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OamScanStage {
    Entry { remaining: u8 },
    LyUpdate { remaining: u8 },
    StatUpdate { remaining: u8 },
    Scan { entry: u8, sub_cycle: u8 },
    Transition1 { remaining: u8 },
    Transition2 { remaining: u8 },
}

impl Default for OamScanStage {
    fn default() -> Self {
        Self::Entry { remaining: 4 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HBlankStage {
    StatUpdate { remaining: u8 },
    PalettesTransition1 { remaining: u8 },
    PalettesTransition2 { remaining: u8 },
    Remainder { remaining: u16 },
    End { remaining: u8 },
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