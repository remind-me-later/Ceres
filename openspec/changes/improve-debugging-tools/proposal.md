# Change: Improve Debugging Tools Based on Timing Investigation Experience

## Why

During the investigation of `test_mooneye_call_cc_timing2` failure, several gaps in the debugging tooling became
apparent. While the tracing infrastructure using the standard Rust `tracing` crate is solid, the workflow for debugging
hardware timing issues revealed areas where better tools could have saved significant time.

### Current State Assessment

**What Works Well:**

- Standard `tracing` crate integration provides structured logging
- CPU execution traces capture all necessary register state and instruction details
- Integration tests automatically download and cache test ROMs
- Test runner provides clear pass/fail signals with register values

**What Needs Improvement:**

- No easy way to collect traces for specific failing tests
- Manual trace analysis requires grep/custom scripts
- No trace comparison tools to diff against reference emulators
- Missing OAM DMA state visibility in traces
- No memory access timeline visualization
- Difficult to correlate CPU cycles with DMA/PPU state changes

### Investigation Experience

The `call_cc_timing2` investigation required:

1. **Manual trace collection setup** - Had to temporarily modify test code to enable tracing
2. **Extensive manual analysis** - Used grep and read entire trace files to find patterns
3. **External research** - Fetched SameBoy source code and test ROM source to understand test methodology
4. **Iterative testing** - Multiple compile-test cycles to verify CPU timing fixes
5. **No DMA visibility** - Could not see DMA state changes in execution flow

The investigation succeeded but took significant manual effort. Better tools would have accelerated the process and made
it accessible to contributors less familiar with the codebase.

### Existing Custom Tooling Analysis

The `ceres-test-runner` currently has **~2,040 lines** of custom trace analysis code:

- **trace-diff.rs** (~350 lines) - Compare two JSONL traces to find differences
- **trace-patterns.rs** (~350 lines) - Detect tight loops, hotspots, patterns
- **trace-query.rs** (~450 lines) - Query interface with custom indexing
- **analyze_traces.py** (~180 lines) - Python analysis script
- **search_traces.sh** (~60 lines) - Bash wrapper for jq queries
- **trace_index.rs** (~500 lines) - Custom indexing for JSONL format
- **schemas/\*.json** (~100 lines) - JSON schema documentation

**All of these tools can be replaced with Perfetto SQL queries.**

Detailed analysis in `PERFETTO_MIGRATION.md` (this directory) shows:

- ✅ **trace-diff** → Perfetto SQL JOINs for trace comparison
- ✅ **trace-patterns** → SQL aggregates for loop/hotspot detection
- ✅ **trace-query** → Perfetto's built-in indexing and SQL WHERE clauses
- ✅ **analyze_traces.py** → Perfetto Python API
- ✅ **search_traces.sh** → Perfetto CLI
- ✅ **trace_index.rs** → Perfetto's optimized columnar storage
- ✅ **schemas** → Chrome Trace Event Format with Game Boy extensions

**Migration Strategy:** Eliminate JSONL format entirely, migrate to Chrome Trace Event Format, delete all custom
tooling.

**Impact:** Delete ~1,900 lines of custom code, replace with ~100 lines of SQL query templates.

## What Changes

### Framework Research Summary

Extensive research into existing tracing and debugging frameworks reveals a mature ecosystem that can replace most
custom tooling needs:

**Key Findings:**

- **`tracing` ecosystem** (6.3k stars, 439k dependents) provides comprehensive infrastructure
- **`tracing-chrome`** (43k downloads) exports to Chrome/Perfetto format viewable at ui.perfetto.dev
- **`tracing-subscriber`** provides flexible formatting and filtering (fmt, JSON, etc.)
- **`trace-deck`** (1.7k downloads) offers GUI visualization for tracing tape files
- **Tracy Profiler** (13.4k stars) - industry-standard real-time profiler with nanosecond resolution and timeline views
- **Perfetto** - Google's trace analysis UI with powerful filtering and visualization (ui.perfetto.dev)

**Integration Strategy:**

Leverage existing mature tools rather than building custom solutions. Focus minimal custom work on Game Boy-specific
needs (DMA/PPU state, hardware-accurate timing). Use standard formats (Chrome Trace Event Format) for interoperability
with industry-standard tools.

**Agent-Friendly Design:**

All tools must work well for both AI agents and human developers:

- **Structured data formats** (JSON) over binary formats - agents can parse and analyze
- **Command-line interfaces** with clear output - agents can invoke and parse results
- **Well-documented schemas** - agents can understand trace structure
- **Programmatic access** - agents can query, filter, and compare traces via code
- **Clear error messages** - agents can diagnose and fix issues

