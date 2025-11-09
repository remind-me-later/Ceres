# Spec-Kit Workflow Guide for Ceres Emulator

This document provides guidance for AI agents working on the Ceres Game Boy emulator using Spec-Kit workflows.

## Quick Start

### First Time Setup

1. **Read the Constitution**: Always start by reading `.specify/memory/constitution.md` to understand project principles
2. **Check Existing Specs**: Look in `.specify/specs/` to see what's already planned or implemented
3. **Understand the Codebase**: Review `AGENTS.md` in the project root for high-level architecture
4. **Install markdownlint-cli** (optional, for formatting):
   ```bash
   npm install -g markdownlint-cli
   # Or using yarn
   yarn global add markdownlint-cli
   ```

### Standard Workflow

```text
/speckit.constitution → /speckit.specify → /speckit.plan → /speckit.tasks → /speckit.implement
```

After each step, format generated markdown files:

```bash
# Format all specs in current feature
markdownlint --fix "specs/001-feature-name/**/*.md"

# Or format all spec files
markdownlint --fix "specs/**/*.md"
```

## Available Commands

### Core Commands (Use in Order)

1. **`/speckit.constitution`** - Establish or review project principles
   - Run once to understand project standards
   - Constitution already created at `.specify/memory/constitution.md`
   
2. **`/speckit.specify`** - Create feature specification
   - Define WHAT needs to be built and WHY
   - Focus on requirements, not implementation
   - Include success criteria and test expectations
   
3. **`/speckit.plan`** - Generate technical implementation plan
   - Define HOW to implement (tech stack, approach)
   - Reference SameBoy and Pan Docs
   - List specific files and modules to change
   
4. **`/speckit.tasks`** - Break down into actionable tasks
   - Generates phased task breakdown
   - Maps tasks to test requirements
   - Creates implementation checklist
   
5. **`/speckit.implement`** - Execute all tasks
   - Works through tasks sequentially
   - Runs tests after each phase
   - Validates against success criteria

### Enhancement Commands (Optional)

- **`/speckit.clarify`** - Ask structured questions before planning
  - Use when requirements are ambiguous
  - Run BEFORE `/speckit.plan`
  
- **`/speckit.analyze`** - Check cross-artifact consistency
  - Run AFTER `/speckit.tasks`, BEFORE `/speckit.implement`
  - Validates specs, plans, and tasks align
  
- **`/speckit.checklist`** - Generate quality validation checklist
  - Creates custom validation criteria
  - Run after `/speckit.plan` for quality assurance

## Ceres-Specific Guidelines

### When to Create a Spec

✅ **DO create specs for:**
- Bug fixes in failing tests (mem_timing-2, interrupt_time)
- New hardware features (RTC, rumble, link cable, serial)
- Performance optimizations that change behavior
- New frontend implementations (WASM, web, mobile)
- API changes in ceres-core
- Major refactors affecting multiple modules

❌ **DON'T create specs for:**
- Typo fixes in comments/documentation
- Code formatting (rustfmt, clippy suggestions)
- Simple documentation updates
- Dependency version bumps (unless breaking changes)

### Spec Granularity

| Size | Duration | Example |
|------|----------|---------|
| **Small** | 1-3 days | Fix single test, add missing register, documentation update |
| **Medium** | 1-2 weeks | Implement new hardware module, add frontend feature |
| **Large** | 1+ month | Complete APU rewrite, WASM frontend, save state system |

### Critical References

Always reference these when creating specs:

1. **SameBoy** - Gold standard for behavior
   - Repository: https://github.com/LIJI32/SameBoy
   - Check SameBoy implementation for timing, edge cases, quirks
   
2. **Pan Docs** - Hardware documentation
   - URL: https://gbdev.io/pandocs/
   - Use for register addresses, bit layouts, timing specs
   
3. **Existing Tests** - Validation suite
   - Location: `ceres-test-runner/tests/`
   - Current status: CPU 98% coverage, overall 54%
   - Must maintain or improve coverage

## Example: Fix mem_timing-2 Test

### Step 1: Create Specification

