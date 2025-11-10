# Tasks: Add Execution Trace Collection

**Status**: Not Started  
**Estimated Time**: ~12 hours  
**Dependencies**: `add-disassembler-cli` must be complete

## Phase 1: Core Infrastructure (4 hours)

### 1.1 Define Trace Data Structures

- [ ] Create `ceres-core/src/trace.rs` module
- [ ] Define `TraceEntry` struct with fields:
  - `pc: u16` - Program counter at instruction start
  - `instruction: heapless::String<32>` - Disassembled instruction
  - `cycles: u8` - Cycles consumed by instruction
  - `a, f, b, c, d, e, h, l: u8` - Register snapshot
  - `sp: u16` - Stack pointer
- [ ] Define `RegisterSnapshot` struct for cleaner organization
- [ ] Add `#[derive(Debug, Clone)]` to all trace types
- [ ] Document struct fields with inline comments

### 1.2 Implement Circular Buffer

- [ ] Define `TraceBuffer` struct with:
  - `entries: heapless::Vec<TraceEntry, N>` - Circular buffer storage
  - `head: usize` - Write position in buffer
  - `size: usize` - Current number of entries
  - `enabled: bool` - Collection enable flag
- [ ] Implement `TraceBuffer::new()` constructor
- [ ] Implement `TraceBuffer::push()` for adding entries
- [ ] Implement circular wraparound logic when buffer fills
- [ ] Implement `TraceBuffer::clear()` to reset buffer
- [ ] Add `TraceBuffer::enable()` and `disable()` methods
- [ ] Add `TraceBuffer::is_enabled()` getter
- [ ] Write unit tests for buffer wraparound behavior
- [ ] Write unit tests for enable/disable toggling

### 1.3 Add Buffer to Gb Struct

- [ ] Add `trace_buffer: TraceBuffer` field to `Gb` struct in `lib.rs`
- [ ] Initialize buffer in `Gb::new()` with disabled state
- [ ] Add `DEFAULT_TRACE_CAPACITY` constant (1000 entries)
- [ ] Document buffer field in struct docs
- [ ] Update `Gb::reset()` to clear trace buffer

## Phase 2: Collection Integration (3 hours)

### 2.1 Hook Into CPU Execution

- [ ] Locate `run_cpu()` in `sm83.rs` where instructions execute
- [ ] Add trace collection call after instruction execution
- [ ] Check `trace_buffer.is_enabled()` before collecting
- [ ] Call `gb.disasm_at(pc)` to get instruction string
- [ ] Capture register state before instruction modifies it
- [ ] Store cycle count from instruction execution result
- [ ] Handle CB-prefixed instructions correctly
- [ ] Add debug assertions to verify trace correctness

### 2.2 Optimize Collection Performance

- [ ] Profile overhead of trace collection with benchmarks
- [ ] Optimize register snapshot copying (memcpy if beneficial)
- [ ] Consider pre-allocating instruction strings
- [ ] Add inline hints to critical path functions
- [ ] Ensure <5% performance impact when enabled
- [ ] Document performance characteristics in code comments

## Phase 3: Query API (2 hours)

### 3.1 Implement Basic Queries

- [ ] Add `Gb::trace_entries()` to get all entries as slice
- [ ] Add `Gb::trace_last_n(n)` to get last N entries
- [ ] Add `Gb::trace_count()` to get current entry count
- [ ] Add `Gb::trace_capacity()` to get buffer max size
- [ ] Document all query methods with examples
- [ ] Write unit tests for each query method

### 3.2 Implement Advanced Filtering

- [ ] Add `Gb::trace_filter(predicate)` with closure parameter
- [ ] Add `Gb::trace_range(start_pc, end_pc)` for address ranges
- [ ] Add `Gb::trace_find_instruction(mnemonic)` for instruction search
- [ ] Consider iterator-based API for better ergonomics
- [ ] Write tests for complex filter combinations
- [ ] Document performance characteristics of filters

### 3.3 Add Trace Control API

- [ ] Add `Gb::trace_enable()` public method
- [ ] Add `Gb::trace_disable()` public method
- [ ] Add `Gb::trace_clear()` to reset buffer
- [ ] Add `Gb::trace_is_enabled()` getter
- [ ] Document control flow in module docs

## Phase 4: Export Functionality (1 hour)

### 4.1 JSON Export Implementation

- [ ] Create `ceres-std/src/trace_export.rs` module
- [ ] Add `serde` and `serde_json` dependencies to `ceres-std/Cargo.toml`
- [ ] Implement `export_trace_json(gb: &Gb) -> String` function
- [ ] Format JSON with instruction array structure
- [ ] Include metadata: timestamp, buffer size, entry count
- [ ] Write example JSON output to documentation
- [ ] Write tests comparing JSON output against expected format

### 4.2 Binary Export (Optional)

- [ ] Consider compact binary format for large traces
- [ ] Document format specification if implemented
- [ ] Provide conversion tools to/from JSON

## Phase 5: CLI Integration (1 hour)

### 5.1 Add CLI Flags

- [ ] Add `--trace-buffer-size N` flag to CLI parser
- [ ] Add `--trace-export FILE` flag for JSON output path
- [ ] Add `--trace-enable` flag to start with tracing on
- [ ] Update `clap` argument definitions in `ceres-std/src/cli.rs`
- [ ] Update help text with flag descriptions
- [ ] Validate buffer size argument (1-100000 range)

### 5.2 Wire Up Flags to Emulator

- [ ] Initialize trace buffer with specified capacity
- [ ] Enable tracing if `--trace-enable` flag present
- [ ] Export trace to JSON file on emulator exit
- [ ] Handle file write errors gracefully
- [ ] Add logging statements for trace operations
- [ ] Update README with trace collection examples

## Phase 6: Test Runner Integration (1 hour)

### 6.1 Add Trace Export to Test Runner

- [ ] Modify `test_runner.rs` to capture traces during tests
- [ ] Export trace JSON when test fails
- [ ] Save traces to `target/traces/<test_name>.json`
- [ ] Log trace file path on test failure
- [ ] Add option to export traces for passing tests too
- [ ] Document trace inspection workflow in `ceres-test-runner/README.md`

### 6.2 Create Trace Analysis Examples

- [ ] Write Python script to analyze trace JSON files
- [ ] Show example: finding bank switch instructions
- [ ] Show example: computing instruction histogram
- [ ] Show example: detecting infinite loops
- [ ] Document analysis workflows in test runner docs

## Documentation (30 minutes)

- [ ] Add module-level docs to `ceres-core/src/trace.rs`
- [ ] Document trace collection overhead and best practices
- [ ] Add usage examples to AGENTS.md for AI agent debugging
- [ ] Update CONTRIBUTING.md with trace debugging section
- [ ] Create example agent workflow in openspec docs

## Summary

**Total Tasks**: 76  
**Estimated completion time**: 12 hours
