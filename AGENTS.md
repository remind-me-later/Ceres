# Ceres

## Project Overview

Ceres is an experimental Game Boy and Game Boy Color
emulator written in Rust. It is designed with a modular architecture,
separating the core emulation logic from the frontend implementations.

## Repository Structure

The repository is organized into several Rust crates, each with a specific
responsibility:

- `ceres-core`: The heart of the emulator. It contains the hardware emulation
  logic, including:
  - SM83 CPU (`sm83.rs`)
  - Audio Processing Unit (APU) (`apu/`)
  - Pixel Processing Unit (PPU) (`ppu/`)
  - Memory Management (`memory/`)
  - Cartridge handling (`cartridge/`)
  - This crate is designed to be `no_std` compatible, allowing it to run on a
    wide range of platforms.

- `ceres-std`: Provides standard library-dependent functionalities for desktop
  frontends, such as:
  - Audio playback (`audio.rs`)
  - Threading (`thread.rs`)
  - A WebGPU-based renderer (`wgpu_renderer/`) used by the `winit` and `egui`
    frontends.

- `ceres-winit`: A minimal, cross-platform CLI frontend using `winit` for
  windowing. It renders the emulator screen but offers no GUI controls.

- `ceres-egui`: A cross-platform frontend built with the `egui` immediate-mode
  GUI library.

- `ceres-gtk`: A Linux-focused frontend using GTK4 for its user interface.

- `gb-bootroms/`: Contains the source code and build scripts for the Game Boy
  boot ROMs used by the emulator.

- `ceres-test-runner`: Integration test suite that validates emulator
  correctness using test ROMs. Currently includes Blargg's CPU instruction
  tests, instruction timing, and memory timing tests. Test ROMs are
  automatically downloaded during the build process (172MB cached download).

## Building and Running

### Prerequisites

- **Rust Toolchain**: Required for building all Rust crates.
- **RGBDS**: The [RGBDS toolchain](https://rgbds.gbdev.io/) is needed to
  assemble the boot ROMs located in `gb-bootroms/`.

### Build Steps

1. Initialize Git submodules: `git submodule update --init --recursive`
2. Build the boot ROMs: `cd gb-bootroms && make`
3. Select the default frontend in the root `Cargo.toml` file. For example, to
   use the GTK frontend, set `default-members = ["ceres-gtk"]`.
4. Build the project: `cargo build`
5. Run the selected frontend: `cargo run`

## Key Resources and Standards

- **Gold Standard Emulator**: We use [SameBoy](https://github.com/LIJI32/SameBoy)
  as the reference for correct emulation behavior. In cases of ambiguity,
  SameBoy's implementation is considered the ground truth.
- **Hardware Documentation**: The
  [Pan Docs](https://gbdev.io/pandocs/) wiki is the primary reference for Game
  Boy hardware specifications, memory maps, and programming details.
- **Testing**: We use the
  [Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms) repository
  for validating the correctness of our emulation.

## Testing

### Running Tests

The test suite includes both unit tests and integration tests using Game Boy
test ROMs:

```bash
# Run all tests (including integration tests)
cargo test --package ceres-core --package ceres-test-runner

# Run only the test runner
cargo test --package ceres-test-runner
```

Test ROMs are automatically downloaded on the first build (172MB). The download
is cached, so subsequent builds don't require re-downloading.

### Code Coverage

To analyze test coverage using `cargo-llvm-cov`:

```bash
# Install cargo-llvm-cov (one-time setup)
cargo install cargo-llvm-cov

# Generate HTML coverage report
cargo llvm-cov --package ceres-core --package ceres-test-runner --html

# Open the report
xdg-open target/llvm-cov/html/index.html

# Or get a terminal summary
cargo llvm-cov --package ceres-core --package ceres-test-runner
```

**Current Coverage Status:**

- **CPU (`sm83.rs`)**: ~98% - Blargg tests thoroughly validate CPU instructions
  and timing
- **Overall**: ~54% - Focus areas include CPU, memory, interrupts, and timing
- **Untested areas**: Save states (BESS), RTC, joypad input, audio details

The integration tests run in ~3-4 seconds and validate all SM83 CPU
instructions, instruction timing, and memory timing against the reference
implementation.

**Note**: Some tests are currently ignored due to emulation bugs that need
fixing:

- `mem_timing-2` suite (all 3 tests + combined test) - times out
- `interrupt_time` - times out

Run with `cargo test --package ceres-test-runner -- --ignored` to see these
failing tests.

### CI/CD Pipeline

GitHub Actions automatically runs tests on every push:

- Installs RGBDS toolchain for bootrom compilation
- Caches dependencies and test ROMs
- Runs tests for `ceres-core` and `ceres-test-runner` only (avoids frontend
  dependencies like GTK)
- Tests complete in under 2 minutes

See `.github/workflows/test.yml` for the complete workflow configuration.
