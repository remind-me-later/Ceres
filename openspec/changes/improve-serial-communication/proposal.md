# Change: Improve Serial Communication Implementation

## Why

The current serial communication implementation in `ceres-core/src/serial.rs` is minimal and only supports basic
functionality for Blargg test ROMs. It:

- Only implements master mode (internal clock) transfers
- Has no support for external clock (slave mode) or link cable emulation
- Lacks accurate timing for the different clock speeds in CGB mode
- Does not properly handle the bit-by-bit serial transfer protocol
- Has no mechanism for external devices or two-emulator communication
- Does not implement timeout behavior or disconnect detection

According to Pan Docs and SameBoy's implementation, proper serial communication requires:

- Support for both internal (master) and external (slave) clock modes
- Accurate timing for DMG (8192 Hz) and CGB speeds (8192 Hz, 16384 Hz, 262144 Hz, 524288 Hz)
- Bit-by-bit transfer with proper shift register behavior
- Callback mechanism for external device communication
- Proper handling of disconnected state (input reads as 1/0xFF)
- Support for asynchronous external clocking

This improvement will enable proper link cable emulation, multiplayer support, and accurate behavior for games that use
serial communication beyond test ROMs.

## What Changes

- **Add external clock (slave mode) support**: Implement proper handling when `SC` bit 0 is 0
- **Add callback system for external communication**: Allow frontends to implement link cable or external device
  emulation (following SameBoy's design)
- **Improve timing accuracy**: Implement correct cycle-based timing for all clock speeds (DMG and CGB)
- **Add bit-by-bit transfer state tracking**: Track individual bit transfers with proper shift register behavior
- **Add disconnected state handling**: Default to reading 0xFF when no device is connected
- **Refactor state management**: Separate master/slave state machines for clarity
- **Add external clock API**: Provide methods for external clock pulses and bit exchange (for slave mode)
- **Update tests**: Expand serial tests to cover new functionality
- **Document callback interface**: Add clear documentation for implementing external devices

## Impact

- **Affected specs**: Creates new `serial-communication` capability specification
- **Affected code**:
  - `ceres-core/src/serial.rs` - Complete refactor with new callback system
  - `ceres-core/src/lib.rs` - Add callback trait and registration methods
  - `ceres-core/src/timing.rs` - May need serial timing integration
  - Frontend implementations (optional) - Can implement callbacks for link cable emulation
  - `ceres-test-runner/tests/serial_test.rs` - Enhanced tests for new features
- **Breaking changes**:
  - **BREAKING**: `Serial` struct will have additional fields and methods
  - **BREAKING**: Constructor/initialization may require callback registration
  - Existing test ROM functionality preserved (backward compatible for current usage)
- **Dependencies**: None (remains `no_std` compatible)
- **Performance**: Minimal impact - only active during serial transfers
- **Testing**: Requires new test cases for external clock and callback functionality
