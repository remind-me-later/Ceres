# Implementation Tasks

## 1. Core Infrastructure

- [ ] 1.1 Add `SerialCallback` trait to `ceres-core/src/lib.rs`
- [ ] 1.2 Update `Serial` struct with new fields (callback storage, is_master flag)
- [ ] 1.3 Add `set_serial_callback()` method to `Gb` struct
- [ ] 1.4 Update `Serial::default()` to initialize new fields

## 2. Master Mode Improvements

- [ ] 2.1 Refactor `run_master()` to use callbacks for bit exchange
- [ ] 2.2 Update disconnect behavior (default to `true`/0xFF when no callback)
- [ ] 2.3 Fix CGB timing masks for all speed modes
- [ ] 2.4 Ensure test ROM output capture still works (backward compatibility)

## 3. Slave Mode Implementation

- [ ] 3.1 Add `external_clock_pulse()` method to `Serial`
- [ ] 3.2 Update `write_sc()` to properly handle slave mode (SC bit 0 = 0)
- [ ] 3.3 Implement bit shifting for external clock
- [ ] 3.4 Handle transfer completion and interrupt request in slave mode

## 4. Register Handling

- [ ] 4.1 Update `read_sc()` to correctly expose CGB-only bits
- [ ] 4.2 Ensure `write_sc()` properly initializes transfer state
- [ ] 4.3 Verify SC bit 7 clearing on transfer completion
- [ ] 4.4 Test register read/write behavior matches hardware

## 5. Testing

- [ ] 5.1 Add unit tests for master mode with callback
- [ ] 5.2 Add unit tests for master mode without callback (disconnect)
- [ ] 5.3 Add unit tests for slave mode with external clock
- [ ] 5.4 Add tests for all CGB speed modes (8192, 16384, 262144, 524288 Hz)
- [ ] 5.5 Verify existing Blargg serial tests still pass
- [ ] 5.6 Add integration test for two-emulator communication (if feasible)
- [ ] 5.7 Test edge cases (rapid start/stop, mid-transfer register writes)

## 6. Documentation

- [ ] 6.1 Add doc comments to `SerialCallback` trait
- [ ] 6.2 Document `external_clock_pulse()` API
- [ ] 6.3 Update `serial.rs` module documentation
- [ ] 6.4 Add example callback implementation in docs
- [ ] 6.5 Document timing behavior for all modes
- [ ] 6.6 Update AGENTS.md with serial communication guidelines

## 7. Validation

- [ ] 7.1 Run full test suite (ensure no regressions)
- [ ] 7.2 Test with Blargg CPU tests (serial output)
- [ ] 7.3 Verify coverage for new code paths
- [ ] 7.4 Check `clippy` warnings resolved
- [ ] 7.5 Verify `no_std` compatibility maintained

## Dependencies

- Tasks 1.1-1.4 must complete before 2.1-2.4 (infrastructure first)
- Task 3.1-3.4 depend on 1.1-1.4 (need callback system)
- Task 5.1-5.7 depend on all implementation tasks
- Task 6.1-6.6 can be done in parallel with implementation

## Notes

- Maintain backward compatibility with test ROM output capture
- Keep `no_std` compatibility (no heap allocations in serial logic)
- Follow existing code style and patterns (especially `AudioCallback`)
- Test on both DMG and CGB models
