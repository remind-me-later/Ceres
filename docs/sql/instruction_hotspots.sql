-- Instruction Hotspots
-- Shows which instructions are executed most frequently
-- Useful for performance optimization and understanding program behavior

WITH cpu_args AS (
  SELECT
    s.ts,
    a.key,
    COALESCE(a.string_value, CAST(a.int_value AS TEXT)) AS value,
    a.int_value
  FROM slice s
  JOIN args a ON s.arg_set_id = a.arg_set_id
  WHERE s.cat = 'cpu_execution'
),
instruction_data AS (
  SELECT
    ts,
    MAX(CASE WHEN key = 'args.pc' THEN value END) AS pc,
    MAX(CASE WHEN key = 'args.instruction' THEN value END) AS instruction,
    MAX(CASE WHEN key = 'args.cycles' THEN int_value END) AS cycles
  FROM cpu_args
  GROUP BY ts
),
instruction_stats AS (
  SELECT
    pc,
    instruction,
    COUNT(*) AS execution_count,
    SUM(cycles) AS total_cycles
  FROM instruction_data
  GROUP BY pc, instruction
),
total_count AS (
  SELECT SUM(execution_count) AS total FROM instruction_stats
)
SELECT
  i.pc,
  i.instruction,
  i.execution_count,
  i.total_cycles,
  CAST(100.0 * i.execution_count / t.total AS INT) AS percent_of_total,
  CAST(1.0 * i.total_cycles / i.execution_count AS INT) AS avg_cycles_per_exec
FROM instruction_stats i, total_count t
WHERE i.pc IS NOT NULL
ORDER BY i.execution_count DESC
LIMIT 50;

-- INTERPRETATION:
-- - Top instructions show where the CPU spends most of its time
-- - High execution_count at specific PC = likely in a loop
-- - Compare percent_of_total to find optimization opportunities
-- - avg_cycles_per_exec shows instruction complexity
--
-- Example patterns:
-- - Many HALT instructions = CPU waiting for interrupts (normal for games)
-- - Repeated memory access at same address = hot data structure
-- - High count at specific loop PC = optimization target
