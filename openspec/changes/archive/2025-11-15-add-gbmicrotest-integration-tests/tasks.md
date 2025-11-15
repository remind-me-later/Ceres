## 1. Scaffolding

- [ ] 1.1. Create a new test file `ceres-test-runner/tests/gbmicro_tests.rs`.
- [ ] 1.2. Add the new file as a module in `ceres-test-runner/src/lib.rs` if needed by the project structure.

## 2. Implementation

- [ ] 2.1. Add a test function for each `gbmicrotest` ROM.
- [ ] 2.2. Implement the test runner logic to:
  - Load the ROM.
  - Run the emulation for a sufficient number of frames (2 frames for most, with an exception for
    `is_if_set_during_ime0.gb`).
  - Check the memory address `0xFF82` for the test result (`0x01` for pass, `0xFF` for fail).
- [ ] 2.3. List all `gbmicrotest` ROMs and create a test case for each.

## 3. Verification

- [ ] 3.1. Run the newly created tests.
- [ ] 3.2. Identify all failing tests.
- [ ] 3.3. Mark the failing tests with `#[ignore]` to ensure the main test suite remains green.
- [ ] 3.4. Add a comment to each ignored test explaining why it is ignored (e.g., "Fails, needs investigation").
