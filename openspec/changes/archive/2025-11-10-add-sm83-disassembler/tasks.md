# Implementation Tasks

## Overview

This change is divided into **two sub-proposals** that should be implemented sequentially:

1. `add-disassembler-core` - Core disassembly functionality
2. `add-disassembler-cli` - CLI integration for execution tracing

Each sub-proposal has its own detailed tasks file and can be validated independently.

## High-Level Milestones

- [x] 1. Create and implement `add-disassembler-core` sub-proposal
- [x] 2. Validate core disassembler with unit tests
- [x] 3. Create and implement `add-disassembler-cli` sub-proposal
- [x] 4. Test CLI integration with mbc3-tester ROM
- [x] 5. Create `debug-mbc3-bank-switching` proposal to use the disassembler
- [ ] 6. Complete MBC3 debugging proposal (separate work item)
- [ ] 7. Unblock `add-mbc3-tester-test` proposal after MBC3 validation

## Dependencies

- No external dependencies required
- Sub-proposals must be implemented in order (#2 depends on #1)
- Both maintain `no_std` compatibility in `ceres-core`

## Success Criteria

- ✅ Can disassemble all SM83 instructions correctly
- ✅ CLI flag enables execution tracing with register state
- ⏸️ Can identify where MBC3 bank switching fails in mbc3-tester ROM (deferred)
- ✅ Zero regressions in existing tests

## Status

**COMPLETE** - Both sub-proposals have been fully implemented and archived:

- `add-disassembler-core` (archived 2025-11-10): Core disassembly module with all 512 opcodes
- `add-disassembler-cli` (archived 2025-11-10): CLI integration with `--trace` flag

### Implementation Summary

The SM83 disassembler is now fully functional with:

1. **Core Module** (`ceres-core/src/disasm/mod.rs`):

   - All 256 base opcodes and 256 CB-prefixed opcodes supported
   - RGBDS-compatible output format
   - `no_std` compatible using `heapless::String<32>`
   - `Gb::disasm_at()` method for runtime disassembly

2. **CLI Integration**:

   - `--trace` flag in all frontends (winit, gtk, egui)
   - Real-time instruction tracing with register state
   - Output format: `[PC:$XXXX] MNEMONIC ; A=XX F=ZNHC BC=XXXX DE=XXXX HL=XXXX SP=XXXX`

3. **Additional Features** (from trace-collection proposal):
   - `--trace-enable` flag for trace buffer collection
   - `--trace-buffer-size N` to configure buffer size
   - `--trace-export FILE` to export traces as JSON
   - Trace analysis tools in `ceres-test-runner/analyze_trace.py`

### MBC3 Debugging Status

Task 5 created a separate proposal: `debug-mbc3-bank-switching`

The actual MBC3 bank switching debugging work has been moved to its own proposal because:

1. The disassembler tool is complete and ready for use
2. MBC3 debugging is a distinct investigation/debug/fix cycle
3. The debugging work should be tracked separately from the tool implementation
4. Note: `rtc3test` validates RTC functionality (clock), not ROM bank switching

### Next Steps

The disassembler capability is complete and ready for:

- ✅ Debugging any Game Boy ROM with `--trace` flag
- ✅ Adding the mbc3-tester integration test (separate proposal)
- ✅ Analyzing execution traces with Python tools
- ✅ General-purpose CPU debugging
