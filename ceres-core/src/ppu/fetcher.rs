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
///
/// SameBoy sprite fetch sequence:
/// - State 27 (WaitForBgFetcher): Wait until fetcher_state >= 5 AND fifo > 0
/// - State 41 (ExtraAdvance): 1 cycle, advances BG fetcher
/// - "Free" advance: Happens at start of State 20, no cycle cost
/// - State 20 (GetTileAndFlags): 2 cycles, OAM read
/// - State 39 (GetDataLow): 2 cycles, VRAM low byte
/// - State 40 (GetDataHighAndPush): 1 cycle, VRAM high byte + overlay
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpriteFetcherState {
    /// Idle - not currently fetching a sprite.
    #[default]
    Idle,
    /// SameBoy State 27: Wait for BG fetcher alignment.
    /// Loops while (fetcher_state < 5 || fifo_size == 0), advancing fetcher each cycle.
    WaitForBgFetcher,
    /// SameBoy State 20: OAM read (2 cycles).
    /// First cycle does the "free" advance from after State 41.
    GetTileAndFlags,
    /// SameBoy State 39: VRAM low byte read (2 cycles).
    GetDataLow,
    /// SameBoy State 40: VRAM high byte read (1 cycle), then overlay to OAM FIFO.
    GetDataHighAndPush,
}
