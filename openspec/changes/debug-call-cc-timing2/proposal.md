# Change: Debug Mooneye test_mooneye_call_cc_timing2 failure

## Why

The Mooneye tests `test_mooneye_call_cc_timing2` and `test_mooneye_call_timing2` are currently failing with all CPU
registers containing 0x42 (Mooneye failure code). These tests use OAM DMA to verify exact M-cycle timing of CALL
instructions by detecting when memory operations occur relative to DMA transfers.

Investigation revealed that the basic CALL instruction timing is **correct** (as proven by passing `call_timing` and
`call_cc_timing` tests), but the timing2 tests fail because they rely on precise OAM DMA behavior that doesn't match
hardware. The tests set SP to OAM memory and use DMA writes as timing sensors to detect the exact M-cycle when PUSH
operations occur.

## Root Cause Analysis

### Test Methodology

The timing2 tests use a clever technique:

1. Start OAM DMA from $8000 to $FE00-$FE9F (160 bytes, 1 byte per M-cycle)
2. Set SP to OAM+$20 ($FE20) where CALL will push return address
3. Carefully time the CALL to execute while DMA is writing to specific addresses
4. Pop the pushed values to check which bytes were corrupted by DMA vs written by CPU

From test source (call_cc_timing2.s):

```text
; CALL cc, nn is expected to have the following timing:
; M = 0: instruction decoding
; M = 1: nn read: memory access for low byte
; M = 2: nn read: memory access for high byte
; M = 3: internal delay
; M = 4: PC push: memory access for high byte
; M = 5: PC push: memory access for low byte
```

The test verifies:

- Round 1 (nops 1): OAM accessible at M=6 → both bytes corrupted by DMA
- Round 2 (nops 2): OAM accessible at M=5 → high byte corrupted, low byte correct
- Round 3 (nops 3): OAM accessible at M=4 → both bytes correct

### Investigation Findings

1. **CPU Timing Verified Correct**:

   - Implemented proper M-cycle timing for CALL: M=1,2 (read nn), M=3 (internal delay), M=4,5 (push PC)
   - Refactored `push()` into `push_raw()` (no delay) and `push()` (with M=1 delay for PUSH instruction)
   - Changed `do_call()` to use `push_raw()` since M=3 delay already consumed
   - Result: `call_timing` and `call_cc_timing` tests PASS

2. **OAM DMA Implementation Issue**:

   - DMA has 2 M-cycle startup delay (-8 dots in code, matches SameBoy's `dma_cycles_modulo = 2`)
   - DMA transfers 1 byte per M-cycle (160 M-cycles total)
   - Timing2 tests still FAIL with all registers 0x42
   - **Conclusion**: OAM DMA timing/behavior doesn't match hardware behavior

3. **Probable DMA Issues**:
   - Exact cycle when DMA starts relative to $FF46 write
   - How DMA blocks/unblocks OAM access during transfer
   - Interaction between DMA writes and CPU writes to same OAM addresses
   - The "warmup" phase behavior (dma_current_dest states in SameBoy)

## What Changes

- ~~Add trace collection capability~~ (not needed - analysis complete)
- ~~Analyze execution trace~~ (completed - found CPU timing was correct)
- ~~Fix CALL instruction timing~~ (completed - CALL timing verified correct)
- **Investigate OAM DMA implementation** (next step)
- **Compare DMA behavior with SameBoy** (cycle-by-cycle comparison needed)
- **Fix OAM DMA timing/access blocking** (specific issue TBD)
- Verify fix by confirming timing2 tests pass
- Un-ignore passing tests in mooneye_tests.rs
- Update AGENTS.md to reflect 44 passing tests (from 42)

## Impact

- Affected specs: N/A (investigation/debugging complete for CPU, moving to DMA)
- Affected code:
  - `ceres-core/src/sm83.rs` - ✅ FIXED: Refactored push timing, do_call now uses push_raw
  - `ceres-core/src/memory/dma.rs` - **NEEDS FIX**: OAM DMA timing/behavior mismatch
  - `ceres-core/src/ppu/oam.rs` - May need updates to OAM access blocking logic
  - `ceres-test-runner/tests/mooneye_tests.rs` - Remove #[ignore] from timing2 tests once fixed
  - `AGENTS.md` - Update passing test count from 42 to 44
- Risk: Low-Medium - DMA fix may affect other DMA-dependent tests, requires careful validation
