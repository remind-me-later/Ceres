# Debugging Tools Capability

## ADDED Requirements

### Requirement: Test-Specific Trace Collection

The test runner SHALL support collecting execution traces for individual tests with a simple CLI flag.

#### Scenario: Run test with tracing enabled

- **WHEN** a developer runs `cargo test -- test_name --trace`
- **THEN** the test executes with tracing enabled and outputs a Chrome Trace Event Format file to
  `target/traces/test_name.json`
- **AND** the trace includes test metadata (ROM name, model, outcome)

#### Scenario: Agent invokes trace collection

- **WHEN** an AI agent needs to debug a failing test
- **THEN** the agent can invoke the test with `--trace` flag programmatically
- **AND** parse the resulting JSON trace file

### Requirement: Perfetto Timeline Visualization

The tracing system SHALL use Chrome Trace Event Format compatible with Perfetto's visualization UI.

#### Scenario: Human visualizes trace in Perfetto

- **WHEN** a developer opens the trace JSON file at ui.perfetto.dev
- **THEN** they see a timeline view of CPU instruction execution with register state
- **AND** can use Perfetto's UI to zoom, filter, and analyze execution patterns

#### Scenario: Agent uses Perfetto SQL queries

- **WHEN** an AI agent needs to find execution patterns like tight loops or DMA uploads
- **THEN** the agent can use Perfetto's trace_processor with SQL queries
- **AND** receive structured results (e.g., "tight loop at PC 0x0150 executed 1000 times in 5ms")

### Requirement: Hardware State Tracing

The emulator SHALL emit trace events for Game Boy hardware state changes (DMA, PPU, memory).

#### Scenario: Trace DMA operations

- **WHEN** an OAM DMA operation occurs during test execution
- **THEN** a trace span is emitted with source address, bytes remaining, and active state
- **AND** the span timing is accurate to M-cycle precision

#### Scenario: Trace PPU mode changes

- **WHEN** the PPU changes mode (HBlank, VBlank, OAM scan, pixel transfer)
- **THEN** a trace event is emitted with old mode, new mode, LY register, and cycle count
- **AND** these events are visible in the Perfetto timeline alongside CPU execution

#### Scenario: Trace memory access conflicts

- **WHEN** a memory access is blocked by OAM DMA
- **THEN** a trace event is emitted showing the access type, address, value, and blocked_by_dma flag
- **AND** this helps identify timing issues where CPU and DMA conflict

### Requirement: Trace Comparison via SQL

The debugging workflow SHALL use Perfetto SQL queries to compare execution traces between Ceres and reference emulators.

#### Scenario: Find first divergence point

- **WHEN** a developer loads both Ceres and SameBoy traces into trace_processor
- **THEN** they can execute a SQL query to find the first instruction where execution diverges
- **AND** see register and cycle count differences at that point in the query results

#### Scenario: Agent analyzes divergence programmatically

- **WHEN** an AI agent needs to compare two traces
- **THEN** the agent can execute Perfetto SQL queries via trace_processor API
- **AND** receive structured results showing divergence points
- **AND** parse the JSON/CSV output to understand differences

#### Scenario: Reusable comparison queries

- **WHEN** debugging similar issues repeatedly
- **THEN** developers can use pre-written SQL query templates from `examples/sql/trace_comparison.sql`
- **AND** adapt queries for specific comparison needs (registers, timing, memory access)

### Requirement: SQL Query Pattern Library

The documentation SHALL include a collection of useful Perfetto SQL queries for common debugging patterns.

#### Scenario: Find tight loops

- **WHEN** a developer needs to identify infinite loops or hot paths
- **THEN** they can use the provided SQL query that finds same PC executed >100 times in <1ms
- **AND** the query returns PC addresses ordered by execution count

#### Scenario: Track DMA uploads

- **WHEN** debugging OAM DMA timing issues
- **THEN** the developer can use the provided SQL query that finds all DMA spans
- **AND** see timing, duration, and source address for each DMA operation

#### Scenario: Find memory hotspots

- **WHEN** optimizing memory access patterns
- **THEN** the developer can use the provided SQL query that counts accesses per address
- **AND** identify the top 20 most frequently accessed memory locations

#### Scenario: Agent adapts query patterns

- **WHEN** an AI agent needs to find a specific pattern not covered by existing queries
- **THEN** the agent can read `docs/debugging-sql-queries.md` and adapt example queries
- **AND** execute custom queries via trace_processor or Python bindings

### Requirement: Comprehensive Documentation

The project SHALL provide documentation for debugging workflows using Perfetto and trace analysis.

#### Scenario: New contributor follows debugging guide

- **WHEN** a new contributor encounters a failing Mooneye test
- **THEN** they can follow `docs/debugging.md` to collect traces and visualize in Perfetto
- **AND** complete the debugging workflow without external assistance

#### Scenario: Developer finds timing-specific guidance

- **WHEN** debugging a hardware timing issue (like OAM DMA conflicts)
- **THEN** `docs/debugging-timing-issues.md` provides specific guidance on using traces to identify timing problems
- **AND** includes examples of what correct vs incorrect timing looks like in traces

#### Scenario: Agent understands trace format

- **WHEN** an AI agent needs to parse trace files programmatically
- **THEN** `docs/trace-format.md` documents the Chrome Trace Event Format structure
- **AND** describes Game Boy-specific extensions (DMA state, PPU state, memory conflicts)

### Requirement: Zero Performance Impact When Disabled

The tracing system SHALL have zero performance impact when not explicitly enabled.

#### Scenario: Normal test execution without tracing

- **WHEN** tests run without the `--trace` flag
- **THEN** execution speed matches baseline (no tracing overhead)
- **AND** no trace files are created

#### Scenario: CI runs without tracing

- **WHEN** GitHub Actions CI runs the test suite
- **THEN** tests complete in normal time (<5 minutes)
- **AND** tracing is not enabled by default
