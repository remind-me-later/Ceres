# Test Contract: cgb-acid2

## Test Function Contract

### test_cgb_acid2()

**Type**: Integration test function  
**Package**: ceres-test-runner  
**Location**: tests/blargg_tests.rs

**Signature**:

```rust
#[test]
fn test_cgb_acid2()
```

**Behavior**:

1. Loads cgb-acid2.gbc ROM from test-roms directory
2. Creates TestRunner with CGB model and 600 frame timeout
3. Runs emulation until completion or timeout
4. Compares final screenshot against cgb-acid2.png
5. Asserts result is TestResult::Passed

**Preconditions**:

- Test ROM exists at test-roms/cgb-acid2/cgb-acid2.gbc
- Reference screenshot exists at test-roms/cgb-acid2/cgb-acid2.png
- ceres-core emulator compiled and linked

**Postconditions**:

- Test passes if screenshot matches exactly
- Test fails if screenshot differs or other error occurs
- Test fails if timeout exceeded

**Error Cases**:

- ROM not found → TestResult::Failed("Failed to load test ROM")
- Screenshot not found → TestResult::Failed("Failed to load expected screenshot")
- Timeout → TestResult::Timeout
- Screenshot mismatch → TestResult::Failed("Screenshot mismatch")

## Timeout Constant Contract

### timeouts::CGB_ACID2

**Type**: Constant  
**Package**: ceres-test-runner  
**Location**: src/test_runner.rs

**Signature**:

```rust
pub const CGB_ACID2: u32 = 600;
```

**Value**: 600 frames (~10 seconds at 59.73 Hz)

**Usage**: Passed to TestConfig.timeout_frames for cgb-acid2 test

**Rationale**: Provides 2x safety margin over expected completion time

## Helper Function Contract (Existing)

### run_test_rom(path, timeout)

**Type**: Helper function (existing, no changes)  
**Package**: ceres-test-runner  
**Location**: tests/blargg_tests.rs

**Signature**:

```rust
fn run_test_rom(path: &str, timeout: u32) -> TestResult
```

**Parameters**:

- `path`: Relative path from test-roms/ directory (e.g., "cgb-acid2/cgb-acid2.gbc")
- `timeout`: Maximum frames to run before timing out

**Returns**: TestResult enum (Passed, Failed, Timeout, Unknown)

**Behavior**:

1. Loads ROM via load_test_rom()
2. Creates TestConfig with:
   - timeout_frames = timeout
   - model = Model::Cgb
   - expected_screenshot = Some(screenshot_path)
   - capture_serial = true (default)
3. Creates and runs TestRunner
4. Returns result

## Integration Points

### File System

- **Read**: test-roms/cgb-acid2/cgb-acid2.gbc (ROM data)
- **Read**: test-roms/cgb-acid2/cgb-acid2.png (reference image)
- **No writes**: Test is read-only

### Emulator Core

- **Uses**: ceres_core::Gb (Game Boy emulator)
- **Uses**: ceres_core::Model::Cgb (CGB mode)
- **Uses**: ceres_core::ColorCorrectionMode::Disabled

### Test Framework

- **Uses**: Rust #[test] attribute
- **Uses**: assert_eq! macro
- **Integrates with**: cargo test runner

## Screenshot Comparison Contract

### compare_screenshot() (Existing)

**Type**: TestRunner method (existing, no changes)

**Behavior**:

1. Get actual screen from Gb::pixel_data_rgba()
2. Load expected image from path
3. Convert expected to RGBA8
4. Compare dimensions (160x144)
5. Compare byte-for-byte

**Returns**: Ok(true) if match, Ok(false) if mismatch, Err if I/O error

**Color Format**: RGBA8 using formula (X << 3) | (X >> 2) for 5-bit→8-bit

## Test Execution Flow

```text
cargo test
  └─> test_cgb_acid2()
       ├─> run_test_rom("cgb-acid2/cgb-acid2.gbc", timeouts::CGB_ACID2)
       │    ├─> load_test_rom() → Vec<u8>
       │    ├─> expected_screenshot_path() → PathBuf
       │    ├─> TestRunner::new(rom, config) → TestRunner
       │    └─> runner.run() → TestResult
       └─> assert_eq!(result, TestResult::Passed, "...")
```

## Exit Criteria

**Success**: assert_eq! succeeds (result == TestResult::Passed)  
**Failure**: assert_eq! panics with message "CGB Acid2 PPU test failed"

## Dependencies

- ceres_test_runner::load_test_rom
- ceres_test_runner::expected_screenshot_path
- ceres_test_runner::test_runner::TestRunner
- ceres_test_runner::test_runner::TestConfig
- ceres_test_runner::test_runner::TestResult
- ceres_test_runner::test_runner::timeouts
- image crate (for PNG loading, already in dependencies)
