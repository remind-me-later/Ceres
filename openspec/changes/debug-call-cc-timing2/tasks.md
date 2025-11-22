## 1. Collect Execution Trace

- [x] 1.1 Run `test_mooneye_call_cc_timing2` with trace collection enabled to capture full execution
- [x] 1.2 Run `test_mooneye_call_cc_timing` (passing test) with trace collection for comparison
- [x] 1.3 Collect traces at points where tests execute `CALL cc, nn` instructions

## 2. Analyze Traces

- [x] 2.1 Identify all `CALL cc, nn` instructions executed in both test traces
- [x] 2.2 Compare cycle counts and register states at each conditional call
- [x] 2.3 Look for discrepancies in timing when condition is true vs false
- [x] 2.4 Check if issue is related to stack operations, PC updates, or M-cycle timing
- [x] 2.5 Implemented optimized tracing (ring buffer + PC filtering) reducing trace from 300MB to 200KB
- [x] 2.6 Discovered CPU writes to OAM were not blocked during DMA startup delay

## 3. Compare with Reference Implementation

- [x] 3.1 Review Pan Docs documentation for `CALL cc, nn` timing specifications
- [x] 3.2 Check SameBoy implementation of `call_cc_a16` for timing differences
- [x] 3.3 Compare our implementation in `ceres-core/src/sm83.rs:636-645` with reference (CALL timing verified correct)
- [x] 3.4 Look for edge cases in condition evaluation timing
- [x] 3.5 Verified SameBoy DMA blocks OAM immediately via `is_enabled()`, not `is_active()`

## 4. Identify Root Cause

- [x] 4.1 Document the specific timing issue: OAM blocking inconsistency and batched DMA transfers
- [x] 4.2 Determine if issue is in `call_cc_a16`, `do_call`, or `satisfies_branch_condition` (CALL timing verified correct)
- [x] 4.3 Issue is in DMA implementation: `memory/dma.rs` and OAM blocking logic
- [x] 4.4 Two bugs identified:
  - Bug 1 (FIXED): `write_oam()` used `is_active()` instead of `is_enabled()`
  - Bug 2 (REMAINING): DMA batches transfers instead of 1 byte per M-cycle

## 5. Implement Fix

- [x] 5.1 Fixed OAM blocking: Changed `write_oam()` to use `dma.is_enabled()` in `memory/mod.rs:332`
- [x] 5.2 Added detailed tracing to `dma.rs` and `oam.rs` showing blocking status
- [x] 5.3 Fix maintains compatibility - 42 Mooneye tests still pass, none broken
- [x] 5.4 TODO: Refactor DMA to transfer 1 byte per M-cycle instead of batching (requires cycle-accurate implementation)

## 6. Validation

- [x] 6.1 Run `test_mooneye_call_cc_timing2` - still fails (requires cycle-accurate DMA)
- [x] 6.2 Run `test_mooneye_call_cc_timing` - still passes
- [x] 6.3 Run all other conditional instruction tests - all still pass
- [x] 6.4 Run full Mooneye test suite - 42 pass, 33 ignored (no regressions from OAM blocking fix)
- [x] 6.5 Keep `#[ignore]` on timing2 tests until cycle-accurate DMA is implemented

## 8. Investigation Phase 2

- [ ] 8.1 Analyze `call_cc_timing2` failure state (registers 0x42) to understand exactly which check failed
- [ ] 8.2 Investigate PPU OAM blocking logic in `ceres-core/src/ppu/oam.rs` for mode-specific edge cases
- [ ] 8.3 Check if `CALL` instruction write timing needs adjustment relative to DMA cycles
- [ ] 8.4 Compare execution traces with SameBoy if possible (or use SameBoy source as reference for specific timing)

## 7. Documentation

- [x] 7.1 Mooneye test count remains 42/75 (timing2 tests still fail)
- [x] 7.2 Added detailed tracing comments in `dma.rs` and `oam.rs`
- [x] 7.3 Documented findings in proposal.md:
  - Fixed OAM blocking bug (is_enabled vs is_active)
  - Identified need for cycle-accurate DMA (1 byte/M-cycle)
  - Created optimized debug tests in `debug_call_cc_timing2.rs`
