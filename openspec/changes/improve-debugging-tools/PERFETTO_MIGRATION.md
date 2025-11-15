# Migration Analysis: Custom Tools → Perfetto SQL

This document analyzes existing custom trace analysis tools and identifies which can be replaced with Perfetto SQL
queries.

## Current Custom Tooling

### 1. **trace-diff** (`src/bin/trace-diff.rs`) - ~350 lines

**Purpose:** Compare two trace files to find execution differences

**Current Features:**

- Load JSONL traces
- Compare PC, instruction, and register values
- Show context around differences
- Display difference statistics
- Field-specific filtering

**Perfetto Replacement:** ✅ **FULLY REPLACEABLE**

Perfetto can do this better with SQL:

```sql
-- Load both traces in trace_processor
-- Find first divergence
WITH ceres AS (
  SELECT ROW_NUMBER() OVER (ORDER BY ts) AS seq,
         ts, args.string_value AS pc
  FROM slice JOIN args ON slice.arg_set_id = args.arg_set_id
  WHERE slice.name = 'cpu_instruction' AND args.key = 'pc'
),
sameboy AS (
  SELECT ROW_NUMBER() OVER (ORDER BY ts) AS seq,
         ts, args.string_value AS pc
  FROM slice JOIN args ON slice.arg_set_id = args.arg_set_id
  WHERE slice.name = 'cpu_instruction' AND args.key = 'pc'
)
SELECT c.seq, c.pc AS ceres_pc, s.pc AS sameboy_pc
FROM ceres c LEFT JOIN sameboy s ON c.seq = s.seq
WHERE c.pc != s.pc
LIMIT 1;
```

**Recommendation:** DELETE - Replace with SQL query templates

---

### 2. **trace-patterns** (`src/bin/trace-patterns.rs`) - ~350 lines

**Purpose:** Detect execution patterns (tight loops, repeated sequences, hotspots)

**Current Features:**

- Detect tight loops (same PC repeating)
- Find repeating instruction sequences
- PC distribution analysis
- Instruction frequency histogram

**Perfetto Replacement:** ✅ **FULLY REPLACEABLE**

All features map directly to SQL:

```sql
-- Tight loops (already shown in proposal)
SELECT args.string_value AS pc, COUNT(*) AS count
FROM slice JOIN args ON slice.arg_set_id = args.arg_set_id
WHERE slice.name = 'cpu_instruction' AND args.key = 'pc'
GROUP BY pc
HAVING count > 100
ORDER BY count DESC;

-- Instruction frequency
SELECT slice.name, COUNT(*) AS count
FROM slice
WHERE slice.name = 'cpu_instruction'
GROUP BY name
ORDER BY count DESC;
```

**Recommendation:** DELETE - Replace with SQL query templates

---

### 3. **trace-query** (`src/bin/trace-query.rs`) - ~450 lines

**Purpose:** Query interface with indexing for fast lookups

**Current Features:**

- Build index for fast PC/instruction lookups
- Query by PC or instruction
- Extract specific line numbers
- Show statistics

**Perfetto Replacement:** ⚠️ **PARTIALLY REPLACEABLE**

**Replaceable:**

- Query by PC/instruction → Perfetto SQL WHERE clauses
- Statistics → Perfetto SQL aggregates
- Extract lines → Perfetto SQL SELECT with LIMIT/OFFSET

**Not Replaceable:**

- Index building for JSONL format (Perfetto uses its own format)
- Custom line number extraction from JSONL

**Recommendation:** KEEP for JSONL compatibility, but add note that Perfetto provides better querying for Chrome Trace
Event Format

---

### 4. **analyze_traces.py** (~180 lines)

**Purpose:** Python script for trace analysis

**Current Features:**

- Show trace summary
- Find address ranges
- Detect potential loops
- Analyze register changes

**Perfetto Replacement:** ✅ **FULLY REPLACEABLE**

Perfetto Python API provides all functionality:

```python
import perfetto
tp = perfetto.TraceProcessor(trace='trace.json')

# Show summary
result = tp.query('SELECT COUNT(*) FROM slice WHERE name = "cpu_instruction"')

# Find address range
result = tp.query('''
  SELECT * FROM slice JOIN args ON slice.arg_set_id = args.arg_set_id
  WHERE slice.name = 'cpu_instruction'
    AND args.key = 'pc'
    AND args.int_value BETWEEN 0x0100 AND 0x0200
''')

# Detect loops (already shown)
# Register changes (already shown in proposal)
```

**Recommendation:** DELETE - Replace with Perfetto Python examples

---

### 5. **search_traces.sh** (~60 lines)

**Purpose:** Bash script for quick trace searches

**Current Features:**

- Show basic trace info
- Search for terms using jq
- Count instruction frequencies

**Perfetto Replacement:** ✅ **FULLY REPLACEABLE**

Perfetto CLI provides better search:

