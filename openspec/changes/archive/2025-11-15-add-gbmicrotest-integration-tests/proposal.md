# Change: Add gbmicrotest Integration Tests

## Why

To improve emulator accuracy and increase test coverage by incorporating the `gbmicrotest` suite, which offers a
collection of small, targeted tests for specific Game Boy hardware behaviors.

## What Changes

- A new test module will be added to `ceres-test-runner` to handle the `gbmicrotest` ROMs.
- Each test ROM will have a corresponding integration test.
- The test runner will execute each ROM and verify the result based on the success/failure signature described in the
  `gbmicrotest` documentation.
- Initially, any failing tests will be marked as `#[ignore]` to establish a baseline, with the goal of fixing them in
  the future.

## Impact

- **Affected specs**: `integration-tests`
- **Affected code**: `ceres-test-runner/tests/`
