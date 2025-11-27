# PPU Design Study

## 1. PPU Modes and Transitions

The Game Boy PPU operates in four modes during a scanline (456 T-cycles total):

### Mode 2: OAM Scan (80 cycles)
*   **Duration:** Fixed at **80 cycles** in SameBoy.
*   **Operation:** Scans OAM memory (0xFE00-0xFE9F) to find sprites visible on the current line.
*   **Timing:**
    *   Iterates 40 times (once per OAM entry).
    *   Each iteration takes 2 cycles.
    *   Checks `Y` coordinate against current `LY`.
    *   If visible, adds to `visible_objs` list.
*   **SameBoy Implementation:**
    *   Explicitly sleeps for 80 cycles total in `GB_display_run`.
    *   Loop iterates 40 times with `GB_SLEEP(2)`.

### Mode 3: Drawing (Variable duration, Min 172 cycles)
*   **Duration:** Variable, depends on SCX, Sprites, and Window.
*   **Operation:** Fetches background/window tiles and sprites, mixes them, and pushes to LCD.
*   **Base Duration:** 172 cycles.
*   **Exit Condition:** PPU stays in Mode 3 until all 160 pixels are rendered (or more accurately, when the state machine completes the line).
*   **Penalties:** See Section 3.

### Mode 0: HBlank (Remainder of scanline)
*   **Duration:** 456 - 80 (Mode 2) - Mode 3 Duration.
*   **Operation:** PPU is idle/accessible.
*   **Transitions:**
    *   From Mode 3 -> Mode 0 when line rendering is complete.
    *   From Mode 0 -> Mode 2 (Next Line) or Mode 1 (VBlank) when 456 cycles elapsed.

### Mode 1: VBlank (Lines 144-153)
*   **Duration:** 4560 cycles (10 lines * 456 cycles).
*   **Operation:** Vertical blanking period.

---

## 2. Pixel FIFO and Fetcher Architecture

SameBoy implements a detailed fetcher state machine.

### Fetcher States (2 cycles each step)
1.  **Get Tile Index (T1/T2):**
    *   Calculates address in Tile Map (0x9800/0x9C00).
    *   Reads Tile Index.
    *   (CGB only) Reads Attributes.
2.  **Get Data Low (T1/T2):**
    *   Calculates address in Tile Data (0x8000/0x8800).
    *   Reads lower byte of tile row.
3.  **Get Data High (T1/T2):**
    *   Reads higher byte of tile row.
4.  **Push (T1/T2):**
    *   Pushes 8 pixels to Background FIFO.
    *   Only if FIFO is empty (or has space?). SameBoy pushes only if `bg_fifo` is empty.

### Pixel FIFO
*   **BG FIFO:** Holds 8 pixels (background/window).
*   **OAM FIFO:** Holds 8 pixels (sprites).
*   **Mixing:** `render_pixel_if_possible` pops from both and mixes based on priority.

---

## 3. Mode 3 Timing Details & Penalties

### Base Duration
*   Minimum: 172 cycles.

### SCX Penalty (Scroll X)
*   **Mechanism:** Pixel discarding.
*   **SameBoy:**
    *   `position_in_line` starts at negative value (-16?).
    *   Pixels are popped and discarded until alignment matches `SCX & 7`.
    *   Specifically: `167 + (SCX & 7)` cycles logic found in `mode3_batching_length`.
    *   BUT in cycle-accurate loop, it seems to check `(position_in_line & 7) == (scx & 7)`.
*   **Pan Docs:** Says "SCX % 8" penalty.
*   **Conclusion:** `SCX & 7` cycles are added to Mode 3.

### Window Penalty
*   **Mechanism:** Fetcher restart?
*   **SameBoy:**
    *   If `WX` triggers, `cycles_for_line += 1`.
    *   Also triggers a fetcher restart/switch to Window Map.
*   **Pan Docs:** Mention 6 cycle penalty + restart.

### Sprite Penalty
*   **Mechanism:** Fetcher interruption.
*   **SameBoy:**
    *   For each visible sprite at current X:
        *   **OAM Read:** 2 cycles.
        *   **VRAM Read 0:** 2 cycles.
        *   **VRAM Read 1:** 1 cycle. (Wait, logic showed +2 then +1?)
        *   Total per sprite: ~5-6 cycles?
        *   Plus alignment/restart cost?
    *   **Penalty Formula:** `11 - min((x + scx % 8) % 8, 5)` (from design doc/comments).
    *   **Observation:** My tests passed closer to 6 cycles per sprite + alignment.

---

## 4. Alignment with Ceres Implementation

### Current Issues
1.  **Mode 2 Duration:** Was 80, tried 81, 76. SameBoy says FIXED 80.
    *   **Action:** Ceres must strictly use 80 dots for Mode 2.
2.  **Mode 3 Base Delay:** `mode3_delay`.
    *   SameBoy uses `GB_SLEEP` delays (3 + 2 cycles?) at start of Mode 3.
    *   This accounts for "pipeline setup".
    *   **Action:** Re-verify exact startup delay.
3.  **Sprite Penalty:**
    *   Ceres tried various formulas.
    *   SameBoy's 6-11 cycle range implies:
        *   Fixed cost (fetching) + Variable cost (alignment).
        *   Alignment depends on `SCX` and `Sprite X`.
    *   **Action:** Implement fetcher-based penalty or robust formula.

### Next Steps
1.  Set OAM Scan to 80.
2.  Implement SCX handling strictly via pixel discarding (or accurate duration calc).
3.  Implement Window penalty.
4.  Refine Sprite penalty using `(11 - min)` formula OR fetcher emulation.

