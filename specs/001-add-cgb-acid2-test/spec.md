# Feature Specification: Add cgb-acid2 Integration Test

## Overview

Add the cgb-acid2 PPU accuracy test to the Ceres integration test suite. This test validates Color Game Boy (CGB) PPU emulation accuracy using pixel-perfect screenshot comparison.

## Background

The cgb-acid2 test is a comprehensive validation of the Game Boy Color's Pixel Processing Unit (PPU). It tests:
- Background/window tile flipping (horizontal/vertical)
- VRAM banking
- Object (sprite) rendering and priorities
- Master priority bit
- Window line counter
- 10 object per line limit
- 8x16 sprite handling

Unlike timing-critical tests, cgb-acid2 uses a simple line-based renderer and writes registers during PPU mode 2 (OAM scan). It completes by executing the `LD B, B` (opcode 0x40) instruction.

## Requirements

### Functional Requirements

1. **Test Execution**
   - Load cgb-acid2.gbc ROM from test-roms/cgb-acid2/
   - Run emulation until exit condition (opcode 0x40: LD B, B)
   - Compare final screen output against reference PNG
   - Use CGB model (not DMG)
   - Disable color correction for pixel-perfect comparison

2. **Screenshot Comparison**
   - Use formula `(X << 3) | (X >> 2)` for 5-bit to 8-bit RGB conversion
   - Compare against cgb-acid2.png reference image
   - Pixel-perfect match required for test to pass

3. **Timeout Handling**
   - Determine appropriate timeout based on test completion time
   - Test should complete quickly (no timing torture, simple renderer)
   - Estimate: <5 seconds on typical hardware

### Non-Functional Requirements

1. **Performance**: Test should complete in under 10 seconds
2. **Reliability**: Test must be deterministic (no flakiness)
3. **Integration**: Follow existing test suite patterns in blargg_tests.rs
4. **Coverage**: Maintain or improve PPU test coverage

## Technical Constraints

- Must use existing TestRunner infrastructure
- Must follow TestConfig pattern with screenshot comparison
- Must add timeout constant to timeouts module
- Test file location: test-roms/cgb-acid2/cgb-acid2.gbc
- Reference screenshot: test-roms/cgb-acid2/cgb-acid2.png

## Success Criteria

1. Test passes when PPU implementation is correct
2. Test fails with clear message when PPU has bugs
3. Test completes within timeout period
4. Screenshot comparison works correctly
5. Test integrates cleanly with existing test suite
6. CI pipeline runs test successfully

## Out of Scope

- Fixing PPU bugs (if test fails, that's expected until PPU is correct)
- Adding other acid tests (dmg-acid2, cgb-acid-hell)
- Modifying screenshot comparison algorithm
- Adding exit condition detection (test runner already uses screenshot comparison)

## Risk Assessment

**Low Risk:**
- Test infrastructure already exists
- Reference screenshot available
- Exit condition documented (though not strictly needed for screenshot-based testing)
- Test doesn't require T-cycle accuracy

**Potential Issues:**
- Test may reveal existing PPU bugs (this is desired behavior)
- Timeout value may need adjustment based on actual completion time

## References

- Test ROM: test-roms/cgb-acid2/cgb-acid2.gbc
- Documentation: test-roms/cgb-acid2/README.md
- Reference image: test-roms/cgb-acid2/cgb-acid2.png
- Exit condition: Opcode 0x40 (LD B, B)
- Color conversion: (X << 3) | (X >> 2)
