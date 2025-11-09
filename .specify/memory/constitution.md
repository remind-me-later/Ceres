# Ceres Game Boy Emulator Constitution

## Core Principles

### I. SameBoy Reference Standard

SameBoy is the **gold standard** for behavior verification and correctness.

- When behavior is ambiguous or undocumented, defer to SameBoy's implementation
- Use SameBoy source code as reference for timing, edge cases, and hardware quirks
- Document any intentional deviations from SameBoy with justification
- SameBoy repository: https://github.com/LIJI32/SameBoy

### II. Test-Driven Development (NON-NEGOTIABLE)

All changes MUST be validated against the Blargg test ROM suite.

- **98%+ CPU coverage** must be maintained at all times
- New features require accompanying integration tests
- Bug fixes must include regression tests
- Test ROMs: Blargg (cpu_instrs, mem_timing, instr_timing, interrupt_time)
- All tests must pass before merging to main branch
- Use ceres-test-runner with screenshot comparison for validation

### III. Pan Docs Compliance

All hardware behavior must align with Pan Docs specifications.

- Pan Docs is the authoritative hardware documentation: https://gbdev.io/pandocs/
- Document register addresses, bit fields, and timing according to Pan Docs
- Flag any hardware quirks or undocumented behavior with references
- Keep comments synchronized with Pan Docs terminology

### IV. no_std Core Requirement

The `ceres-core` crate MUST remain `no_std` compatible.

- Core emulation logic must be platform-agnostic
- No dependencies on standard library in ceres-core
- Frontend-specific code belongs in separate crates (ceres-std, ceres-gtk, etc.)
- Allows deployment on embedded systems, WASM, and resource-constrained platforms

### V. Modular Architecture

Clear separation of concerns across emulator subsystems.

**Module boundaries:**
- **CPU (sm83)**: Instruction execution, registers, flags
- **PPU**: Graphics rendering, sprites, backgrounds, window
- **APU**: Sound generation, channels, envelope, sweep
- **Memory**: MMU, memory mapping, cartridge interface
- **Cartridge**: MBC controllers, RAM, ROM banking, RTC

Each module must:
- Have well-defined public APIs
- Be independently testable
- Minimize cross-module dependencies
- Document inter-module contracts

### VI. Performance Requirements

Maintain real-time performance on mid-tier hardware.

- Target: 60fps (59.73 Hz exact) on mid-tier hardware
- Optimize hot paths identified through profiling
- Performance regressions require justification
- Benchmark critical code paths regularly
- Consider embedded/WASM performance implications

### VII. Code Coverage Standards

New code requires comprehensive test coverage.

- **80%+ coverage** for new code (lines)
- **Integration tests** for hardware interactions
- **Unit tests** for isolated logic
- Current overall coverage: ~54% (target: 70%+)
- CPU coverage: ~98% (maintain or improve)
- Use `cargo llvm-cov` for coverage analysis

### VIII. Documentation Standards

All public APIs must be documented with hardware context.

**Required documentation:**
- Hardware register addresses and bit layouts
- Timing information (cycles, frame counts)
- References to Pan Docs sections
- Examples for non-trivial APIs
- Edge cases and hardware quirks

**Example:**
```rust
/// Reads from the LCD Control register (LCDC) at address 0xFF40.
///
/// Bit layout (Pan Docs):
/// - Bit 7: LCD Enable (0=Off, 1=On)
/// - Bit 6: Window Tile Map (0=9800-9BFF, 1=9C00-9FFF)
/// ...
pub fn read_lcdc(&self) -> u8 { ... }
```

**Markdown Formatting:**

All specification documents must be formatted with markdownlint:

```bash
# Format spec files after creation
markdownlint --fix "specs/**/*.md"

# Configuration in .markdownlint.json
```

This ensures:
- Consistent formatting across all spec documents
- No linter warnings in documentation
- Easy readability and maintenance
- Standard markdown best practices

## Technology Stack

### Languages & Frameworks
- **Rust**: Primary implementation language (Rust 1.91+, Edition 2024)
- **RGBDS**: Boot ROM assembly (gbz80/SM83 assembler)

### Dependencies
- Minimize external dependencies in ceres-core
- Use well-maintained crates with active communities
- Audit security vulnerabilities regularly
- Document rationale for each major dependency

### Build System
- Cargo for Rust builds
- Make for boot ROM builds
- GitHub Actions for CI/CD
- Test ROM downloads cached in CI

