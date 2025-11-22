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

### Documentation

- **Location**: Create and modify documentation in the `docs/` or `openspec/` directories unless instructed otherwise.

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

See `README.md` for build instructions and prerequisites.

## Key Resources and Standards

- **Gold Standard Emulator**: We use [SameBoy](https://github.com/LIJI32/SameBoy) as the reference for correct emulation
  behavior. In cases of ambiguity, SameBoy's implementation is considered the ground truth.
- **Hardware Documentation**: The [Pan Docs](https://gbdev.io/pandocs/) wiki is the primary reference for Game Boy
  hardware specifications, memory maps, and programming details.
- **Testing**: We use the [Game Boy Test Roms](https://github.com/c-sp/gameboy-test-roms) repository for validating the
  correctness of our emulation.

## Development Documentation

- **Testing & CI/CD**: See `docs/TESTING.md` for running tests, adding new tests, code coverage, and CI/CD details.
- **Tracing & Debugging**: See `docs/TRACING_GUIDE.md` for execution tracing, Perfetto analysis, and debugging
  workflows.
- **Code Style**: See `CONTRIBUTING.md` for formatting rules and tools.
- **Commit Messages**: See `CONTRIBUTING.md` for the Conventional Commits convention.
