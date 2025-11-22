# Change: Fix Timer Reloading Behavior

## Why

The current timer implementation in `ceres-core` does not correctly emulate the 4-cycle delay that occurs when the TIMA
register overflows and is reloaded from TMA. This inaccuracy causes the Mooneye acceptance tests `tma_write_reloading`,
`tima_write_reloading`, and `tima_reload` to fail (or they would fail if enabled). Correct emulation of this behavior is
required for high accuracy and passing the Mooneye test suite.

## What Changes

- Implement a 4-cycle delay state for TIMA reloading.
- During the reload delay, TIMA will read as 0x00.
- Handle writes to TIMA during the reload delay (writes are ignored).
- Handle writes to TMA during the reload delay (the new value is used for the reload).
- Update `ceres-core/src/timing.rs` to reflect these hardware behaviors.

## Comparison with SameBoy and Mooneye GB

Both SameBoy and Mooneye GB correctly implement the TIMA reload state machine/logic to match hardware behavior.

| Feature                      | SameBoy / Mooneye GB / Hardware                                              | Current Ceres Implementation                             |
| :--------------------------- | :--------------------------------------------------------------------------- | :------------------------------------------------------- |
| **Reload Timing**            | TIMA overflows to 0x00, stays 0x00 for 4 cycles, then reloads TMA.           | TIMA reloads TMA immediately upon overflow.              |
| **Read TIMA during reload**  | Returns 0x00.                                                                | Returns the reloaded value (TMA).                        |
| **Write TMA during reload**  | Updates TMA and immediately updates TIMA with the new value.                 | Updates TMA only; TIMA retains the old TMA value.        |
| **Write TIMA during reload** | Specific writes are ignored (or handled specially) to match hardware quirks. | Writes are always accepted, overwriting the timer value. |

## Impact

- Affected specs: `cpu-timing`
- Affected code: `ceres-core/src/timing.rs`
