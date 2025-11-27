# PPU Design Study

## 1. PPU Modes and Transitions

The Game Boy PPU operates in four modes during a scanline (456 T-cycles total):

### Mode 2: OAM Scan (80 cycles)
*   **Duration:** Fixed at **80 cycles** in SameBoy.
*   **Operation:** Scans OAM memory (0xFE00-0xFE9F) to find sprites visible on the current line.
*   **Implementation:**
    *   Iterates 40 times (0-39).
    *   Each iteration takes **2 T-cycles**.
    *   Checks `Y` coordinate against current `LY`.
    *   If visible, adds to `visible_objs` list.
    *   Sleeps for 2 cycles explicitly in `GB_display_run`.

### Mode 3: Drawing (Variable duration, Min 172 cycles)
*   **Duration:** Variable, depends on SCX, Sprites, and Window.
*   **Base Duration:** 172 cycles (minimum).
*   **Pipeline Setup:** 
    *   **5 T-cycles** of fixed delay before the main loop starts.
    *   Implemented as `cycles += 3` (sleep 3) then `cycles += 2` (sleep 2).
*   **Main Loop:**
    *   Runs until `lcd_x` reaches 160.
    *   Each iteration typically advances 1 T-cycle (plus penalties).
    *   Calls `render_pixel_if_possible` and `advance_fetcher_state_machine`.

### Mode 0: HBlank
*   **Duration:** Remainder of scanline (456 - 80 - Mode 3 Duration).
*   **Operation:** PPU idle.

### Mode 1: VBlank
*   **Duration:** 4560 cycles (lines 144-153).

---

## 2. Fetcher State Machine

The fetcher operates in 2-cycle steps (except PUSH).

| State | Description | Duration | Logic |
| :--- | :--- | :--- | :--- |
| **GET_TILE_T1** | Calc Tile Index Addr | 1 cycle | Selects Map (9800/9C00) based on LCDC & Window. Calc `x`, `y`. |
| **GET_TILE_T2** | Read Tile Index | 1 cycle | Reads VRAM. (CGB: Reads Attributes). |
| **DATA_LOW_T1** | Calc Data Low Addr | 1 cycle | Uses Tile Index & Attributes (Flip Y, Bank). |
| **DATA_LOW_T2** | Read Data Low | 1 cycle | Reads VRAM (Byte 0). |
| **DATA_HIGH_T1** | Calc Data High Addr | 1 cycle | Address + 1. |
| **DATA_HIGH_T2** | Read Data High | 1 cycle | Reads VRAM (Byte 1). Incrs Window X if active. |
| **PUSH** | Push to FIFO | Variable | **Blocks** if FIFO is not empty. Pushes 8 px. Goto TILE_T1. |

*   **Total Cycle per Tile:** 6 cycles min (if FIFO ready) + PUSH wait.
*   **Window Trigger:**
    *   Checked in `GET_TILE_T1`.
    *   If `wx_triggered`: uses Window Map, `window_tile_x`.
    *   `window_tile_x` increments in `DATA_HIGH_T2`.

---

## 3. Pixel FIFO & Rendering

### Structures
*   **FIFO:** Capacity 8 pixels. Holds `color`, `palette`, `priority`.
*   **Mixing:** `render_pixel_if_possible` pops from BG and OAM FIFOs.

### Rendering Logic (`render_pixel_if_possible`)
1.  **Sprite Blocking:** Checks if a sprite at `x=0` is pending. If so, stalls.
2.  **Pop:** Pops BG pixel. Pops OAM pixel (if available).
3.  **SCX Handling:**
    *   `position_in_line` starts at **-16**.
    *   Pixels are popped and discarded if `position_in_line < -8`.
    *   Alignment check: `(position_in_line & 7) == (SCX & 7)`.
    *   If aligned, jumps to `-8`. This effectively consumes `SCX % 8` cycles.
4.  **Output:** If `position_in_line >= 0`, pushes to LCD buffer and increments `lcd_x`.

---

## 4. Timing Penalties (Mode 3)

### Pipeline Setup
*   **+5 cycles** fixed delay at start of Mode 3.

### SCX Penalty
*   **Mechanism:** Pixel discarding.
*   **Cost:** `SCX % 8` cycles.
*   **Implementation:** `position_in_line` starts at -16. Render loop runs but discards pixels until alignment matches SCX.

### Window Penalty
*   **Mechanism:**
    *   Fetcher restart (switch to Window Map).
    *   Extra delay?
    *   SameBoy code: `if (WX==0 && SCX&7) { cycles += 1; sleep(1); }`.
    *   Also `fifo_clear` when window activates.

### Sprite Penalty
*   **Mechanism:** Fetcher Interruption.
*   **Trigger:** When `objects_x[i] == x_for_object_match`.
*   **Sequence:**
    1.  **Stall:** Loops `advance_fetcher` until fetcher reaches specific state (`DATA_HIGH_T2`?) or FIFO empty.
    2.  **OAM Read:** 2 cycles. (`cycles += 2; sleep(2)`).
    3.  **VRAM Read 0:** 2 cycles. (`cycles += 2; sleep(2)`).
    4.  **VRAM Read 1:** 1 cycle. (`cycles += 1; sleep(1)`).
    5.  **Overlay:** calls `fifo_overlay_object_row`.
*   **Total Cost:** ~5 cycles per sprite + alignment/stall time.

---

## 5. Fractional Scrolling & "Fractional Pixels"

### SCX Handling (Revisited)
SameBoy's `render_pixel_if_possible` function contains specific logic for handling SCX alignment, which it refers to as "fractional scrolling".

*   **Initial State:** `gb->position_in_line` starts at -16.
*   **Discard Loop:**
    *   If `position_in_line < -8`:
        *   Checks alignment: `(position_in_line & 7) == (SCX & 7)`.
        *   If aligned: Jumps `position_in_line` to -8.
        *   If not aligned: Pops from FIFOs and increments `position_in_line` by 1.
    *   This mechanism effectively discards `(SCX % 8)` pixels at the start of the line.
    *   The cycle cost is implicit: the rendering loop runs for more iterations (discarding pixels) before `position_in_line` reaches 0 (start of LCD output).

### "Fractional Pixels"
*   The term "fractional pixels" appears in iOS/layout code (scaling related) and in HexFiend (UI view), which are unrelated to the PPU logic.
*   In `display.c`, `gb->line_has_fractional_scrolling` flag is set if `SCX % 8 != 0` during the discard phase.
*   This flag affects `WX` glitch logic: `gb->cgb_wx_glitch = ... || (gb->position_in_line == (uint8_t)-7 && gb->line_has_fractional_scrolling)`.

---

## 6. Implementation Plan

1.  **Mode 2:** Strict 80 cycles.
2.  **Mode 3 Setup:** Initialize `cycles` with +5.
3.  **Main Loop:** Implement `while(lcd_x < 160)`.
4.  **Fetcher:** Implement full state machine (T1/T2 states).
5.  **FIFO:** Implement blocking PUSH state.
6.  **Sprites:** Implement "interrupt" logic in main loop. When sprite matches X, pause rendering, run sprite fetch sequence (OAM->VRAM->VRAM), then resume.
7.  **SCX:** Implement pixel discarding logic in renderer:
    *   Start `position_in_line` at -16.
    *   Discard logic matching `(position_in_line & 7) == (SCX & 7)`.