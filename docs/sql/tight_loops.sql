-- Tight Loop Detection
-- Identifies locations where the CPU is executing the same instruction repeatedly
-- Useful for finding infinite loops or busy-wait loops

-- Note: This simplified version groups by instruction to find repeated patterns
WITH cpu_args AS (
  SELECT
    s.ts,
    a.key,
    COALESCE(a.string_value, CAST(a.int_value AS TEXT)) AS value
  FROM slice s
  JOIN args a ON s.arg_set_id = a.arg_set_id
  WHERE s.cat = 'cpu_execution'
),
instruction_data AS (
  SELECT
    ts,
    MAX(CASE WHEN key = 'args.pc' THEN value END) AS pc,
    MAX(CASE WHEN key = 'args.instruction' THEN value END) AS instruction
  FROM cpu_args
  GROUP BY ts
),
instruction_counts AS (
  SELECT
    pc,
    instruction,
    COUNT(*) AS execution_count,
    MIN(ts) / 1e6 AS first_seen_ms,
    MAX(ts) / 1e6 AS last_seen_ms
  FROM instruction_data
  WHERE pc IS NOT NULL AND instruction IS NOT NULL
  GROUP BY pc, instruction
  HAVING COUNT(*) >= 100  -- Only show instructions executed 100+ times
)
SELECT
  pc,
  instruction,
  execution_count,
  CAST((last_seen_ms - first_seen_ms) AS INT) AS active_duration_ms,
  CAST(first_seen_ms AS INT) AS first_seen_ms,
  CAST(last_seen_ms AS INT) AS last_seen_ms
FROM instruction_counts
ORDER BY execution_count DESC
LIMIT 50;

-- INTERPRETATION:
-- - High consecutive_executions with short instructions = tight busy loop
-- - Common patterns:
--   * JR relative jump to itself = infinite loop (bug)
--   * HALT instruction repeated = waiting for interrupt (normal)
--   * LD/CP sequence repeated = polling register (check if waiting for hardware)
