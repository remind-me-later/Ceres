-- Tight Loop Detection
-- Identifies locations where the CPU is executing the same instruction repeatedly
-- Useful for finding infinite loops or busy-wait loops

-- Note: This simplified version groups by instruction to find repeated patterns
WITH instruction_data AS (
  SELECT
    s.id,
    s.ts,
    s.dur,
    (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.pc') AS pc,
    (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.instruction') AS instruction
  FROM slice s
  WHERE s.cat = 'cpu_execution'
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
