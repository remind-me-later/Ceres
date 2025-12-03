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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpriteFetcherState {
    /// Idle - not currently fetching a sprite.
    #[default]
    Idle,
    /// Wait for BG fetcher alignment (fetcher must be past GetDataHigh and FIFO not empty).
    WaitForBgFetcher,
    /// Read sprite tile index and flags from OAM (2 cycles).
    GetTileAndFlags,
    /// Read low byte of sprite tile data (2 cycles).
    GetDataLow,
    /// Read high byte of sprite tile data (1 cycle), then push to FIFO.
    GetDataHighAndPush,
}

impl SpriteFetcherState {
    /// Advances to the next sprite fetcher state.
    #[inline]
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Idle => Self::Idle,
            Self::WaitForBgFetcher => Self::GetTileAndFlags,
            Self::GetTileAndFlags => Self::GetDataLow,
            Self::GetDataLow => Self::GetDataHighAndPush,
            Self::GetDataHighAndPush => Self::Idle,
        }
    }
}
