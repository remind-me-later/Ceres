# DMA Timing Fix Analysis

## Current Implementation vs SameBoy

| Feature        | Ceres (Current)                                  | SameBoy                                   |
| :------------- | :----------------------------------------------- | :---------------------------------------- |
| **Trigger**    | `write_mem` calls `dma.write`                    | Write to FF46 calls `write_high_memory`   |
| **State Init** | `Starting(8)`, `accumulator = 0`                 | `dma_cycles_modulo = 2`, `dma_cycles = 0` |
| **Timing**     | `write_cpu` calls `tick` (4 dots) _before_ write | Accounts for write M-cycle implicitly     |
| **Execution**  | `run_dma` called in `advance_dots`               | `GB_dma_run` called periodically          |

## The Timing Gap

In Ceres, `write_cpu` executes `tick_m_cycle()` (advancing time by 4 dots) _before_ performing the memory write that
triggers DMA.

1. **T=0**: `tick_m_cycle()` starts. `advance_dots(4)` runs. DMA is Inactive.
2. **T=4**: `write_mem()` runs. DMA becomes `Starting(8)`.
3. **T=8** (Next Instruction): `tick_m_cycle()` runs. `advance_dots(4)` runs. DMA `accumulator` += 4. `step()` runs.

**Result**: The 4 dots of the write instruction's M-cycle are "lost" to the DMA state machine because it was Inactive
when they were processed. The DMA effectively starts 4 dots _late_ relative to the start of the write cycle.

## Trace Discrepancy (24 dots vs 8 dots)

The trace shows a ~24 dot delay.

- Expected: 8 dots (2 M-cycles).
- Observed: ~24 dots.
- Gap: 16 dots (4 M-cycles).

This suggests that not only is the write cycle lost, but potentially the _entire instruction_ (if it was 4 M-cycles
long) or subsequent cycles are not advancing the DMA as expected. However, the "lost cycle" theory definitely accounts
for at least 4 dots of delay.

## Proposed Approaches

### Approach 1: Immediate `run_dma` (Fix 1)

Call `run_dma()` immediately after `dma.write()` in `memory/mod.rs`.

- **Pros**:
  - Ensures DMA state is processed immediately after trigger.
  - Mimics SameBoy's "start immediately" behavior.
- **Cons**:
  - Requires hacking `accumulator` to force a step (since `accumulator` is reset to 0).
  - `write_mem` is not supposed to advance time/logic, just state.

### Approach 2: Adjust Startup Delay (Fix 2)

Change `Starting(8)` to `Starting(4)` (or `Starting(0)`).

- **Pros**:
  - Simple, low-risk change.
  - Directly compensates for the "lost" M-cycle of the write instruction.
  - If we set `Starting(4)`, we acknowledge that 4 dots have "already passed" during the write cycle.
- **Cons**:
  - Magic number.
  - Doesn't fully explain the 24-dot delay (only 4 dots).

### Approach 3: Reorder `write_cpu` (Fix 3)

Move `tick_m_cycle()` to _after_ `write_mem()` in `sm83.rs`.

- **Pros**:
  - Correct causality: Write happens, _then_ time passes.
  - DMA would be active during the `advance_dots` of the write cycle.
- **Cons**:
  - High risk: Changes timing for ALL CPU writes, potentially breaking other tests.
  - Hardware accuracy: Real hardware puts address on bus, then waits.

## Recommendation

**Adopt Approach 2 (Adjust Startup Delay)** as the primary fix, potentially tuning the value to `Starting(0)` if the
delay is indeed measured from the _start_ of the write cycle.

If `Starting(8)` results in 8 dots of delay _after_ the write instruction finishes, then the total delay from the
_start_ of the write instruction (assuming 1 M-cycle write) is 4+8 = 12 dots. If the trace measures from "DMA Start
Event" (which is at `write_mem`), then we are measuring from the _end_ of the instruction.