```
/speckit.specify Fix mem_timing-2 test timeout issue

The mem_timing-2 test from Blargg's test suite currently times out after 360 frames 
when it should complete successfully. This test validates advanced memory timing 
behavior that real Game Boy hardware exhibits.

Requirements:
- Investigate why mem_timing-2 test times out at 360 frames
- Compare Ceres memory timing implementation against SameBoy reference
- Identify discrepancies in memory access timing:
  * VRAM access during different PPU modes
  * OAM access timing and blocking
  * Memory bus contention handling
  * DMA transfer timing
- Fix timing issues to make test pass within reasonable frame count
- Ensure no regressions in other passing tests

Success Criteria:
- mem_timing-2 test completes with "Passed" screenshot match
- All existing tests continue to pass (cpu_instrs, mem_timing, instr_timing)
- Test completes in < 400 frames
- Code coverage remains >= 54% overall
```

### Step 2: Create Technical Plan

```
/speckit.plan Technical approach for mem_timing-2 fix

Tech Stack:
- Existing Rust codebase in ceres-core
- Focus modules: memory/ (mmu.rs), ppu/ (ppu.rs), timing.rs
- Reference: SameBoy implementation (https://github.com/LIJI32/SameBoy)
- Test framework: ceres-test-runner with screenshot comparison

Implementation Strategy:
1. Add detailed timing logging to memory access paths
2. Run test with debug output to identify failure point
3. Extract SameBoy timing logic for memory access during PPU modes
4. Compare Ceres vs SameBoy for:
   - VRAM access blocking during mode 3
   - OAM access restrictions during modes 2 and 3
   - Memory access timing during DMA
   - CPU wait states for contested memory regions
5. Implement timing fixes incrementally, testing after each change
6. Add integration test that validates memory timing behavior

Key Files to Modify:
- ceres-core/src/memory/mmu.rs - Memory access timing logic
- ceres-core/src/ppu/ppu.rs - PPU mode timing and VRAM/OAM blocking
- ceres-core/src/timing.rs - CPU timing and wait states
- ceres-test-runner/tests/blargg_tests.rs - Remove ignore attribute
```

### Step 3: Generate Tasks

```
/speckit.tasks
```

### Step 4: Execute Implementation

```
/speckit.implement
```

## Project Structure

```
Ceres/
├── .specify/
│   ├── memory/
│   │   └── constitution.md          # Project principles (READ THIS FIRST)
│   ├── scripts/
│   │   ├── create-new-feature.sh    # Creates new spec branches
│   │   ├── setup-plan.sh            # Generates plan artifacts
│   │   └── update-agent-context.sh  # Updates AGENTS.md references
│   ├── templates/
│   │   ├── spec-template.md         # Specification template
│   │   ├── plan-template.md         # Implementation plan template
│   │   └── tasks-template.md        # Task breakdown template
│   ├── specs/                       # Generated specs (created by commands)
│   │   └── 001-feature-name/
│   │       ├── spec.md              # Feature specification
│   │       ├── plan.md              # Implementation plan
│   │       ├── tasks.md             # Task breakdown
│   │       ├── research.md          # Technical research
│   │       ├── data-model.md        # Data structures
│   │       ├── quickstart.md        # Test scenarios
│   │       └── contracts/           # API contracts
│   └── AGENTS.md                    # This file
│
├── ceres-core/                      # Core emulation (no_std)
│   └── src/
│       ├── sm83.rs                  # CPU (~98% coverage)
│       ├── timing.rs                # Timing and cycles
│       ├── memory/                  # Memory management
│       ├── ppu/                     # Picture Processing Unit
│       ├── apu/                     # Audio Processing Unit
│       └── cartridge/               # Cartridge/MBC handling
│
├── ceres-test-runner/               # Integration tests
│   ├── tests/
│   │   └── blargg_tests.rs         # Blargg test suite
│   └── src/
│       └── test_runner.rs          # Test infrastructure
│
└── test-roms/                       # Test ROM collection (172MB)
    └── blargg/
        ├── cpu_instrs/              # ✅ Passing (11 tests)
        ├── instr_timing/            # ✅ Passing
        ├── mem_timing/              # ✅ Passing
        ├── mem_timing-2/            # ❌ Timing out
        └── interrupt_time/          # ❌ Timing out
```

## Markdown Formatting

