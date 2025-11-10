<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

## For AI Agents: Use OpenSpec for Development

**This project uses [OpenSpec](https://openspec.dev) for structured, spec-driven development.**

See `openspec/AGENTS.md` for complete OpenSpec workflow documentation.

### When to Create a Spec

✅ Create specs for:

- Bug fixes affecting multiple modules (e.g., PPU rendering issues)
- New hardware features (RTC, serial, link cable, etc.)
- Performance optimizations that change behavior
- New frontend implementations
- API changes in ceres-core

❌ Simple changes (typos, formatting) don't need specs.

### Key Principles

- **SameBoy is the gold standard** for behavior verification
- **Test-driven development** - maintain high CPU coverage
- **Pan Docs compliance** - all hardware behavior documented
- **no_std core** - keep ceres-core platform-agnostic

---

## Project Overview

Ceres is an experimental Game Boy and Game Boy Color emulator written in Rust. It is designed with a modular
architecture, separating the core emulation logic from the frontend implementations.

## Repository Structure

The repository is organized into several Rust crates, each with a specific responsibility:

- `ceres-core`: The heart of the emulator. It contains the hardware emulation logic, including:

  - SM83 CPU (`sm83.rs`)
  - Audio Processing Unit (APU) (`apu/`)
  - Pixel Processing Unit (PPU) (`ppu/`)
  - Memory Management (`memory/`)
  - Cartridge handling (`cartridge/`)
  - This crate is designed to be `no_std` compatible, allowing it to run on a wide range of platforms.

- `ceres-std`: Provides standard library-dependent functionalities for desktop frontends, such as:

  - Audio playback (`audio.rs`)
  - Threading (`thread.rs`)
  - A WebGPU-based renderer (`wgpu_renderer/`) used by the `winit` and `egui` frontends.

- `ceres-winit`: A minimal, cross-platform CLI frontend using `winit` for windowing. It renders the emulator screen but
  offers no GUI controls.

- `ceres-egui`: A cross-platform frontend built with the `egui` immediate-mode GUI library.

- `ceres-gtk`: A Linux-focused frontend using GTK4 for its user interface.

- `gb-bootroms/`: Contains the source code and build scripts for the Game Boy boot ROMs used by the emulator.

- `ceres-test-runner`: Integration test suite that validates emulator correctness using test ROMs. Tests use screenshot
  comparison against reference images from Blargg's test suite (CPU instructions, instruction timing, and memory
  timing). Test ROMs are automatically downloaded during the build process (172MB cached download).

## Building and Running

### Prerequisites

