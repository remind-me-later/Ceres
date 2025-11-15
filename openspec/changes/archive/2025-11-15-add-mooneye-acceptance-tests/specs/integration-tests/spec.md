## ADDED Requirements

### Requirement: CPU Register Reading API

The emulator core SHALL provide public methods to read individual CPU registers for test validation.

#### Scenario: Read B register

- **WHEN** test code calls `gb.cpu_b()`
- **THEN** the current value of the B register is returned as a u8
- **AND** the register value is not modified by the read operation

#### Scenario: Read C register

- **WHEN** test code calls `gb.cpu_c()`
- **THEN** the current value of the C register is returned as a u8
- **AND** the register value is not modified by the read operation

#### Scenario: Read D register

- **WHEN** test code calls `gb.cpu_d()`
- **THEN** the current value of the D register is returned as a u8
- **AND** the register value is not modified by the read operation

#### Scenario: Read E register

- **WHEN** test code calls `gb.cpu_e()`
- **THEN** the current value of the E register is returned as a u8
- **AND** the register value is not modified by the read operation

#### Scenario: Read H register

- **WHEN** test code calls `gb.cpu_h()`
- **THEN** the current value of the H register is returned as a u8
- **AND** the register value is not modified by the read operation

#### Scenario: Read L register

- **WHEN** test code calls `gb.cpu_l()`
- **THEN** the current value of the L register is returned as a u8
- **AND** the register value is not modified by the read operation

### Requirement: Mooneye Test Suite Validation

The test runner SHALL support validating Mooneye Test Suite ROMs using CPU register-based pass/fail detection.

#### Scenario: Detect passing test via Fibonacci registers

- **WHEN** a Mooneye test ROM completes execution
- **AND** the `ld b, b` breakpoint is detected
- **THEN** the test runner reads CPU registers B, C, D, E, H, L
- **AND** if the registers contain Fibonacci numbers (B=3, C=5, D=8, E=13, H=21, L=34), the test passes
- **AND** the test result is reported as `TestResult::Passed`

#### Scenario: Detect failing test via error code registers

- **WHEN** a Mooneye test ROM completes execution
- **AND** the `ld b, b` breakpoint is detected
- **THEN** the test runner reads CPU registers B, C, D, E, H, L
- **AND** if all registers contain 0x42, the test fails
- **AND** the test result is reported as `TestResult::Failed` with message "Mooneye test failed"

#### Scenario: Timeout for incomplete tests

- **WHEN** a Mooneye test ROM does not hit the `ld b, b` breakpoint
- **AND** the configured timeout is reached (7160 frames for 120 seconds at 59.73 Hz)
- **THEN** the test result is reported as `TestResult::Timeout`
- **AND** this prevents infinite loops in incomplete or broken test ROMs

#### Scenario: Early completion on breakpoint

- **WHEN** a Mooneye test ROM executes `ld b, b` before the timeout
- **THEN** the test runner immediately checks the CPU registers
- **AND** the test completes without waiting for the full timeout
- **AND** this allows passing tests to complete quickly (typically under 1 second)

### Requirement: Mooneye Acceptance Test Suite Integration

The test suite SHALL include all 75 Mooneye acceptance test ROMs with appropriate test execution and result reporting.

#### Scenario: Run passing acceptance tests

- **WHEN** a Mooneye acceptance test that currently passes on Ceres is executed
- **THEN** the test function runs without the `#[ignore]` attribute
- **AND** the test completes successfully with `TestResult::Passed`
- **AND** the test is included in the default test run

#### Scenario: Skip failing acceptance tests

- **WHEN** a Mooneye acceptance test that currently fails on Ceres is defined
- **THEN** the test function is marked with `#[ignore]` attribute
- **AND** a tracking comment explains why the test is ignored (e.g., "PPU timing not implemented")
- **AND** the test is not executed during normal test runs
- **AND** the test can be individually run with `cargo test -- --ignored <test_name>`

#### Scenario: Test organization by category

- **WHEN** acceptance tests are organized in the test file
- **THEN** tests are grouped by subdirectory (root level, bits/, instr/, interrupts/, oam_dma/, ppu/, serial/, timer/)
- **AND** each test function name follows the pattern `test_mooneye_<category>_<test_name>`
- **AND** test names use the ROM filename without the `.gb` extension, with hyphens converted to underscores

#### Scenario: Model-specific test execution

- **WHEN** a Mooneye test name contains model hints (e.g., `-dmgABC`, `-GS`, `-cgb`)
- **THEN** the test is executed on the appropriate Game Boy model (DMG, MGB, SGB, SGB2, or CGB)
- **AND** tests without model hints default to CGB model
- **AND** model selection ensures tests run in the environment they were designed for

### Requirement: Mooneye Test Timeout Configuration

The test runner SHALL define a timeout constant for Mooneye tests based on the documented maximum runtime.

#### Scenario: Mooneye timeout constant defined

- **WHEN** the `MOONEYE_ACCEPTANCE` timeout constant is defined as 7160 frames
- **THEN** this allows 120 seconds of emulation time at 59.73 fps
- **AND** this matches the maximum runtime specified in the Mooneye test suite documentation
- **AND** the timeout provides sufficient time for the slowest acceptance tests to complete