```bash
# Show info
trace_processor --run-metrics trace.json

# Search for specific PC
trace_processor trace.json <<EOF
SELECT * FROM slice WHERE name LIKE '%0x0150%';
EOF

# Instruction frequency (SQL query already shown)
```

**Recommendation:** DELETE - Replace with Perfetto CLI examples

---

### 6. **TraceIndex** (`src/trace_index.rs`) - Custom indexing

**Purpose:** Build indexes for fast JSONL trace queries

**Perfetto Replacement:** ⚠️ **NOT REPLACEABLE**

Perfetto has its own highly optimized indexing for Chrome Trace Event Format. However, custom JSONL indexing may still
be useful for backward compatibility.

**Recommendation:** KEEP if maintaining JSONL format, otherwise migrate to Chrome Trace Event Format and delete

---

### 7. **JSON Schemas** (`schemas/*.json`)

**Purpose:** Document trace format

**Perfetto Replacement:** ⚠️ **MODIFY**

Chrome Trace Event Format has its own schema. Update documentation to describe how we extend it with Game Boy-specific
fields.

**Recommendation:** UPDATE - Document Chrome Trace Event Format extensions

---

## Summary Table

| Tool              | Lines | Perfetto Replacement | Recommendation           |
| ----------------- | ----- | -------------------- | ------------------------ |
| trace-diff.rs     | ~350  | ✅ Full              | DELETE → SQL             |
| trace-patterns.rs | ~350  | ✅ Full              | DELETE → SQL             |
| trace-query.rs    | ~450  | ⚠️ Partial           | KEEP for JSONL or DELETE |
| analyze_traces.py | ~180  | ✅ Full              | DELETE → Python API      |
| search_traces.sh  | ~60   | ✅ Full              | DELETE → CLI examples    |
| trace_index.rs    | ~500  | ❌ Not needed        | DELETE with JSONL        |
| schemas/\*.json   | ~100  | ⚠️ Modify            | UPDATE documentation     |

**Total Deletable:** ~1,390 lines (if migrating fully to Chrome Trace Event Format) **Total Deletable (conservative):**
~940 lines (keeping JSONL support)

---

## Migration Benefits

### Code Reduction

- **Optimistic:** Delete ~1,390 lines of custom code
- **Conservative:** Delete ~940 lines (keep JSONL tooling)
- Replace with ~100 lines of SQL query templates

### Feature Improvements

1. **More powerful:** Perfetto SQL supports joins, window functions, CTEs
2. **Better performance:** Perfetto's columnar storage and indexes are highly optimized
3. **Visualization:** SQL results can be viewed in Perfetto UI
4. **Standardization:** Industry-standard format and tooling

### Maintenance Reduction

- No need to maintain custom parsers
- No need to optimize indexing algorithms
- Bug fixes and improvements come from Google
- Large community for support

---

## Migration Strategy

### Phase 1: Parallel Operation (Low Risk)

1. Keep existing JSONL tools
2. Add Chrome Trace Event Format export option
3. Document Perfetto SQL equivalents
4. Test both approaches

### Phase 2: Transition (Medium Risk)

1. Default to Chrome Trace Event Format
2. Mark JSONL tools as deprecated
3. Update documentation to recommend Perfetto
4. Provide migration guide

### Phase 3: Cleanup (High Risk)

1. Remove JSONL export
2. Delete deprecated tools
3. Remove custom indexing code
4. Update schemas to Chrome Trace Event Format extensions only

---

## Recommended Timeline

**Immediate (Week 1-2):**

- Add `tracing-chrome` export
- Document basic Perfetto SQL queries
- Create `examples/sql/` with query templates

**Short-term (Month 1):**

- Test Perfetto workflow thoroughly
- Write comprehensive debugging documentation
- Create examples showing both approaches

**Long-term (Month 2-3):**

- Deprecate JSONL tools
- Update all documentation to Perfetto-first
- Consider removing custom tools if Perfetto proves sufficient

**Future (Month 4+):**

- Evaluate removing JSONL entirely
- Delete deprecated code
- Simplify codebase

---

## Risk Mitigation

### Risks:

1. **Learning curve:** Team needs to learn Perfetto SQL
2. **Format lock-in:** Chrome Trace Event Format is the only option
3. **External dependency:** Rely on Google's Perfetto project
4. **Workflow changes:** Different debugging process

### Mitigations:

1. **Documentation:** Comprehensive SQL query examples and debugging guides
2. **Examples:** Real-world debugging scenarios documented
3. **Fallback:** Keep JSONL export as backup during transition
4. **Community:** Perfetto is widely used (Chrome DevTools, Android, etc.)

---

## Conclusion

**Recommendation: Migrate to Perfetto SQL**

- Delete ~1,000 lines of custom code
- Gain more powerful query capabilities
- Reduce maintenance burden significantly
- Use industry-standard tooling
- Better agent accessibility (SQL is well-documented)

The migration is low-risk if done gradually (parallel operation first), and the benefits are substantial.