Perfetto and Chrome Trace Event Format excel here: JSON-based, well-documented, SQL queryable.

### 1. Test-Specific Trace Collection (Minimal Custom)

Add built-in support for running individual tests with tracing enabled:

```rust
cargo test --package ceres-test-runner -- test_mooneye_call_cc_timing2 --trace
```

Implementation (leveraging `tracing-subscriber`):

- Add `--trace` flag to test runner CLI
- Configure `tracing-chrome` layer when flag is present
- Output to `target/traces/test_name.json` in Chrome Trace Event Format
- Include test ROM name, model, and outcome in trace metadata

**Why custom code needed:** Test runner integration and test metadata embedding.

**Estimated effort:** 1-2 hours (mostly CLI flag and subscriber configuration).

### 2. Trace Visualization (Use Existing Tools)

#### Recommendation: Use ui.perfetto.dev instead of custom tool

Perfetto provides:

- Side-by-side timeline view of CPU execution
- Powerful SQL-based filtering and analysis
- Frame-accurate timing visualization
- Memory access pattern analysis
- Zero maintenance burden

**Perfetto SQL Queries for Common Debugging Patterns:**

Perfetto's trace_processor lets you query traces with SQL to find patterns:

```sql
-- Find tight loops (same PC executed many times in short time)
SELECT
  args.string_value AS pc,
  COUNT(*) AS executions,
  MAX(ts) - MIN(ts) AS duration_ns
FROM slice
JOIN args ON slice.arg_set_id = args.arg_set_id
WHERE slice.name = 'cpu_instruction' AND args.key = 'pc'
GROUP BY pc
HAVING executions > 100 AND duration_ns < 1000000
ORDER BY executions DESC;

-- Find all DMA uploads (OAM DMA spans)
SELECT
  ts / 1000000.0 AS time_ms,
  dur / 1000.0 AS duration_us,
  args.string_value AS src_addr
FROM slice
JOIN args ON slice.arg_set_id = args.arg_set_id
WHERE slice.name = 'oam_dma' AND args.key = 'src_addr'
ORDER BY ts;

-- Find when register A changes value
SELECT
  ts / 1000000.0 AS time_ms,
  LAG(args.int_value) OVER (ORDER BY ts) AS old_a,
  args.int_value AS new_a,
  slice.name AS instruction
FROM slice
JOIN args ON slice.arg_set_id = args.arg_set_id
WHERE args.key = 'a' AND slice.name LIKE 'cpu_instruction'
HAVING old_a != new_a OR old_a IS NULL;

-- Find memory access hotspots
SELECT
  args.string_value AS addr,
  COUNT(*) AS access_count
FROM slice
JOIN args ON slice.arg_set_id = args.arg_set_id
WHERE slice.name = 'memory_access' AND args.key = 'addr'
GROUP BY addr
ORDER BY access_count DESC
LIMIT 20;

-- Find instructions that take longer than expected
SELECT
  ts / 1000000.0 AS time_ms,
  slice.name AS instruction,
  dur / 1000.0 AS duration_us,
  args.string_value AS pc
FROM slice
JOIN args ON slice.arg_set_id = args.arg_set_id
WHERE slice.name = 'cpu_instruction'
  AND args.key = 'pc'
  AND dur > 50000  -- > 50 T-states
ORDER BY dur DESC;
```

**Agents can use these queries too:**

```bash
# Run SQL query via trace_processor (command-line tool)
trace_processor --httpd trace.json
# Then POST SQL queries to http://localhost:9001/query

# Or use Python bindings
pip install perfetto
python -c "
import perfetto
tp = perfetto.TraceProcessor(trace='trace.json')
result = tp.query('SELECT COUNT(*) FROM slice WHERE name = \"oam_dma\"')
print(result)
"
```

Usage workflow (human):

```bash
# Run test with tracing
cargo test -- test_name --trace

# Open in browser
xdg-open https://ui.perfetto.dev
# Then drag-and-drop target/traces/test_name.json
```

Usage workflow (agent):

```rust
// Agents can parse JSON traces programmatically
let trace = std::fs::read_to_string("target/traces/test.json")?;
let events: Vec<TraceEvent> = serde_json::from_str(&trace)?;

// Find first CPU instruction event
let first_instr = events.iter()
    .find(|e| e.name == "cpu_instruction")
    .expect("No CPU instructions found");

// Agents can also use Perfetto's trace_processor for SQL queries
// https://perfetto.dev/docs/analysis/trace-processor
```

