# Ceres Tests

Integration tests for the Ceres Game Boy emulator using actual Game Boy test ROMs.

## Setup

Before running tests, you need to download the test ROMs:

```bash
cd ../test-roms
./download-test-roms.sh
```

This will download the latest version of the test ROM collection from the
[c-sp/gameboy-test-roms](https://github.com/c-sp/gameboy-test-roms) repository.

## Running Tests

Run all tests:

```bash
cargo test --package ceres-tests
```

Run only fast tests (excluding ignored tests):

```bash
cargo test --package ceres-tests
```

Run all tests including slow/ignored ones:

```bash
cargo test --package ceres-tests -- --ignored --include-ignored
```

## Test Organization

Tests are organized by source:

- **Blargg tests**: CPU instructions, timing, memory, and sound tests
- **Mooneye tests**: Hardware behavior tests
- **Acid2 tests**: Graphics rendering tests
- **SameSuite**: Compatibility tests
- And more...

## Writing New Tests

To add a new test ROM:

1. Ensure the ROM is available in `test-roms/`
2. Use the `load_test_rom()` helper to load it
3. Create a `TestRunner` with appropriate configuration
4. Run the test and assert on the result
