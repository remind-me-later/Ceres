# Ceres Emulator

## Project Overview

Ceres is an experimental Game Boy and Game Boy Color emulator written in Rust,
designed with a modular architecture separating core emulation logic from
frontend implementations.

## Repository Structure

The repository is organized into several Rust crates:

- `ceres-core`: Core emulation logic (CPU, APU, PPU, Memory, Cartridge).
  `no_std` compatible.
- `ceres-std`: Standard library-dependent functionalities for desktop frontends
  (audio, threading, WebGPU renderer).
- `ceres-winit`: Minimal cross-platform CLI frontend (winit).
- `ceres-egui`: Cross-platform GUI frontend (egui).
- `ceres-gtk`: Linux-focused GUI frontend (GTK4).
- `gb-bootroms/`: Game Boy boot ROMs source and build scripts.
- `ceres-test-runner`: Integration test suite for emulator correctness using
  test ROMs and screenshot comparison.

## Key Resources and Standards

- **Gold Standard Emulator**: [SameBoy](https://github.com/LIJI32/SameBoy) is
  the reference for correct emulation behavior.
- **Hardware Documentation**: The [Pan Docs](https://gbdev.io/pandocs/) wiki is
  the primary reference for Game Boy hardware specifications.
- **Testing**: The `ceres-test-runner` uses
  [Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms) and its
  integration tests serve as the primary specification for emulator behavior.

## Development Guidelines

See `README.md` for general build/run instructions and `CONTRIBUTING.md` for
detailed development guidelines, including:

- **Code Style**: Rust (`cargo fmt`), TOML (`tombi format`), JSON/Markdown/YAML
  (`prettier`).
- **Commit Messages**: Conventional Commits.
- **Testing**: Running, writing, and coverage analysis.

## Accuracy & Testing Workflow

When troubleshooting failing integration tests (e.g., Mooneye, gbmicro), follow
this systematic approach:

1. **Isolation**: Identify the specific hardware behavior causing the failure.
   Use SameBoy or Gambatte as a reference to compare internal state transitions
   at a sub-M-cycle level.
2. **Unit Testing**: Instead of relying solely on the integration test loop,
   create minimal, high-speed unit tests in the relevant `ceres-core` module
   (e.g., `timing.rs`, `ppu/mod.rs`).
3. **Documenting Assumptions**: Every cycle-accurate assumption (e.g., "TIMA
   increments at T=4 of the M-cycle" or "LCD ON has a 4-tick offset") must be
   codified in a unit test. These tests serve as the primary documentation for
   hardware quirks.
4. **Sub-M-cycle Verification**: Ensure tests verify state at individual
   T-cycles (respecting the 2+2 timing model) to prevent "coincidental" passes
   that might break when instruction timing changes.
5. **Regression Guard**: After a fix passes the localized unit test, always run
   the full integration suite (`cargo test -p ceres-test-runner`) to verify that
   the fix doesn't break unrelated edge cases.
