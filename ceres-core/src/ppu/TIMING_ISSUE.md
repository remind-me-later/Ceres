# PPU 2x Timing Issue

## Status: Unresolved

**58 tests failing** out of 277 in `cargo test -p ceres-test-runner --test gambatte`.
All attempts to fix the 2x timing issue have made things worse.

## Problem

Ceres' PPU runs at **2 ticks per T-cycle** (4MHz mode) via
`PPU_CYCLES_PER_T_CYCLE = 2` in `ceres-core/src/timing/mod.rs:13`.
SameBoy's PPU state machine runs at **1 state step per T-cycle**.

Result: Ceres' mode 2 is 82 T-cycles, SameBoy's is 42 T-cycles.
Ceres is ~2x too slow in absolute PPU timing.

## What Was Tried (all made things worse)

| Attempt | Result | Net |
|---|---|---|
| `PPU_CYCLES_PER_T_CYCLE = 1` only | 58 → 89 | +31 |
| OAM scan 160→80 only | 58 → 61 | +3 |
| OAM scan + mode 3 overhead (14→7) | 58 → 61 | +3 |
| + HBlank stages (4→1) | 58 → 61 | +3 |
| + line length (912→456) | 58 → 97 | +39 |
| Full rescale (all at once) | 58 → 68 | +10 |

## Why Full Rescale Fails

SameBoy's PPU state machine has **sub-T-cycle granularity** (0.5 T-cycles):
- State 22: 0.5 T-cycles
- State 6: 0.5 T-cycles
- State 7: 0.5 T-cycles

Ceres with `PPU_CYCLES_PER_T_CYCLE = 1` (1 tick = 1 T-cycle) cannot represent
0.5 T-cycles. Merging these sub-states into single T-cycle events loses
critical timing distinctions (e.g. HBlank interrupt fires 0.5 T-cycles before
STAT mode change).

Ceres with `PPU_CYCLES_PER_T_CYCLE = 2` (1 tick = 0.5 T-cycles) preserves
sub-T-cycle granularity but all the "tick" magic numbers are scaled 2x.

## Why OAM Scan Fix Breaks Other Tests

Shortening the OAM scan loop (160→80 ticks) shifts mode 3 timing by 80 ticks
(40 T-cycles). This breaks:
- `sprites_10spritesprline_10xposa7_m3stat_1` (m3stat interrupt timing)
- `window_late_disable_early_scx03_wx{10,11,12}_1` (window disable timing)

Net: fixes 1 test (`window_m2int_wxa6_m0irq_1`), breaks 4.

## The Coupling Problem

The PPU's internal state machines are tightly coupled:
- OAM scan length → mode 3 start time → mode 3 duration → mode 0 start time
- HBlank StatUpdate timing → mode 0 STAT interrupt
- Mode 3 overhead → pixel output timing → mode 3 end time
- Line length → PreEnd timing → line transition

Changing one duration shifts all downstream timings. The test suite is
sensitive to T-cycle-precise timing in many places.

## What Would Work (but is too large for a single PR)

A full PPU rescale requires:
1. Change `PPU_CYCLES_PER_T_CYCLE` to 1.
2. Halve all internal tick values.
3. Merge sub-T-cycle sub-states (state 6+7, state 22) into single events.
4. Add a "sub-tick" counter to preserve 0.5 T-cycle granularity where needed.
5. Verify each test category iteratively.
6. Likely need SameBoy's exact state machine ported to Rust.

This is a multi-week effort, not a single PR.

## Test Impact Summary

| Category | Failing | Notes |
|---|---|---|
| TIMA | 8 | State machine 2x too slow |
| LYC | 7 | LY=LYC coincidence timing |
| Mode STAT (m0/m1/m2) | 14 | Interrupt pulse timing |
| OAM access | 11 | Block schedule off |
| SCX | 2 | Mid-mode-3 SCX writes |
| Sprites | 10 | m3stat timing |
| Window | 5 | WX trigger timing |
| HALT | 1 | wake-up timing |
| **Total** | **58** | All 277 tests in suite |

## Reference

- SameBoy PPU state machine: `external/reference-implementations/SameBoy/Core/display.c:1760-1850`
- SameBoy timer state machine: `external/reference-implementations/SameBoy/Core/timing.c:164-290`
- SameBoy state machine macros: `external/reference-implementations/SameBoy/Core/timing.h:25-54`
- Gambatte test ROMs: `external/test-roms/gambatte/`
- Failing test list: `cargo test -p ceres-test-runner --test gambatte 2>&1 | grep FAILED`
