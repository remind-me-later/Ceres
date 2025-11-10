# Change: Debug MBC3 Bank Switching Using Trace

## Why

The `add-sm83-disassembler` proposal has been fully implemented, providing execution tracing with the `--trace` flag.
Now we need to use this capability to debug the MBC3 bank switching behavior by analyzing the mbc3-tester ROM execution.

The mbc3-tester ROM is a 4MB ROM that tests MBC3 bank switching by attempting to read from all 128 ROM banks (255 with
MBC30). It displays visual feedback showing which banks were successfully accessed. Currently, we don't know if our MBC3
implementation correctly handles bank switching.

## Source Code

The test ROM source code is available at: https://github.com/EricKirschenmann/MBC3-Tester-gb

This repository contains:

- Assembly source code for the MBC3 bank switching test
- Reference screenshots for DMG and CGB modes
- Documentation on expected behavior

This change uses the disassembler's trace functionality to:

- Capture execution trace of the mbc3-tester ROM
- Analyze bank switching patterns (writes to $2000-$3FFF for ROM bank selection)
- Compare with expected behavior from the disassembled source code
- Identify any incorrect bank switching behavior
- Fix MBC3 implementation issues if found
- Add integration test once MBC3 is validated

## What Changes

This is an investigation and debugging task that will:

1. **Capture Execution Trace**: Run mbc3-tester with `--trace` flag and capture ~10,000-20,000 instructions
2. **Analyze Bank Switching**: Look for writes to MBC3 control registers:
   - $0000-$1FFF: RAM enable
   - $2000-$3FFF: ROM bank number (bits 0-6)
   - $4000-$5FFF: RAM bank number or RTC register select
   - $6000-$7FFF: Latch clock data
3. **Compare Screenshots**: Check if emulator output matches reference screenshots (dmg/cgb)
4. **Fix Issues**: If bank switching is incorrect, fix the MBC3 implementation
5. **Add Integration Test**: Once working, add test to `ceres-test-runner/tests/`

## Impact

- **Change Type**: Investigation/Debugging with validation test
- Affected specs: `integration-tests` (adds MBC3 validation test)
- Spec deltas: `specs/integration-tests/spec.md` (adds MBC3 bank switching validation requirements)
- Affected code (potentially):
  - `ceres-core/src/cartridge/mbc3.rs` (if bugs found)
  - `ceres-test-runner/tests/` (new integration test)
- No breaking changes expected
- May uncover MBC3 bugs that need fixing

**Note**: This is primarily a debugging task to validate existing MBC3 implementation. If bugs are found, fixes will
restore spec-compliant behavior. The integration test validates existing MBC3 bank switching requirements from the Pan
Docs specification.

## Deliverables

1. Execution trace analysis document (findings)
2. Fixed MBC3 implementation (if issues found)
3. Integration test for mbc3-tester ROM
4. Unblocks `add-mbc3-tester-test` proposal

## Dependencies

- **Requires**: `add-sm83-disassembler` (complete) - provides `--trace` flag
- **Enables**: `add-mbc3-tester-test` - can proceed after MBC3 is validated
