# Contributing to Ceres

Thank you for your interest in contributing to Ceres!

## Commit Message Convention

This project uses
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) for
commit messages.

### Format

```text
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Types and Scopes

Refer to the
[Conventional Commits specification](https://www.conventionalcommits.org/en/v1.0.0/#summary)
for the full list of commit types (e.g., `feat`, `fix`, `docs`, `style`,
`refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`).

Common scopes include: `core`, `ppu`, `cpu`, `apu`, `memory`, `cartridge`,
`gtk`, `egui`, `winit`, `std`, `tests`, `bootrom`.

### Breaking Changes

Indicate breaking changes by appending `!` after the type/scope (e.g.,
`feat(api)!: change memory access API`) or by adding a `BREAKING CHANGE:` footer
in the commit body.

### AI-Assisted Contributions

Commits written with the help of an AI agent (Claude Code, Copilot, Cursor and
the like) must say so with an `Assisted-by:` footer naming the tool and, where
known, the model:

```text
fix(ppu): correct the mode 3 length with a window

Assisted-by: Claude Code (Claude Opus 5.5)
```

AI agents working on this repository must add this footer to every commit they
create. The contributor submitting the change remains responsible for it and
should review the result as they would their own code.

### Examples

```text
feat(ppu): add sprite rendering support
fix(cpu): correct timing for HALT instruction
docs: update README with build instructions
test(cpu): add Blargg CPU instruction tests
refactor(memory)!: change memory access API
perf(ppu): optimize tile rendering loop
chore(deps): update winit to 0.29
```

## Code Style

- Format Rust code with `cargo fmt --all`
- Format TOML with `tombi format`
- Format JSON, Markdown, YAML and JSON with
  `prettier --write "**/*.{json,yaml,yml,md,json}"`
- Ensure tests pass (see below)

## Testing

Ceres includes a comprehensive integration test suite `ceres-test-runner` that
validates emulator accuracy using actual Game Boy test ROMs and pixel-perfect
screenshot comparisons. These tests serve as the primary specification for the
emulator's behavior, validating its accuracy against actual Game Boy hardware.

The test runner uses multiple mechanisms to detect completion, as required by
each test suite:

- **Breakpoint detection**: Uses `ld b, b` (opcode 0x40) as a debug breakpoint
  for immediate completion (e.g., in Acid2 tests).
- **Screenshot comparison**: Tests pass when output matches reference images.
- **Timeout safety**: Prevents infinite loops.

### Setup

Test ROMs are **automatically downloaded** when you build or test. The build
script downloads the test ROM collection from
[c-sp/gameboy-test-roms](https://github.com/c-sp/gameboy-test-roms) into the
`test-roms/` directory.

### Running Tests

#### Run All Tests

```bash
cargo test --package ceres-test-runner
```

#### Run Specific Tests

```bash
# Run a specific test case
cargo test --package ceres-test-runner test_blargg_cpu_instrs

# Run all dmg-acid2 tests
cargo test --package ceres-test-runner test_dmg_acid2

# Run ignored tests (known failures)
cargo test --package ceres-test-runner -- --ignored
```

### CI/CD Pipeline

GitHub Actions runs the tests on pushes and pull requests to `main`. It
installs the RGBDS toolchain, caches dependencies and test ROMs, runs the
tests for `ceres-core` and `ceres-test-runner` with coverage, and uploads the
coverage to [Codecov](https://codecov.io/gh/remind-me-later/Ceres).

### Code Coverage

To analyze test coverage locally with `cargo-llvm-cov` and `cargo-nextest`:

```bash
# Install cargo-llvm-cov and nextest (Rust's llvm-tools must match rustc's
# LLVM: `rustup component add llvm-tools-preview`, or the system LLVM on
# distributions whose rustc uses it)
cargo install cargo-llvm-cov cargo-nextest

# Generate HTML report
cargo llvm-cov nextest --release --package ceres-core \
    --package ceres-test-runner --html

# Open report
xdg-open target/llvm-cov/html/index.html
```

Run it in release with nextest, as above. A debug build makes the ROM tests
very slow, and test threads sharing one process fight over the coverage
counters: nextest runs each test in its own process.