If the trace shows 24 dots from `write_mem` to `First Transfer`, then `Starting(8)` is behaving like `Starting(24)`.
This implies `step()` is not running every M-cycle.

**Hypothesis**: The `accumulator` logic in `step()` might be flawed or `advance_dots` is called in bursts. But
`Starting(4)` is the safest first step to align with the "lost cycle" reality.

## Comparison with SameBoy

SameBoy sets `modulo = 2` (2 M-cycles). If Ceres sets `Starting(4)` (1 M-cycle), it effectively treats the current
(lost) cycle as the first wait cycle, and the next cycle as the second. This matches SameBoy's 2 M-cycle delay.

## Test Results & Regression Analysis

Applying **Approach 2** (changing `Starting(8)` to `Starting(4)`) yielded the following results:

| Test Case         | `Starting(8)` (Baseline) | `Starting(4)` (Fix) |
| :---------------- | :----------------------- | :------------------ |
| `call_timing`     | **PASS**                 | **FAIL**            |
| `call_timing2`    | **PASS**                 | **PASS**            |
| `call_cc_timing`  | **PASS**                 | **FAIL**            |
| `call_cc_timing2` | **FAIL**                 | **PASS**            |

### The Dilemma

The fix successfully resolves the timing issues for the `...2` variants of the tests but causes regressions in the
original versions. This indicates that a static adjustment to the startup delay is insufficient to cover all cases, or
that the underlying issue is more complex than a simple fixed offset.

### Trace Analysis of Regressions

- **`call_timing` (Passing with Starting(8))**:

  - DMA Start to First Transfer: **~30.26 μs (~127 dots)**.
  - This is significantly longer than the expected 8 dots.
  - The test code relies on `nops 3` and `jp hl`.

- **`call_timing` (Failing with Starting(4))**:
  - DMA Start to First Transfer: **~17.44 μs (~73 dots)**.
  - The delay reduced by ~54 dots (approx 13.5 M-cycles).

### Hypothesis Refinement

The massive discrepancy in measured dots (127 vs 73) compared to the small change in startup delay (4 dots) suggests
that **OAM Blocking** or **Bus Contention** is the dominant factor, not just the startup timer.

1. **OAM Blocking**: The PPU blocks OAM access during certain modes. If the DMA tries to start during a blocked period,
   it might be stalling (or failing to transfer) until the block is lifted.
2. **Mode Alignment**: The `Starting(4)` change shifts the DMA start time slightly. This shift might be enough to move
   the first transfer attempt from a "blocked" window to an "unblocked" window (or vice versa), causing a massive change
   in effective start time.

### Next Steps

1. **Investigate OAM Blocking Logic**: Verify if `ceres-core/src/ppu/oam.rs` correctly implements OAM blocking for DMA.
   Currently, `read_oam` returns `0xFF` if blocked, but `run_dma` in `memory/mod.rs` reads from memory (which could be
   ROM/RAM) and writes to OAM.
2. **Check `write_oam_by_dma`**: Does it respect PPU blocking?
   - Code: `self.oam.write(addr, val);` (Direct write, no checks).
   - **Finding**: DMA writes to OAM are _never_ blocked in the current implementation.
