# Implementation Tasks

## Phase 0: Remove JSONL Tooling (2-3 hours)

- [ ] Delete `ceres-test-runner/src/bin/trace-diff.rs` (~350 lines)
- [ ] Delete `ceres-test-runner/src/bin/trace-patterns.rs` (~350 lines)
- [ ] Delete `ceres-test-runner/src/bin/trace-query.rs` (~450 lines)
- [ ] Delete `ceres-test-runner/analyze_traces.py` (~180 lines)
- [ ] Delete `ceres-test-runner/search_traces.sh` (~60 lines)
- [ ] Delete `ceres-test-runner/src/trace_index.rs` (~500 lines)
- [ ] Delete `ceres-test-runner/src/test_tracer.rs` (JSONL export logic)
- [ ] Delete `ceres-test-runner/schemas/` directory (JSONL schemas)
- [ ] Remove JSONL-related dependencies from `Cargo.toml`
- [ ] Update `ceres-test-runner/README.md` to remove JSONL documentation
- [ ] Update `ceres-test-runner/README.md` to document Perfetto workflow
- [ ] Verify tests still compile after deletions

## Phase 1: Tracing Infrastructure (2-3 hours)

- [ ] Add `tracing-chrome` dependency to `ceres-test-runner/Cargo.toml`
- [ ] Implement `--trace` flag in test runner CLI (`test_runner.rs`)
- [ ] Configure ChromeLayer when `--trace` flag is present
- [ ] Set output path to `target/traces/test_name.json` (Chrome Trace Event Format)
- [ ] Add test metadata (ROM name, model, outcome) to trace
- [ ] Test trace collection with a simple test ROM
- [ ] Verify trace opens in ui.perfetto.dev

## Phase 2: Hardware State Tracing (4-6 hours)

- [ ] Add DMA tracing to `ceres-core/src/memory/dma.rs`
  - [ ] Emit span with `src_addr`, `bytes_remaining`, `active` fields
  - [ ] Ensure M-cycle accurate timing
- [ ] Add PPU tracing to `ceres-core/src/ppu/mod.rs`
  - [ ] Emit events on mode changes with `old_mode`, `new_mode`, `ly`, `cycle`
- [ ] Add memory conflict tracing to `ceres-core/src/memory/mod.rs`
  - [ ] Emit events when DMA blocks CPU access
  - [ ] Include `access_type`, `addr`, `value`, `blocked_by_dma` fields
- [ ] Verify traces include hardware state in Perfetto UI

## Phase 3: SQL Query Library (2-3 hours)

- [ ] Create `examples/sql/` directory structure
- [ ] Create `examples/sql/trace_comparison.sql` with comparison query templates
  - [ ] Query to find first divergence point (with CTEs and JOINs)
  - [ ] Query to count total differences
  - [ ] Query to compare timing between traces
  - [ ] Query to compare memory access patterns
  - [ ] Query to compare register states at divergence
- [ ] Create `examples/sql/tight_loops.sql` - Detect infinite loops
- [ ] Create `examples/sql/dma_operations.sql` - Track all DMA uploads
- [ ] Create `examples/sql/memory_hotspots.sql` - Find frequently accessed addresses
- [ ] Create `examples/sql/register_changes.sql` - Track register value changes
- [ ] Create `examples/sql/slow_instructions.sql` - Find instructions taking >50 T-states
- [ ] Test all queries with real traces in trace_processor
- [ ] Document query usage in debugging guide

## Phase 4: Documentation (4-5 hours)

- [ ] Create `docs/debugging.md` - Overview of Perfetto workflow
  - [ ] Basic workflow: run test with `--trace`, open in ui.perfetto.dev
  - [ ] How to use trace_processor CLI
  - [ ] How to use Perfetto Python API
  - [ ] Link to all SQL query examples
- [ ] Create `docs/debugging-timing-issues.md` - Timing-specific debugging guide
  - [ ] How to identify OAM DMA timing issues
  - [ ] Using SQL to find timing divergences
  - [ ] Comparing execution timing between emulators
  - [ ] Real-world example: call_cc_timing2 investigation
- [ ] Create `docs/debugging-sql-queries.md` - SQL query patterns reference
  - [ ] All query patterns with explanations
  - [ ] How to adapt queries for specific needs
  - [ ] Common query patterns (window functions, CTEs, JOINs)
  - [ ] Agent-friendly examples showing programmatic usage
- [ ] Create `docs/trace-format.md` - Chrome Trace Event Format + Game Boy extensions
  - [ ] Chrome Trace Event Format basics
  - [ ] Game Boy-specific span types (oam_dma, ppu_mode_change)
  - [ ] Game Boy-specific event types (memory_access, register_change)
  - [ ] Schema documentation for custom fields
- [ ] Create `examples/debug_mooneye_test.rs` - Complete debugging workflow example
  - [ ] Show test setup with tracing enabled
  - [ ] Example SQL queries to analyze results
  - [ ] Python script showing Perfetto API usage

## Phase 5: Testing and Validation (2-3 hours)

- [ ] Run test with `--trace` and verify Chrome Trace Event Format JSON output
- [ ] Open trace in ui.perfetto.dev and verify timeline view works
- [ ] Verify CPU instruction events visible with register state
- [ ] Verify DMA/PPU state visible as spans/events in Perfetto
- [ ] Load multiple traces in trace_processor for comparison testing
- [ ] Test all SQL queries in `examples/sql/` with real traces
  - [ ] trace_comparison.sql with Ceres vs SameBoy traces
  - [ ] tight_loops.sql with infinite loop test ROM
  - [ ] dma_operations.sql with OAM DMA test
  - [ ] memory_hotspots.sql with normal execution
  - [ ] register_changes.sql with register manipulation code
- [ ] Test Perfetto Python API with example script
- [ ] Verify zero performance impact when `--trace` not used
- [ ] Verify all old JSONL traces are no longer generated
- [ ] Update AGENTS.md with Perfetto debugging workflow examples

## Phase 6: Migration Cleanup (1 hour)

- [ ] Verify no remaining references to JSONL format in code
- [ ] Verify no remaining references to deleted tools in documentation
- [ ] Update any CI/CD scripts that referenced old trace format
- [ ] Archive PERFETTO_MIGRATION.md with change when complete

## Total Estimated Time: 15-21 hours

**Code Impact Summary:**

- Delete: ~1,900 lines (JSONL tooling)
- Add: ~300 lines (Chrome tracing integration + hardware tracing)
- Net: -1,600 lines (88% reduction)
