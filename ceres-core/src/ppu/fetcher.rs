/// Fetcher state machine states.
///
/// The background/window fetcher retrieves tile data from VRAM and pushes
/// 8 pixels to the FIFO. Each step takes 2 T-cycles (1 M-cycle).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FetcherState {
    /// Read tile index from tilemap (cycle 1 of 2).
    #[default]
    GetTile,
    /// Read low byte of tile data (cycle 1 of 2).
    GetDataLow,
    /// Read high byte of tile data (cycle 1 of 2).
    GetDataHigh,
    /// Attempt to push 8 pixels to FIFO (repeats until FIFO has space).
    Push,
}

/// Sprite fetcher state machine states.
///
/// When a sprite is encountered during rendering, the background fetcher pauses
/// and the sprite fetcher retrieves sprite tile data.
#[expect(
    dead_code,
    reason = "Prepared for future sprite fetcher implementation"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpriteFetcherState {
    /// Idle - not currently fetching a sprite.
    #[default]
    Idle,
    /// Read sprite tile index (from OAM entry).
    GetTile,
    /// Read low byte of sprite tile data.
    GetDataLow,
    /// Read high byte of sprite tile data.
    GetDataHigh,
    /// Push sprite pixels to OAM FIFO.
    Push,
}

#[expect(
    dead_code,
    reason = "Prepared for future sprite fetcher implementation"
)]
impl SpriteFetcherState {
    /// Advances to the next sprite fetcher state.
    #[inline]
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Idle => Self::Idle,
            Self::GetTile => Self::GetDataLow,
            Self::GetDataLow => Self::GetDataHigh,
            Self::GetDataHigh => Self::Push,
            Self::Push => Self::Idle,
        }
    }
}
