# Test ROMs

This directory contains Game Boy test ROMs used for validating the Ceres emulator.

## Downloading Test ROMs

The test ROMs are downloaded from the
[c-sp/gameboy-test-roms](https://github.com/c-sp/gameboy-test-roms) repository releases.

To download the latest test ROMs, run:

```bash
./download-test-roms.sh
```

Or to download a specific version:

```bash
./download-test-roms.sh v7.0
```

### Requirements

- `curl` or `wget` (for downloading)
- `unzip` (for extracting)

## Structure

After downloading, the test ROMs will be organized as follows:

```bash
test-roms/
├── blargg/
│   ├── cpu_instrs/
│   ├── instr_timing/
│   ├── mem_timing/
│   ├── dmg_sound/
│   └── ...
├── mooneye-test-suite/
├── same-suite/
├── acid2/
└── ...
```

## Running Tests

To run the test suite:

```bash
cd ..
cargo test --package ceres-tests -- --ignored --include-ignored
```

## Test Sources

- **Blargg's tests**: Classic CPU, timing, and sound tests
- **Mooneye Test Suite**: Comprehensive hardware tests
- **SameSuite**: Compatibility tests
- **Acid2**: Graphics rendering tests
- **And many more** from various authors

Total size: ~3.6 MB

## Credits

Test ROMs compiled by [c-sp/gameboy-test-roms](https://github.com/c-sp/gameboy-test-roms).
Original authors: Blargg, Gekkio, LIJI32, and many others.

- **SameSuite**: Additional compatibility tests
- **Acid2 tests**: Graphics rendering tests
- And many more...

For more information about the test suites, see the [gameboy-test-roms documentation](https://github.com/c-sp/gameboy-test-roms).
