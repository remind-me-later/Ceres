-- Execution Divergence Detection
-- Compares execution flow to detect where behavior diverges from expected
-- Useful for regression testing and debugging non-deterministic issues
--
-- Note: This query works on a single trace. To compare two traces,
-- export results from each and compare externally, or load both into
-- Perfetto (requires manual trace merging).

-- This query creates a "fingerprint" of execution that can be compared
WITH execution_summary AS (
  SELECT
    CAST(ts / 1000000 AS INT) AS time_bucket_ms,
    (SELECT string_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'args.pc') AS pc,
    COUNT(*) AS executions
  FROM slice
  WHERE cat = 'cpu_execution'
  GROUP BY time_bucket_ms, pc
),
execution_fingerprint AS (
  SELECT
    time_bucket_ms,
    GROUP_CONCAT(pc || ':' || executions, ',') AS execution_pattern,
    SUM(executions) AS total_instructions,
    COUNT(DISTINCT pc) AS unique_pcs
  FROM execution_summary
  GROUP BY time_bucket_ms
)
SELECT
  time_bucket_ms,
  total_instructions,
  unique_pcs,
  ROUND(1.0 * total_instructions / unique_pcs, 2) AS avg_executions_per_pc,
  SUBSTR(execution_pattern, 1, 100) AS pattern_sample
FROM execution_fingerprint
ORDER BY time_bucket_ms
LIMIT 100;

-- INTERPRETATION:
-- This creates a per-millisecond summary of execution:
-- - total_instructions: How many instructions executed in this ms
-- - unique_pcs: How many different locations were executed
-- - avg_executions_per_pc: Loop intensity (higher = tighter loops)
-- - pattern_sample: Preview of execution pattern
--
-- Usage for comparison:
-- 1. Run this query on reference trace, export results
-- 2. Run on test trace, export results  
-- 3. Compare line by line to find divergence point
-- 4. Look at time_bucket_ms where patterns differ
-- 5. Zoom into that time range in Perfetto to debug
--
-- Look for:
-- - Sudden changes in total_instructions = timing change
-- - Different unique_pcs = execution path diverged
-- - Pattern differences = behavioral change