#### Alternative: Tracy Profiler for real-time visualization

For development/debugging, Tracy offers:

- Real-time trace capture with nanosecond resolution
- Beautiful timeline views with memory access visualization
- CPU/GPU profiling (could track PPU as separate thread)
- Rust bindings available (rust_tracy_client)

**No custom tool needed:** Perfetto and Tracy provide superior visualization to anything we could build.

### 3. Enhanced Trace Information (Game Boy Specific Custom)

Expand trace events to include Game Boy hardware state using `tracing::span!` and `tracing::event!`:

```rust
// In DMA code
let span = tracing::span!(Level::DEBUG, "oam_dma",
    src_addr = self.source,
    bytes_remaining = self.bytes_remaining,
    active = self.active
);

// In PPU code
tracing::event!(Level::DEBUG, "ppu_mode_change",
    old_mode = ?old_mode,
    new_mode = ?new_mode,
    ly = self.ly,
    cycle = self.cycle
);

// Memory accesses
tracing::event!(Level::TRACE, "memory_access",
    access_type = "write",
    addr = addr,
    value = value,
    blocked_by_dma = true
);
```

**Why custom code needed:** Game Boy-specific hardware state not in standard profilers.

**Estimated effort:** 4-6 hours (add tracing to DMA, PPU, memory subsystems).

### 4. Trace Comparison (Use Perfetto SQL - No Custom Tool)

**Use Perfetto SQL queries for trace comparison instead of building a custom tool.**

Perfetto can load multiple traces and compare them with SQL joins:

```bash
# Load both traces into trace_processor
trace_processor --httpd ceres_trace.json sameboy_trace.json
```

**SQL Query to Find First Divergence:**

```sql
-- Compare CPU instruction execution between two traces
WITH ceres AS (
  SELECT
    ROW_NUMBER() OVER (ORDER BY ts) AS seq,
    ts,
    args.string_value AS pc,
    (SELECT int_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'a') AS reg_a,
    (SELECT int_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'f') AS reg_f
  FROM slice
  JOIN args ON slice.arg_set_id = args.arg_set_id
  WHERE slice.name = 'cpu_instruction' AND args.key = 'pc'
),
sameboy AS (
  SELECT
    ROW_NUMBER() OVER (ORDER BY ts) AS seq,
    ts,
    args.string_value AS pc,
    (SELECT int_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'a') AS reg_a,
    (SELECT int_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'f') AS reg_f
  FROM slice
  JOIN args ON slice.arg_set_id = args.arg_set_id
  WHERE slice.name = 'cpu_instruction' AND args.key = 'pc'
)
SELECT
  c.seq AS instruction_num,
  c.ts / 1000000.0 AS ceres_time_ms,
  s.ts / 1000000.0 AS sameboy_time_ms,
  c.pc AS ceres_pc,
  s.pc AS sameboy_pc,
  c.reg_a AS ceres_a,
  s.reg_a AS sameboy_a,
  c.reg_f AS ceres_f,
  s.reg_f AS sameboy_f
FROM ceres c
LEFT JOIN sameboy s ON c.seq = s.seq
WHERE c.pc != s.pc OR c.reg_a != s.reg_a OR c.reg_f != s.reg_f
LIMIT 1;  -- First divergence
```

**Simpler Query - Just Count Differences:**

```sql
-- Quick check: do traces match?
SELECT
  (SELECT COUNT(*) FROM slice WHERE name = 'cpu_instruction') AS ceres_count,
  (SELECT COUNT(*) FROM slice WHERE name = 'cpu_instruction') AS sameboy_count,
  CASE
    WHEN ceres_count = sameboy_count THEN 'MATCH'
    ELSE 'DIVERGE'
  END AS result;
```

**Benefits of SQL approach:**

- **No custom tool to maintain** - Zero lines of code
- **More powerful** - Can compare any aspect of execution (timing, memory access, DMA)
- **Flexible** - Easy to adapt queries for different comparison needs
- **Agent-friendly** - Agents can execute SQL queries programmatically
- **Debuggable** - Can inspect intermediate results in Perfetto UI

**For convenience, provide SQL query templates:**

Create `examples/sql/trace_comparison.sql` with common comparison patterns that both humans and agents can use.

### 5. Test Suite Debugging Helpers (Minimal Custom)

Add simple API for trace configuration in tests:

```rust
// In test code
let mut runner = TestRunner::new(rom, config)
    .with_chrome_tracing("target/traces/test.json")  // Uses tracing-chrome
    .run();
```