### Using markdownlint-cli

All spec markdown files should be formatted with markdownlint-cli to maintain consistency and avoid linter warnings.

**Installation:**

```bash
npm install -g markdownlint-cli
```

**Format after spec creation:**

```bash
# Format current feature's markdown files
markdownlint --fix "specs/001-add-cgb-acid2-test/**/*.md"

# Format all spec files
markdownlint --fix "specs/**/*.md"

# Format constitution and guides
markdownlint --fix ".specify/**/*.md"

# Format entire project (specs + root docs)
markdownlint --fix "**/*.md"
```

**Integrated workflow:**

```bash
# After /speckit.specify
/speckit.specify Add new feature
markdownlint --fix "specs/001-new-feature/**/*.md"

# After /speckit.plan
/speckit.plan
markdownlint --fix "specs/001-new-feature/**/*.md"

# Before committing
git add specs/001-new-feature/
markdownlint --fix "specs/001-new-feature/**/*.md"
git add specs/001-new-feature/  # Re-add formatted files
git commit -m "feat: add new feature spec"
```

**Configuration:**

The project uses a `.markdownlint.json` configuration with relaxed rules for technical documentation:

```json
{
  "MD013": {
    "line_length": 120,     // Allow longer lines (120 chars)
    "code_blocks": false,   // Don't check code blocks
    "tables": false         // Don't check tables
  },
  "MD033": false,  // Allow inline HTML
  "MD034": false,  // Allow bare URLs
  "MD041": false   // Don't require H1 as first line
}
```

This allows for:
- Longer lines (up to 120 characters) for URLs and code examples
- Bare URLs in documentation
- Flexible heading structure in templates

## Best Practices

### Specification Writing

**Focus on WHAT and WHY, not HOW:**
```
✅ GOOD: "Fix VRAM access timing to match hardware behavior during PPU mode 3"
❌ BAD: "Add a check in mmu.rs line 123 to return 0xFF when in mode 3"
```

**Include testable success criteria:**
```
✅ GOOD: "mem_timing-2 test completes with 'Passed' screenshot match"
❌ BAD: "Memory timing is better"
```

**Reference existing systems:**
```
✅ GOOD: "Use SameBoy's memory blocking logic as reference"
❌ BAD: "Make memory timing more accurate"
```

### Planning Technical Details

**Be specific about modules:**
```
✅ GOOD: "Modify ceres-core/src/memory/mmu.rs::read_vram() to check PPU mode"
❌ BAD: "Update memory code"
```

**Reference documentation:**
```
✅ GOOD: "Per Pan Docs section 4.3, VRAM is inaccessible during mode 3"
❌ BAD: "VRAM can't be read sometimes"
```

**Compare with SameBoy:**
```
✅ GOOD: "SameBoy blocks VRAM reads in GB_read_memory() during mode 3"
❌ BAD: "Other emulators do this differently"
```

### Task Breakdown

**Phase tasks appropriately:**
- **Phase 1**: Investigation and research
- **Phase 2**: Core implementation
- **Phase 3**: Testing and validation
- **Phase 4**: Documentation and cleanup

**Make tasks testable:**
```
✅ GOOD: "1.1 Add VRAM blocking, verify with vram_timing test"
❌ BAD: "1.1 Fix VRAM stuff"
```

**Include regression testing:**
```
✅ GOOD: "3.2 Run full test suite, ensure no regressions"
❌ BAD: "3.2 Make sure it works"
```

## Testing Integration

### Test Suite Overview

| Test | Status | Duration | Coverage |
|------|--------|----------|----------|
| cpu_instrs | ✅ Passing | ~33s | 98% CPU |
| instr_timing | ✅ Passing | ~3.6s | Timing |
| mem_timing | ✅ Passing | ~4.6s | Memory |
| mem_timing-2 | ❌ Timeout | N/A | Advanced memory |
| interrupt_time | ❌ Timeout | N/A | Interrupt timing |

### Running Tests

```bash
# Run all tests
cargo test --package ceres-test-runner

# Run specific test
cargo test --package ceres-test-runner test_blargg_mem_timing_2

# With coverage
cargo llvm-cov --package ceres-core --package ceres-test-runner --html
```

### Test Expectations

