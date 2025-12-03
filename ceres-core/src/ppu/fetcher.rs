/// Fetcher state machine states.
///
/// The background/window fetcher retrieves tile data from VRAM and pushes
/// 8 pixels to the FIFO. Uses T1/T2 sub-states matching SameBoy:
/// - T1: Calculate addresses/setup
/// - T2: Perform VRAM read
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FetcherState {
    /// Read tile index from tilemap - T1: calculate address.
    #[default]
    GetTileT1,
    /// Read tile index from tilemap - T2: read VRAM.
    GetTileT2,
    /// Read low byte of tile data - T1: calculate address.
    GetDataLowT1,
    /// Read low byte of tile data - T2: read VRAM.
    GetDataLowT2,
    /// Read high byte of tile data - T1: calculate address.
    GetDataHighT1,
    /// Read high byte of tile data - T2: read VRAM.
    GetDataHighT2,
    /// Attempt to push 8 pixels to FIFO - T1 (wait for space).
    PushT1,
    /// Attempt to push 8 pixels to FIFO - T2 (push if space, else stay).
    PushT2,
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
