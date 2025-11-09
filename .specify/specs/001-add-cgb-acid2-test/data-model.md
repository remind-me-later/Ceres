# Data Model: cgb-acid2 Test

## Overview

This feature adds a single integration test with minimal data structures. The test uses existing infrastructure from the
TestRunner and does not introduce new data models.

## Existing Structures Used

### TestConfig

```rust
pub struct TestConfig {
    pub capture_serial: bool,
    pub model: Model,
    pub timeout_frames: u32,
    pub expected_screenshot: Option<std::path::PathBuf>,
}
```

**Usage in this feature**:

- `capture_serial`: false (cgb-acid2 doesn't use serial output)
- `model`: Model::Cgb (Color Game Boy)
- `timeout_frames`: 600 (timeouts::CGB_ACID2)
- `expected_screenshot`: Some("test-roms/cgb-acid2/cgb-acid2.png")

### TestResult

```rust
pub enum TestResult {
    Failed(String),
    Passed,
    Timeout,
    Unknown,
}
```

**Expected values**:

- `Passed`: Screenshot matches reference image
- `Failed(msg)`: Screenshot differs or other error
- `Timeout`: Test exceeded 600 frames

## Constants

### timeouts Module

```rust
pub mod timeouts {
    pub const CPU_INSTRS: u32 = 2091;
    pub const INSTR_TIMING: u32 = 250;
    pub const MEM_TIMING: u32 = 300;
    pub const MEM_TIMING_2: u32 = 360;
    pub const INTERRUPT_TIME: u32 = 240;
    pub const HALT_BUG: u32 = 330;
    pub const CGB_ACID2: u32 = 600;  // NEW
}
```

## File References

### Test ROM

- **Path**: `test-roms/cgb-acid2/cgb-acid2.gbc`
- **Type**: Game Boy Color ROM (binary)
- **Size**: Small (~32KB typical for test ROMs)

### Reference Screenshot

- **Path**: `test-roms/cgb-acid2/cgb-acid2.png`
- **Type**: PNG image (160x144 pixels, RGBA8)
- **Purpose**: Pixel-perfect comparison target

## State Transitions

The test follows a simple linear flow:

```text
[Start]
  → Load ROM → Run emulation → Compare screenshot → [Pass/Fail/Timeout]
```

**States**:

1. **Initial**: Test function invoked by cargo test
2. **Loading**: ROM loaded via load_test_rom()
3. **Running**: Emulation executes frames (0..600)
4. **Comparing**: Screenshot compared against reference
5. **Complete**: Result returned (Passed/Failed/Timeout)

No persistent state between test runs.

## Validation Rules

1. **ROM Path**: Must exist at `test-roms/cgb-acid2/cgb-acid2.gbc`
2. **Screenshot Path**: Must exist at `test-roms/cgb-acid2/cgb-acid2.png`
3. **Timeout**: Must be > 0 frames
4. **Model**: Must be Model::Cgb (not DMG)
5. **Screenshot Match**: Byte-for-byte RGBA comparison

## Dependencies

- `ceres_test_runner::load_test_rom` - Load test ROM from file
- `ceres_test_runner::expected_screenshot_path` - Build screenshot path
- `ceres_test_runner::test_runner::TestRunner` - Execute test
- `ceres_test_runner::test_runner::TestConfig` - Configure test
- `ceres_test_runner::test_runner::TestResult` - Test outcome
- `ceres_test_runner::test_runner::timeouts` - Timeout constants

## Entity Summary

| Entity         | Type           | Purpose            | Source                |
| -------------- | -------------- | ------------------ | --------------------- |
| test_cgb_acid2 | Function       | Test entry point   | New (blargg_tests.rs) |
| CGB_ACID2      | Constant (u32) | Timeout value      | New (test_runner.rs)  |
| TestConfig     | Struct         | Test configuration | Existing              |
| TestResult     | Enum           | Test outcome       | Existing              |
| TestRunner     | Struct         | Test executor      | Existing              |