Keep it simple - just enable/disable tracing. Advanced features (breakpoints, watchpoints) are out of scope for now.

**Why custom needed:** Test runner API convenience.

**Estimated effort:** 1-2 hours (simple builder pattern).

### 6. Documentation and Examples

Create documentation focused on using existing tools:

- **docs/debugging.md**: Overview of Perfetto/Tracy workflow
- **docs/debugging-timing-issues.md**: Specific guide using trace visualization
- **docs/debugging-sql-queries.md**: Collection of useful Perfetto SQL queries for common patterns
- **docs/trace-format.md**: Chrome Trace Event Format reference + Game Boy extensions
- **examples/debug_mooneye_test.rs**: Complete workflow with ui.perfetto.dev
- **examples/sql/**: Directory of .sql files for common queries (tight loops, DMA, hotspots)

**Estimated effort:** 3-4 hours (documentation, examples, and SQL query collection).

## Impact

### Affected Components

**New Dependencies:**

- `tracing-chrome` - Chrome Trace Event Format export
- (Optional) `rust_tracy_client` - Tracy profiler integration if desired

**Modified Files:**

- **Modified**: `ceres-test-runner/src/test_runner.rs` - Add `--trace` flag and Chrome tracing configuration
- **Modified**: `ceres-test-runner/Cargo.toml` - Add `tracing-chrome` dependency
- **Modified**: `ceres-core/src/memory/dma.rs` - Add tracing for DMA operations
- **Modified**: `ceres-core/src/ppu/mod.rs` - Add tracing for PPU state changes
- **Modified**: `ceres-core/src/memory/mod.rs` - Add tracing for memory access conflicts

- **New**: `docs/debugging.md` - Debugging guide with Perfetto/Tracy workflow
- **New**: `docs/debugging-timing-issues.md` - Timing-specific debugging guide
- **New**: `docs/debugging-sql-queries.md` - Perfetto SQL query patterns (tight loops, DMA, hotspots)
- **New**: `docs/trace-format.md` - Chrome Trace Event Format + Game Boy extensions
- **New**: `examples/debug_mooneye_test.rs` - Complete debugging workflow example
- **New**: `examples/sql/` - Directory of reusable .sql query files
- **New**: `PERFETTO_MIGRATION.md` - Detailed migration analysis document (this directory)

**Deleted Files (JSONL Tooling Removal):**

- **Deleted**: `ceres-test-runner/src/bin/trace-diff.rs` (~350 lines) → Replaced by Perfetto SQL
- **Deleted**: `ceres-test-runner/src/bin/trace-patterns.rs` (~350 lines) → Replaced by Perfetto SQL
- **Deleted**: `ceres-test-runner/src/bin/trace-query.rs` (~450 lines) → Replaced by Perfetto SQL
- **Deleted**: `ceres-test-runner/analyze_traces.py` (~180 lines) → Replaced by Perfetto Python API
- **Deleted**: `ceres-test-runner/search_traces.sh` (~60 lines) → Replaced by Perfetto CLI
- **Deleted**: `ceres-test-runner/src/trace_index.rs` (~500 lines) → Perfetto handles indexing
- **Deleted**: `ceres-test-runner/src/test_tracer.rs` - JSONL export logic
- **Deleted**: `ceres-test-runner/schemas/` - JSONL-specific schemas
- **Modified**: `ceres-test-runner/README.md` - Update to document Perfetto workflow

**Total Estimated Implementation:** 12-16 hours

- Test runner integration (Chrome tracing): 2-3 hours
- DMA/PPU/memory tracing: 4-6 hours
- Documentation/examples/SQL queries: 4-5 hours
- Delete JSONL tooling and update references: 2 hours

### Benefits

1. **Massive code reduction** - Delete ~1,900 lines of custom tooling, replace with ~100 lines of SQL templates
2. **Minimal maintenance burden** - Leverage mature tools (Perfetto, Tracy) instead of custom visualization
3. **Faster debugging** - Reduce investigation time from hours to minutes with timeline visualization
4. **Better visibility** - See DMA/PPU interactions in Perfetto's powerful UI
5. **Industry-standard format** - Chrome Trace Event Format works with multiple tools
6. **Lower barrier to entry** - Perfetto is familiar to developers from other projects
7. **Better documentation** - Clear examples of proven debugging workflows
8. **Agent-accessible** - AI agents can parse JSON traces and execute SQL queries programmatically
9. **Structured output** - Chrome Trace Event Format with well-documented schema
10. **Powerful pattern detection** - SQL queries can find tight loops, DMA patterns, hotspots without custom code
11. **Reusable queries** - Save common SQL queries as snippets for repeated debugging tasks

### Risks

- **Learning curve**: Contributors need to learn Perfetto UI (mitigated by documentation)
- **Performance**: Tracing overhead may slow down tests (mitigated by opt-in `--trace` flag)
- **Trace file size**: Long-running tests generate large traces (mitigated by filtering, compression)
- **External dependency**: Requires browser for Perfetto (mitigated by Tracy as alternative)

### Testing Strategy

- Add unit tests for trace parsing and comparison logic
- Add integration tests for trace collection with known test ROMs
- Validate timeline visualization with simple test cases
- Document expected trace output format with JSON schema

## Success Criteria

**Human Usage:**

- ✅ Run failing test with `--trace` flag and get Chrome Trace Event Format output
- ✅ Open trace in ui.perfetto.dev and see CPU instruction timeline with register state
- ✅ See DMA/PPU state changes alongside CPU execution in timeline
- ✅ Use Perfetto SQL queries to compare Ceres vs SameBoy traces and find first divergence in < 1 minute
- ✅ New contributor can debug timing issue following documented Perfetto workflow
- ✅ Zero performance impact when tracing is disabled (gated by `--trace` flag)

**Agent Usage:**

- ✅ Agent can invoke `cargo test -- test_name --trace` and parse JSON output
- ✅ Agent can parse Chrome Trace Event Format and extract CPU instruction events
- ✅ Agent can execute Perfetto SQL comparison queries and parse structured results
- ✅ Agent can write simple Rust code to query traces (e.g., find first write to address X)
- ✅ Agent can read trace-format.md and understand schema without human assistance
- ✅ Agent can use Perfetto SQL queries to find tight loops, DMA uploads, and memory hotspots
- ✅ Agent can adapt example SQL queries from docs/debugging-sql-queries.md for specific debugging needs

## Recommended Tools Summary

| Tool                       | Purpose                 | Status     | Maintenance | Agent-Friendly   |
| -------------------------- | ----------------------- | ---------- | ----------- | ---------------- |
| `tracing-chrome`           | Export to Chrome format | Existing   | Upstream    | ✅ JSON output   |
| `ui.perfetto.dev`          | Timeline visualization  | Existing   | Google      | ✅ SQL queries   |
| `Tracy Profiler`           | Real-time profiling     | Existing   | wolfpld     | ⚠️ GUI-focused   |
| Perfetto SQL (all queries) | Pattern detection       | Existing   | Google      | ✅ SQL queries   |
| Test runner `--trace` flag | Trace collection        | New        | Us (~100 L) | ✅ CLI invocable |
| DMA/PPU tracing            | Hardware state logging  | New        | Us (~200 L) | ✅ Structured    |
| ~~trace-diff~~             | ~~Trace comparison~~    | **Delete** | -           | -                |
| ~~trace-patterns~~         | ~~Loop detection~~      | **Delete** | -           | -                |
| ~~trace-query~~            | ~~JSONL queries~~       | **Delete** | -           | -                |
| ~~analyze_traces.py~~      | ~~Python analysis~~     | **Delete** | -           | -                |
| ~~trace_index.rs~~         | ~~JSONL indexing~~      | **Delete** | -           | -                |

**Code Change Summary:**

- **Delete:** ~1,900 lines of custom JSONL tooling
- **Add:** ~300 lines (test integration + hardware tracing + SQL templates)
- **Net:** -1,600 lines (88% reduction in custom tooling)

**Maintenance Burden:** Minimal - most complexity delegated to mature upstream tools

## Migration Analysis

See `PERFETTO_MIGRATION.md` in this directory for comprehensive analysis of replacing custom JSONL tooling with Perfetto
SQL.

**Summary:**

- All existing trace analysis tools can be replaced with Perfetto SQL queries
- Delete ~1,900 lines of custom code
- Chrome Trace Event Format provides better performance and interoperability
- No need to maintain JSONL format or custom indexing

## Future Enhancements (Out of Scope)

- Tracy Profiler integration for real-time debugging (`rust_tracy_client`)
- Interactive debugger with step-through execution
- GDB remote debugging protocol support
- Automated trace regression detection in CI
- Generate reference traces from SameBoy for all Mooneye tests
- Perfetto trace viewer integration in CI (render trace diffs in PRs)
- Perfetto trace viewer integration in CI (render trace diffs in PRs)