- **Rust Toolchain**: Required for building all Rust crates.
- **RGBDS**: The [RGBDS toolchain](https://rgbds.gbdev.io/) is needed to assemble the boot ROMs located in
  `gb-bootroms/`.

### Build Steps

1. Initialize Git submodules: `git submodule update --init --recursive`
2. Build the boot ROMs: `cd gb-bootroms && make`
3. Select the default frontend in the root `Cargo.toml` file. For example, to use the GTK frontend, set
   `default-members = ["ceres-gtk"]`.
4. Build the project: `cargo build`
5. Run the selected frontend: `cargo run`

## Key Resources and Standards

- **Gold Standard Emulator**: We use [SameBoy](https://github.com/LIJI32/SameBoy) as the reference for correct emulation
  behavior. In cases of ambiguity, SameBoy's implementation is considered the ground truth.
- **Hardware Documentation**: The [Pan Docs](https://gbdev.io/pandocs/) wiki is the primary reference for Game Boy
  hardware specifications, memory maps, and programming details.
- **Testing**: We use the [Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms) repository for validating the
  correctness of our emulation.

## Testing

### Running Tests

The test suite includes both unit tests and integration tests using Game Boy test ROMs:

```bash
# Run all tests (including integration tests)
cargo test --package ceres-core --package ceres-test-runner

# Run only the test runner
cargo test --package ceres-test-runner
```

Test ROMs are automatically downloaded on the first build (172MB). The download is cached, so subsequent builds don't
require re-downloading.

### Debugging with Execution Traces

When tests fail, the emulator can export execution traces for analysis. Traces capture the last N instructions before
failure, including PC, disassembled instruction, cycle count, and full register state.

**Enabling trace collection in tests:**

```rust
use ceres_test_runner::{TestRunner, TestConfig};

let config = TestConfig {
    timeout_seconds: 10,
    enable_trace: true,              // Enable trace collection
    export_trace_on_failure: true,   // Auto-export on failure
    trace_buffer_size: 1000,         // Circular buffer size
};

let runner = TestRunner::new(rom_path, reference_path, config);
runner.run().expect("Test failed");
```

Traces are automatically exported to `target/traces/<timestamp>_trace.json` on test failure or timeout.

**Analyzing traces:**

```bash
# Show last 20 instructions before failure
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --last 20

# Generate instruction frequency histogram
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --histogram

# Detect infinite loops (repeated PC sequences)
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --loops

# Find specific instructions (e.g., all JP/CALL instructions)
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --inst JP
```

**Common debugging workflows:**

1. **Test hangs/timeouts**: Use `--loops` to detect infinite loops, then `--last 50` to see the repeated sequence
2. **Incorrect behavior**: Use `--histogram` to see execution profile, compare against SameBoy
3. **Specific instruction issues**: Use `--inst` to find all occurrences of problematic instructions
4. **Memory-mapped I/O bugs**: Use `--range` to filter instructions in specific address ranges (e.g., 0xFF00-0xFFFF)

See `ceres-test-runner/README.md` for complete trace collection documentation and JSON format details.

**Integration Tests:**

The integration tests use screenshot comparison to validate emulator accuracy:

- `test_blargg_cpu_instrs` - All CPU instructions (11 tests in one ROM, ~33s)
- `test_blargg_instr_timing` - Instruction cycle timing (~3.6s)
- `test_blargg_mem_timing` - Memory access timing (~4.6s)
- `test_blargg_mem_timing_2` - Advanced memory timing (~5.9s)
- `test_blargg_interrupt_time` - Interrupt timing (~3.6s)
- `test_cgb_acid2` - CGB PPU accuracy test (~0.4s)
- `test_dmg_acid2_cgb` - DMG Acid2 PPU test in CGB mode (~0.2s)
- `test_dmg_acid2_dmg` - DMG Acid2 PPU test in DMG mode (currently ignored - known PPU rendering issue)

Each test compares the emulator's screen output pixel-by-pixel against reference PNG screenshots from the test ROM
repository, with color correction disabled for accuracy. Timeout values are based on actual completion times with
minimal margin for reliability.

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

- **CPU (`sm83.rs`)**: ~98% - Blargg tests thoroughly validate CPU instructions and timing
- **Overall**: ~54% - Focus areas include CPU, memory, interrupts, and timing
- **Untested areas**: Save states (BESS), RTC, joypad input, audio details

Integration tests complete in ~3-4 seconds and validate all SM83 CPU instructions, instruction timing, memory timing,
and interrupt timing against reference screenshots. All integration tests currently pass!

### CI/CD Pipeline

GitHub Actions automatically runs tests on every push:

- Installs RGBDS toolchain for bootrom compilation
- Caches dependencies and test ROMs
- Runs tests for `ceres-core` and `ceres-test-runner` only (avoids frontend dependencies like GTK)
- Tests complete in under 2 minutes

See `.github/workflows/test.yml` for the complete workflow configuration.

## Code Formatting

This project uses automated formatting tools to maintain consistent code style across all files.

### Rust Code

Format Rust code using `cargo fmt`:

```bash
# Format all Rust code in the workspace
cargo fmt --all

# Check formatting without making changes
cargo fmt --all -- --check
```

### JSON, Markdown, and YAML Files

Format JSON, Markdown, and YAML files using [Prettier](https://prettier.io/):

```bash
# Install prettier (one-time setup)
npm install -g prettier

# Format all supported files
prettier --write "**/*.{json,md,yaml,yml}"

# Check formatting without making changes
prettier --check "**/*.{json,md,yaml,yml}"
```

**Note**: Always format your code before committing changes to maintain consistency across the codebase.

## Commit Message Convention

This project uses [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

**Format**: `<type>[optional scope]: <description>`

**Common types**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`

**Common scopes**: `core`, `ppu`, `cpu`, `apu`, `memory`, `cartridge`, `gtk`, `egui`, `winit`, `std`, `tests`, `bootrom`

**Examples**:

- `feat(ppu): add sprite rendering support`
- `fix(cpu): correct timing for HALT instruction`
- `test(cpu): add Blargg CPU instruction tests`
- `refactor(memory)!: change memory access API` (breaking change)

See `CONTRIBUTING.md` for complete commit message guidelines and examples.
