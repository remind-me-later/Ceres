# Contributing to Ceres

Thank you for your interest in contributing to Ceres!

## Commit Message Convention

This project uses [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) for commit messages.

### Format

```text
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Types

- **feat**: A new feature
- **fix**: A bug fix
- **docs**: Documentation only changes
- **style**: Changes that don't affect code meaning (whitespace, formatting, etc)
- **refactor**: Code change that neither fixes a bug nor adds a feature
- **perf**: Performance improvements
- **test**: Adding or correcting tests
- **build**: Changes to build system or dependencies
- **ci**: Changes to CI configuration files and scripts
- **chore**: Other changes that don't modify src or test files
- **revert**: Reverts a previous commit

### Common Scopes

- **core**: Changes to ceres-core
- **ppu**: PPU-related changes
- **cpu**: CPU (SM83) related changes
- **apu**: Audio Processing Unit
- **memory**: Memory management
- **cartridge**: Cartridge/MBC handling
- **gtk**: GTK frontend
- **egui**: egui frontend
- **winit**: winit frontend
- **std**: ceres-std library
- **tests**: Test runner or test infrastructure
- **bootrom**: Boot ROM changes

### Breaking Changes

For breaking changes, add `!` after the type/scope:

```text
feat(api)!: change memory access API signature
```

Or add a `BREAKING CHANGE:` footer:

```text
feat(api): change memory access API

BREAKING CHANGE: Memory::read() now returns Result instead of u8
```

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
- Format JSON, Markdown, and YAML with `prettier --write "**/*.{json,md,yaml,yml}"`
- Run tests with `cargo test --package ceres-core --package ceres-test-runner`

## Debugging Failed Tests

### Using Execution Traces

When integration tests fail, you can enable trace collection to capture the last N instructions before failure:

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

Traces are exported to `target/traces/<timestamp>_trace.json` automatically.

### Analyzing Traces

Use the provided Python script to analyze execution traces:

```bash
# Show last 20 instructions with register state
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --last 20

# Find all JP instructions
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --inst JP

# Generate instruction histogram
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --histogram

# Detect infinite loops
python ceres-test-runner/analyze_trace.py target/traces/1234567890_trace.json --loops
```

### Common Debugging Patterns

1. **Test timeout/hang**: Use `--loops` to detect infinite loops
2. **Wrong behavior**: Compare `--histogram` output against SameBoy execution
3. **Instruction bugs**: Use `--inst MNEMONIC` to find specific instruction occurrences
4. **I/O issues**: Use `--range 0xFF00 0xFFFF` to filter I/O-related instructions

See `ceres-test-runner/README.md` for complete trace documentation.

## Development Workflow

See `AGENTS.md` for detailed development guidelines and the OpenSpec workflow for larger changes.
