## 1. Investigation

- [ ] 1.1 Run test with detailed tracing to capture DMA timing events
- [ ] 1.2 Analyze when `dma.is_enabled()` returns true vs when OAM should be inaccessible
- [ ] 1.3 Compare with SameBoy source code for DMA state transitions
- [ ] 1.4 Trace CPU instruction execution around DMA write to understand M-cycle alignment
- [ ] 1.5 Document exact failing assertions from test ROM

## 2. Root Cause Analysis

- [ ] 2.1 Verify DMA `Starting` state timing (currently 8 dots = 2 M-cycles)
- [ ] 2.2 Determine if `is_enabled()` should return false during initial M-cycle
- [ ] 2.3 Check if OAM accessibility check happens at the right time in instruction execution
- [ ] 2.4 Investigate DMA restart behavior (when previous DMA is already running)

## 3. Implementation Options

- [ ] 3.1 Option A: Modify `is_enabled()` to distinguish "starting" from "blocking OAM"
- [ ] 3.2 Option B: Add separate `is_oam_blocked()` method to DMA
- [ ] 3.3 Option C: Use a longer startup delay and track which cycle we're in
- [ ] 3.4 Document chosen approach with rationale

## 4. Code Changes

- [ ] 4.1 Update `DmaState` enum if needed (add states or modify `Starting`)
- [ ] 4.2 Modify `dma.rs` to implement chosen timing solution
- [ ] 4.3 Update `oam.rs` OAM access methods to use correct blocking check
- [ ] 4.4 Add/update tracing to verify timing behavior

## 5. Testing

- [ ] 5.1 Verify `test_mooneye_oam_dma_start` passes
- [ ] 5.2 Verify `test_mooneye_oam_dma_restart` still passes
- [ ] 5.3 Verify `test_mooneye_oam_dma_timing` still passes
- [ ] 5.4 Run all OAM DMA tests in mooneye suite
- [ ] 5.5 Verify no regressions in other test suites (blargg, gbmicro)

## 6. Documentation

- [ ] 6.1 Add code comments explaining the 1 M-cycle accessibility window
- [ ] 6.2 Reference Mooneye test and SameBoy behavior in comments
- [ ] 6.3 Update any relevant tracing documentation if timing logs change
