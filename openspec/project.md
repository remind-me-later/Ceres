# Project Context

## Purpose

Ceres is an experimental Game Boy and Game Boy Color emulator written in Rust. It is designed with a modular
architecture, separating the core emulation logic from the frontend implementations to allow for portability and
multiple user interfaces. The primary goal is to achieve high emulation accuracy.

## Tech Stack

- **Primary Language**: Rust (2024 Edition)
- **Core Emulation**: A `no_std` compatible crate (`ceres-core`) containing the CPU, PPU, APU, and memory logic.
- **Desktop Frontends**:
  - `ceres-winit`: A minimal CLI frontend using `winit`.
  - `ceres-egui`: A cross-platform GUI frontend using `egui`.
  - `ceres-gtk`: A Linux-focused GUI frontend using GTK4.
- **Graphics**: WebGPU (`wgpu`) is used for rendering in the `winit` and `egui` frontends, managed by the `ceres-std`
  crate.
- **Build System**: Cargo manages the Rust workspace and dependencies.
- **Boot ROMs**: The Game Boy boot ROMs are assembled using the RGBDS toolchain.

## Project Conventions

### Code Style

- **Rust**: Code is formatted using the standard `rustfmt` tool.
- **Linting**: A strict set of `clippy` lints is enforced, as defined in the root `Cargo.toml`. The project aims to be
  free of `unsafe` code.
- **Other**: Markdown and other configuration files are formatted using Prettier and markdownlint.

### Architecture Patterns

- **Modular Workspace**: The project is a Cargo workspace with multiple crates.
- **Decoupled Core**: The main emulation logic in `ceres-core` is completely separate from any frontend-specific code.
- **Shared Standard Library**: The `ceres-std` crate provides common functionalities (like audio and rendering) for
  desktop-based frontends that depend on a standard library.

### Testing Strategy

- **Unit Tests**: There are currently no unit tests.
- **Integration Tests**: The project relies on a dedicated `ceres-test-runner` crate that runs test ROMs from
  established suites (e.g., Blargg, Acid2) and validates correctness using screenshot comparison against reference
  images. These tests are run on pushes to the `main` branch and on pull requests. More details can be found in
  `AGENTS.md` and `.github/workflows/test.yml`.
- **Framework**: Testing is done via `cargo test`.
- **Reference Standard**: [SameBoy](https://github.com/LIJI32/SameBoy) is considered the gold standard for correct
  emulation behavior.
- **Coverage**: High test coverage is a priority, especially for the CPU core (`sm83.rs`). Coverage is measured with
  `cargo-llvm-cov`.

### Git Workflow

- **Branching**: Development is done on a `dev` branch. Features and bugfixes are developed on separate branches and
  merged into the `dev` branch. The `dev` branch is then merged into `main` for releases.
- **Submodules**: Git submodules are used to manage external dependencies like the boot ROM source code.
- **CI**: A GitHub Actions workflow automatically runs the full test suite on every push to the repository.

## Domain Context

- **Hardware Reference**: The [Pan Docs](https://gbdev.io/pandocs/) wiki is the primary reference for all Game Boy
  hardware specifications and behavior.
- **Emulator Accuracy**: The project prioritizes accuracy, using SameBoy as a reference and validation through extensive
  test ROMs.

## Important Constraints

- **`no_std` Core**: The `ceres-core` crate must remain `no_std` compatible to allow it to be compiled for a wide range
  of platforms, including embedded systems.
- **Accuracy over Performance**: While performance is important, emulation accuracy takes precedence.

## External Dependencies

- **Rust Toolchain**: Required to build all Rust crates.
- **RGBDS**: The RGBDS assembler is required to build the boot ROMs from source.
- **Test ROMs**: A large set of test ROMs (~172MB) is automatically downloaded and cached by the build script for
  `ceres-test-runner`.