Every spec should:
1. Reference which tests will validate the changes
2. Ensure existing tests continue to pass
3. Add regression tests for bug fixes
4. Update test documentation if needed

## Git Workflow

### Branch Strategy

Ceres uses a **three-tier branch strategy**:
- `main` - Production/deployment branch (stable releases only)
- `dev` - Development integration branch (all features merge here first)
- `001-feature-name` - Feature branches (created by Spec-Kit)

Spec-Kit automatically manages feature branches:

```bash
# Spec-Kit creates feature branch automatically
# After /speckit.specify, you'll be on branch: 001-feature-name

# Check branch
git branch  # Shows: 001-fix-mem-timing-2

# Work on the feature in this branch
# Make commits as you implement tasks
git add .
git commit -m "feat: implement feature"

# Push feature branch to remote
git push -u origin 001-feature-name
```

### Merging to Dev (Integration)

**Always merge to `dev` first**, never directly to `main`:

```bash
# Option 1: Create Pull Request (RECOMMENDED)
# After pushing feature branch, create PR to dev
# On GitHub: Create Pull Request → Base: dev, Compare: 001-feature-name
# Review, approve, and merge via GitHub UI (use squash merge)

# Option 2: Manual merge (local testing)
git checkout dev
git merge 001-feature-name
git push origin dev

# After PR is merged, update local dev
git checkout dev
git fetch origin
git reset --hard origin/dev  # Sync with remote

# Clean up feature branch (optional)
git branch -d 001-feature-name
git push origin --delete 001-feature-name
```

### Merging to Main (Deployment)

Only merge `dev` to `main` for releases:

```bash
# After testing thoroughly on dev
git checkout main
git merge dev
git push origin main

# Tag the release
git tag -a v0.1.0 -m "Release v0.1.0: Add cgb-acid2 test"
git push origin v0.1.0
```

### Pull Request Workflow

**Standard workflow for features:**

1. **Create feature branch** (automatic via `/speckit.specify`)
   ```bash
   # You'll be on: 001-feature-name
   ```

2. **Implement and commit changes**
   ```bash
   git add ceres-core/src/memory/mmu.rs
   git commit -m "fix(memory): implement VRAM blocking during mode 3"
   ```

3. **Push to remote**
   ```bash
   git push -u origin 001-feature-name
   ```

4. **Create Pull Request to `dev`**
   ```bash
   # Via GitHub CLI (if installed)
   gh pr create --base dev --title "Add cgb-acid2 test" --body "Implements spec 001-add-cgb-acid2-test"
   
   # Or via GitHub web UI:
   # - Navigate to repository
   # - Click "Pull requests" → "New pull request"
   # - Base: dev, Compare: 001-feature-name
   # - Fill in description, link to spec
   ```

5. **Review and merge**
   ```bash
   # After PR approval, merge via GitHub UI (squash merge recommended)
   # Or via CLI:
   gh pr merge 29 --squash --delete-branch
   ```

6. **Update local dev branch**
   ```bash
   git checkout dev
   git pull origin dev
   # Or if you have local commits:
   git fetch origin
   git reset --hard origin/dev
   ```

### Example: Complete Feature Workflow

```bash
# 1. Start feature (automatically creates branch)
/speckit.specify Add cgb-acid2 integration test

# 2. You're now on: 001-add-cgb-acid2-test
git branch
# * 001-add-cgb-acid2-test

# 3. Implement changes
# ... make code changes ...

# 4. Commit changes
git add ceres-test-runner/src/test_runner.rs
git add ceres-test-runner/tests/blargg_tests.rs
git commit -m "feat(tests): add cgb-acid2 PPU accuracy test"

# 5. Push feature branch
git push -u origin 001-add-cgb-acid2-test

# 6. Create PR to dev (via GitHub UI or CLI)
gh pr create --base dev --title "Add cgb-acid2 test" \
  --body "Implements spec 001-add-cgb-acid2-test. All tests passing."

# 7. After PR is merged, switch to dev
git checkout dev
git fetch origin
git reset --hard origin/dev

# 8. Verify tests pass on dev
cargo test --package ceres-test-runner

# 9. When ready for release, merge dev to main
git checkout main
git merge dev
git tag v0.2.0
git push origin main --tags
```