3. **Re-examine Trace**: Look for PPU Mode changes relative to DMA start. The 127-dot delay might be waiting for a
   specific PPU mode (though DMA shouldn't care if it ignores blocking).

**Correction**: If DMA writes are never blocked, then the delay must be coming from `step()` not being called, or
`advance_dots` not running.

## Deep Code Analysis: `run_dma` Consistency

### Investigation into "Inconsistent Intervals"

The previous trace analysis suggested that `run_dma` was being called at irregular intervals (10-61 dots). A code review
of `ceres-core` reveals this is likely a **measurement artifact** of using wall-clock time for traces, rather than a
logic bug.

1. **Call Site Verification**: `run_dma` is called inside `advance_dots_no_timers`.
2. **Caller Verification**: `advance_dots_no_timers` is called by `advance_dots`.
3. **Cycle Verification**: `advance_dots(4)` is called by `tick_m_cycle`.
4. **Instruction Loop**: Every instruction fetch, memory read, memory write, and internal delay calls `tick_m_cycle`.
   Even `halt` calls `tick_m_cycle`.

**Conclusion**: `run_dma` **IS** called consistently every 4 dots (1 M-cycle) of emulated time. The variance observed in
traces reflects the host CPU's execution speed and logging overhead, not the emulated timing.

### The Real Issue: Accumulator Reset & Startup Delay

The root cause of the timing dilemma lies in how `dma.write` interacts with the `accumulator`.

1. **The Write Cycle**:

   - `write_mem(FF46)` is called at the _end_ of an M-cycle (after `tick_m_cycle`).
   - `tick_m_cycle` has already called `advance_dots(4)`, adding 4 dots to the DMA `accumulator`.
   - `dma.write` sets `state = Starting(X)` and **resets `accumulator = 0`**.
   - **Result**: The 4 dots of progress from the write cycle are discarded. The DMA effectively "starts" at the
     beginning of the _next_ M-cycle.

2. **Delay Calculation**:

   - **Hardware Spec**: DMA starts 2 M-cycles (8 dots) after the write instruction finishes.
   - **With `Starting(8)`**:
     - Cycle T+1 (Next M-cycle): `acc`=4. `step` reduces delay to 4.
     - Cycle T+2: `acc`=4. `step` reduces delay to 0 and transitions to `Transferring`.
     - **First Transfer**: Cycle T+2. (Correct 2 M-cycle delay).
   - **With `Starting(4)`**:
     - Cycle T+1: `acc`=4. `step` reduces delay to 0 and transitions to `Transferring`.
     - **First Transfer**: Cycle T+1. (1 M-cycle delay - Too fast).

3. **The Dilemma**:
   - `call_cc_timing2` **PASSES** with `Starting(4)` (1-cycle delay).
   - `call_timing` **FAILS** with `Starting(4)` (likely expects 2-cycle delay).
   - This implies `call_cc_timing2` has a timing dependency that requires the DMA to be active 1 cycle earlier than
     standard behavior, OR there is another timing bug in `call cc` or interrupt handling that `Starting(4)`
     accidentally compensates for.

### Next Steps (Revised)

Since `Starting(8)` is theoretically correct but fails a specific test, and `Starting(4)` is theoretically wrong but
fixes that test (while breaking others), we must investigate why `call_cc_timing2` requires faster DMA startup.

### Tracing System Improvements

To prevent future confusion regarding timing measurements, the tracing system has been updated to use **deterministic
timestamps** based on the emulator's internal dot counter (`total_dots`) instead of wall-clock time.

- **Implementation**: Added `total_dots` to `Gb` struct, incremented in `advance_dots_no_timers`.
- **Tracing Layer**: `RingBufferLayer` now prioritizes the `sim_dots` field in trace events for the timestamp.
- **Timestamp Capture**: Added `dma_write_start_dots` to capture the exact timestamp BEFORE the write cycle advances
  time, ensuring accurate measurement of the DMA startup delay.
- **Verification**: Re-running the trace analysis with the corrected system confirms:
  - DMA transfers occur exactly every 4 dots (1 M-cycle) - **perfectly consistent**
  - `Starting(4)` produces a **4-dot (1 M-cycle)** startup delay
  - `Starting(8)` produces an **8-dot (2 M-cycle)** startup delay
  - The "inconsistent intervals" observed previously were purely measurement artifacts from wall-clock timing

### Conclusive Findings

With deterministic tracing, we can now definitively state:

1. **`run_dma` IS called consistently** every 4 dots. The irregular intervals observed in wall-clock traces were purely
   measurement artifacts caused by host CPU scheduling and logging overhead.

2. **The startup delay works as designed**: `Starting(X)` produces exactly X dots of delay between the DMA write and the
   first transfer.

3. **The regression is confirmed**: `Starting(4)` makes `call_cc_timing2` pass but breaks `call_timing`. `Starting(8)`
   makes `call_timing` pass but breaks `call_cc_timing2`.

4. **Root cause identified**: The issue is NOT in the DMA implementation itself, but in how the test timing expectations
   differ. One of the tests has incorrect timing expectations, OR there is a different bug (likely in `call cc` or
   interrupt handling) that `Starting(4)` accidentally compensates for.

## Deep Test Analysis

### Reference Emulator Behavior

**Mooneye-GB** (the ground truth emulator by the test suite author):

- DMA startup delay: **2 CPU cycles** (8 dots)
- Implementation: `requested` → `starting` (1 cycle) → `start()` + first transfer (1 cycle)
- Source: `core/src/hardware.rs`, `Peripherals::emulate_oam_dma`

**SameBoy** (gold standard for accuracy):

- DMA startup delay: **2 CPU cycles** according to documentation
- Implementation: `dma_cycles_modulo = 2` (initial offset in PPU dots)
- However, analysis suggests actual delay may be **1.5 M-cycles** (6 PPU dots):
  - Initial state: `dma_cycles_modulo = 2`, `dma_cycles = 0`
  - After 1 M-cycle: `cycles = 4 + 2 = 6`, loop executes once, first byte transferred

**Ceres (our implementation)**:

- `Starting(8)` = 2 M-cycles (8 dots) startup delay
- Matches Mooneye-GB's behavior exactly

### Test Comparison

**Both tests agree on CALL cc, nn timing**:

- M=0: Instruction decoding
- M=1: Read low byte of address (nn)
- M=2: Read high byte of address (nn)
- M=3: Internal delay
- M=4: Push PC high byte to stack
- M=5: Push PC low byte to stack

**`call_cc_timing.s` (PASSES with Starting(8))**:

- **Tests**: Reading the call **target address** (nn parameter) from OAM during DMA
- **Mechanism**: CALL instruction at OAM-2, so address bytes are in OAM
- **Critical accesses**: M=1 and M=2 read from memory being DMA'd
- **Timing alignments**: Uses `nops 2` and `nops 3`
- Round 1: Address read 1 cycle before DMA ends (sees $FF from DMA)
- Round 2: Address read after DMA ends (sees $1A from normal memory)

**`call_cc_timing2.s` (FAILS with Starting(8))**:

- **Tests**: Writing **PC to the stack** (push destination) in OAM range during DMA
- **Mechanism**: Stack pointer positioned in OAM ($FE00+)
- **Critical accesses**: M=4 and M=5 write to stack
- **Timing alignments**: Uses `nops 1`, `nops 2`, and `nops 3`
- Round 1 (nops 1): M=6 timing - expects both bytes corrupted by DMA ($81)
- Round 2 (nops 2): M=5 timing - expects high byte corrupted, low byte correct
- Round 3 (nops 3): M=4 timing - expects both bytes correct

### The Critical Insight

The 4-dot difference in startup delay (Starting(4) vs Starting(8)) **shifts when the DMA transfer completes** relative
to the CALL instruction's execution phases. This changes which memory accesses see DMA-protected memory vs normal
memory.

**With Starting(8)** (2 M-cycle delay):

- DMA completes later
- Round 2 of `call_cc_timing2` expects M=5 access to partially see normal memory
- But the DMA is still active, so it fails

**With Starting(4)** (1 M-cycle delay):

- DMA completes 1 M-cycle earlier
- Round 2's M=5 access now sees the expected memory state
- Test passes

### Resolution Path

**Definitive Finding**: Mooneye-GB (the authoritative reference emulator by the test author) uses **2 CPU cycles** (8
dots) startup delay, exactly matching our `Starting(8)` implementation.

**The Paradox**: If both our implementation and the reference use 2-cycle delay, why does `call_cc_timing2` fail while
`call_cc_timing` passes?

**Possible Explanations**:

1. **Memory Access Timing During Startup**: The 2-cycle "startup" delay in Mooneye-GB may behave differently than our
   `Starting(8)` state. Perhaps memory protection begins immediately (at cycle 0) in Mooneye-GB, whereas our
   implementation might not protect memory until after the startup completes.

2. **Bus Conflict Resolution**: During the startup delay, memory accesses by the CPU might be handled differently. The
   CPU might successfully read/write during startup, but be blocked during active transfer.

3. **Timing Granularity**: Mooneye-GB processes DMA state changes within the same cycle that calls `emulate()`,
   potentially affecting sub-cycle timing of memory protection.

4. **Stack Access Timing**: The tests use different memory access patterns (reading call target vs writing to stack).
   Perhaps stack writes have different conflict behavior than reads.

### Implementation Comparison: Mooneye-GB vs Ceres

#### Mooneye-GB Implementation

**State Machine** (in `OamDma` struct):

```
requested → starting → active (bus = Some)
  (1 cycle)  (1 cycle)   (transfer begins)
```

**Memory Protection**: Activates when `is_active()` returns true (i.e., when `bus` is `Some`)

**Order of Operations** in `emulate_oam_dma` (called every cycle):

1. **Transfer byte** if `is_active()` (calls `oam_dma.emulate()`)
2. **Activate DMA** if `starting.take()` has value (calls `oam_dma.start()`)
3. **Move to starting** if `requested.take()` has value

**Key Insight**: The transfer happens in the SAME cycle that the DMA becomes active!

- Cycle 0: Write to $FF46 → `requested = Some(val)`
- Cycle 1: `starting = Some(val)` (first startup cycle)
- Cycle 2: `start()` called (sets `bus = Some`), then IMMEDIATELY `emulate()` transfers byte 0

So the "2-cycle delay" includes the first transfer!

#### Ceres Implementation

**State Machine** (in `DmaState` enum):

```
Inactive → Starting(8) → Transferring(1)
           (2 cycles)     (first transfer)
```

**Memory Protection**: Activates when `is_enabled()` returns true (any non-Inactive state)

**Order of Operations** in `step()`:

1. **Count down startup** while in `Starting(n)` state
2. **Transition to Transferring** when startup reaches 0
3. **Transfer byte** in `Transferring` state

**Key Difference**: We count down 2 full cycles THEN transfer, whereas Mooneye-GB transfers ON the second cycle.

#### The Bug

**Mooneye-GB**: Memory protection starts when DMA becomes active (cycle 2), and the first byte is transferred in that
same cycle.

**Ceres**: Memory protection starts immediately (cycle 0 when `Starting(8)` is set), but the first byte isn't
transferred until cycle 2.

This means Ceres protects memory for the full 2-cycle startup delay, while Mooneye-GB only protects memory starting from
cycle 2 onwards. For tests that check memory access timing relative to DMA transfers, this 1-cycle difference is
critical.

### The Fix

We need to change our implementation to match Mooneye-GB's behavior:

**Option 1**: Keep `Starting(8)` but DON'T protect memory during the startup phase

- Modify `is_enabled()` to only return `true` for `Transferring` and `Finishing`
- This would allow CPU access to OAM during the 2-cycle startup

**Option 2**: Use `Starting(4)` AND protect memory immediately

- Accept that we protect memory 1 cycle earlier than hardware
- This is what accidentally makes `call_cc_timing2` pass

**Option 3**: Refactor to match Mooneye-GB's state machine more closely

- Add `Requested` and `Starting` states that don't protect memory
- Transition to `Active` and transfer the first byte in the same cycle

Option 3 is the most accurate but requires more refactoring. Option 1 is the minimal fix that preserves our current
timing model.