## Development Workflow

### When to Create a Spec

✅ **DO create specs for:**
- Bug fixes affecting multiple modules
- New hardware features (RTC, rumble, link cable, serial port)
- Performance optimizations that change behavior
- New frontend implementations (WASM, SDL, etc.)
- API changes in ceres-core
- Test suite additions or modifications

❌ **DON'T create specs for:**
- Typo fixes in comments or docs
- Code formatting/style changes
- Simple documentation updates
- Dependency version bumps (unless breaking)

### Spec Granularity

- **Small specs** (1-3 days): Bug fixes, minor features
- **Medium specs** (1-2 weeks): Hardware modules, frontend features
- **Large specs** (1+ month): Major refactors, new emulator capabilities

### Branch Strategy

Ceres uses a **three-tier branch strategy**:

```
main         - Production/deployment branch (stable releases only)
dev          - Development integration branch (all features merge here first)
001-feature  - Feature branch (created by /speckit.specify)
```

**Workflow:**
1. Feature branches created automatically by Spec-Kit (`001-feature-name`)
2. All feature branches merge to `dev` via Pull Request
3. `dev` is tested and stabilized
4. `dev` merges to `main` for releases

**Rules:**
- ❌ Never commit directly to `main`
- ❌ Never merge feature branches directly to `main`
- ✅ Always merge features to `dev` first
- ✅ Use Pull Requests for all merges to `dev`
- ✅ Only merge `dev` to `main` for releases

### Code Review Requirements

All code must:
- Pass all Blargg tests (no regressions)
- Maintain or improve code coverage
- Follow Rust style guidelines (rustfmt, clippy)
- Include appropriate documentation
- Reference relevant spec documents

### Testing Gates

**Before merge to dev:**
- ✅ All unit tests pass
- ✅ All integration tests pass
- ✅ Code coverage meets standards
- ✅ Clippy warnings resolved
- ✅ Documentation complete

**Before merge to main:**
- ✅ All tests pass on CI
- ✅ Performance benchmarks acceptable
- ✅ Release notes updated
- ✅ CHANGELOG.md updated

## Quality Standards

### Error Handling

- Use `Result<T, E>` for fallible operations
- Provide meaningful error messages
- Log errors with context
- Fail fast on unrecoverable errors

### Safety

- Minimize `unsafe` code
- Document all unsafe blocks with safety invariants
- Use safe abstractions over unsafe primitives
- Run Miri on unsafe code

### Idiomatic Rust

- Follow Rust API guidelines
- Use strong types over primitives
- Leverage the type system for correctness
- Prefer composition over inheritance
- Use iterators over manual loops where appropriate

## Hardware Accuracy Priorities

### Priority 1 (Critical)
- CPU instruction execution
- Memory timing and access patterns
- PPU rendering and timing
- Interrupt handling

### Priority 2 (Important)
- APU audio generation
- DMA transfers
- Timer accuracy
- Serial communication

### Priority 3 (Nice to have)
- Boot ROM execution
- Obscure hardware quirks
- Undocumented behavior
- Performance optimizations

## Governance

### Constitution Authority

This constitution supersedes all other development practices.

- All PRs must demonstrate compliance
- Spec-Kit workflows must follow these principles
- Complexity must be justified against these standards
- Runtime guidance in `.specify/AGENTS.md`

### Amendments

Constitution changes require:
1. Documented rationale for the change
2. Review by project maintainers
3. Migration plan for existing code
4. Version number update

### Enforcement

- PRs violating principles require justification or revision
- Maintainers may reject PRs that compromise quality standards
- Technical debt must be documented and tracked
- Regular reviews ensure ongoing compliance

## Resources

### Primary References
- **SameBoy**: https://github.com/LIJI32/SameBoy
- **Pan Docs**: https://gbdev.io/pandocs/
- **Test ROMs**: https://github.com/c-sp/gameboy-test-roms

### Community
- **GB Dev Community**: https://gbdev.io/
- **Bootstrap ROM Info**: https://gbdev.gg8.se/wiki/articles/Gameboy_Bootstrap_ROM

### Existing Documentation
- See `AGENTS.md` in project root for high-level project overview
- See `.specify/AGENTS.md` for Spec-Kit workflow guidance
- See individual spec documents in `.specify/specs/` for feature details

---

**Version**: 1.0.0  
**Ratified**: 2025-11-08  
**Last Amended**: 2025-11-08