### Commit Messages

Use conventional commit format:

```
fix(memory): implement VRAM access blocking during PPU mode 3

- Add mode check in mmu.rs::read_vram()
- Block reads during mode 3 per Pan Docs 4.3
- Fixes mem_timing-2 test timeout
- Reference: SameBoy GB_read_memory() logic
```

## Common Patterns

### Investigating Test Failures

1. **Run test with output:**
   ```bash
   cargo test test_name -- --nocapture
   ```

2. **Add debug logging:**
   ```rust
   eprintln!("VRAM read at {:04X} during mode {}", addr, ppu_mode);
   ```

3. **Compare with SameBoy:**
   - Clone SameBoy: https://github.com/LIJI32/SameBoy
   - Find equivalent code
   - Document differences

### Implementing Hardware Features

1. **Read Pan Docs section**
2. **Check SameBoy implementation**
3. **Write integration test first**
4. **Implement incrementally**
5. **Test each step**
6. **Document behavior**

### Performance Optimization

1. **Profile first:**
   ```bash
   cargo build --release --package ceres-winit
   perf record ./target/release/ceres-winit rom.gb
   perf report
   ```

2. **Optimize hot paths**
3. **Benchmark before/after**
4. **Document tradeoffs**
5. **Ensure tests still pass**

## Troubleshooting

### Spec-Kit Issues

**Slash commands not appearing:**
1. Restart VS Code / IDE
2. Check `.github/prompts/*.prompt.md` files exist
3. Open new chat window

**Scripts failing:**
```bash
# Make scripts executable
chmod +x .specify/scripts/*.sh

# Check prerequisites
specify check
```

**Git branch issues:**
```bash
# Ensure you're in a git repo
git status

# Check current branch
git branch

# List all branches (including feature branches)
git branch -a
```

### Build Issues

**Test ROMs missing:**
```bash
# ROMs are auto-downloaded on first test
cargo test --package ceres-test-runner
# This downloads 172MB to test-roms/
```

**Coverage not working:**
```bash
# Install llvm-cov
cargo install cargo-llvm-cov

# Run coverage
cargo llvm-cov --package ceres-core --package ceres-test-runner
```

**Boot ROMs missing:**
```bash
cd gb-bootroms
make  # Requires RGBDS toolchain
```

## Resources

### Primary Documentation

- **Constitution**: `.specify/memory/constitution.md` (project principles)
- **Project Overview**: `AGENTS.md` (root directory, architecture overview)
- **Test Suite**: `ceres-test-runner/README.md`
- **Specs**: `.specify/specs/*/` (feature-specific documentation)

### External References

- **SameBoy**: https://github.com/LIJI32/SameBoy (reference implementation)
- **Pan Docs**: https://gbdev.io/pandocs/ (hardware documentation)
- **Test ROMs**: https://github.com/c-sp/gameboy-test-roms
- **GB Dev**: https://gbdev.io/ (community resources)
- **Spec-Kit**: https://github.github.io/spec-kit/ (workflow documentation)

### Code Coverage

Current status:
- **CPU (sm83.rs)**: ~98% - Excellent coverage
- **Overall**: ~54% - Target: 70%+
- **Untested areas**: Save states (BESS), RTC, joypad input, audio details

Track with:
```bash
cargo llvm-cov --package ceres-core --package ceres-test-runner --html
xdg-open target/llvm-cov/html/index.html
```

## Success Metrics

After implementing changes, verify:

- ✅ All tests pass (no regressions)
- ✅ Target test passes (e.g., mem_timing-2)
- ✅ Code coverage maintained or improved
- ✅ Performance acceptable (60fps)
- ✅ Documentation updated
- ✅ Spec artifacts complete (spec, plan, tasks)

## Getting Help

1. **Check constitution first**: `.specify/memory/constitution.md`
2. **Review existing specs**: `.specify/specs/*/`
3. **Read Pan Docs**: https://gbdev.io/pandocs/
4. **Compare with SameBoy**: https://github.com/LIJI32/SameBoy
5. **Check test output**: `cargo test -- --nocapture`

---

**Remember**: Always start by reading the constitution and understanding the project principles before creating specs!
