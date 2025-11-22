-- Register Change Tracking
-- Tracks how CPU registers change over time
-- Useful for debugging algorithms and following data flow

-- This query extracts register values from CPU execution events
-- and shows when they change
WITH cpu_args AS (
  SELECT
    s.ts,
    a.key,
    COALESCE(a.string_value, CAST(a.int_value AS TEXT)) AS value
  FROM slice s
  JOIN args a ON s.arg_set_id = a.arg_set_id
  WHERE s.cat = 'cpu_execution'
),
register_values AS (
  SELECT
    ts / 1000000.0 AS time_ms,
    MAX(CASE WHEN key = 'args.pc' THEN value END) AS pc,
    MAX(CASE WHEN key = 'args.instruction' THEN value END) AS instruction,
    CAST(MAX(CASE WHEN key = 'args.a' THEN value END) AS INT) AS reg_a,
    CAST(MAX(CASE WHEN key = 'args.f' THEN value END) AS INT) AS reg_f,
    CAST(MAX(CASE WHEN key = 'args.b' THEN value END) AS INT) AS reg_b,
    CAST(MAX(CASE WHEN key = 'args.c' THEN value END) AS INT) AS reg_c,
    CAST(MAX(CASE WHEN key = 'args.d' THEN value END) AS INT) AS reg_d,
    CAST(MAX(CASE WHEN key = 'args.e' THEN value END) AS INT) AS reg_e,
    CAST(MAX(CASE WHEN key = 'args.h' THEN value END) AS INT) AS reg_h,
    CAST(MAX(CASE WHEN key = 'args.l' THEN value END) AS INT) AS reg_l,
    CAST(MAX(CASE WHEN key = 'args.sp' THEN value END) AS INT) AS reg_sp
  FROM cpu_args
  GROUP BY ts
),
register_changes AS (
  SELECT
    time_ms,
    pc,
    instruction,
    reg_a,
    reg_f,
    reg_b,
    reg_c,
    reg_d,
    reg_e,
    reg_h,
    reg_l,
    reg_sp,
    reg_a != LAG(reg_a) OVER (ORDER BY time_ms) AS a_changed,
    reg_f != LAG(reg_f) OVER (ORDER BY time_ms) AS f_changed,
    reg_b != LAG(reg_b) OVER (ORDER BY time_ms) AS b_changed,
    reg_c != LAG(reg_c) OVER (ORDER BY time_ms) AS c_changed,
    reg_d != LAG(reg_d) OVER (ORDER BY time_ms) AS d_changed,
    reg_e != LAG(reg_e) OVER (ORDER BY time_ms) AS e_changed,
    reg_h != LAG(reg_h) OVER (ORDER BY time_ms) AS h_changed,
    reg_l != LAG(reg_l) OVER (ORDER BY time_ms) AS l_changed,
    reg_sp != LAG(reg_sp) OVER (ORDER BY time_ms) AS sp_changed
  FROM register_values
)
SELECT
  ROUND(time_ms, 3) AS time_ms,
  pc,
  instruction,
  CASE WHEN a_changed THEN printf('A=%02X', reg_a) ELSE '' END AS a,
  CASE WHEN f_changed THEN printf('F=%02X', reg_f) ELSE '' END AS f,
  CASE WHEN b_changed THEN printf('B=%02X', reg_b) ELSE '' END AS b,
  CASE WHEN c_changed THEN printf('C=%02X', reg_c) ELSE '' END AS c,
  CASE WHEN d_changed THEN printf('D=%02X', reg_d) ELSE '' END AS d,
  CASE WHEN e_changed THEN printf('E=%02X', reg_e) ELSE '' END AS e,
  CASE WHEN h_changed THEN printf('H=%02X', reg_h) ELSE '' END AS h,
  CASE WHEN l_changed THEN printf('L=%02X', reg_l) ELSE '' END AS l,
  CASE WHEN sp_changed THEN printf('SP=%04X', reg_sp) ELSE '' END AS sp
FROM register_changes
WHERE a_changed OR f_changed OR b_changed OR c_changed 
   OR d_changed OR e_changed OR h_changed OR l_changed OR sp_changed
ORDER BY time_ms
LIMIT 1000;

-- INTERPRETATION:
-- This shows only the instructions that modify registers
-- Useful for:
-- - Following data flow through registers
-- - Debugging arithmetic operations
-- - Tracking function parameters (often in BC, DE, HL)
-- - Understanding algorithm behavior
--
-- Common patterns:
-- - LD instructions move data between registers
-- - ALU ops (ADD, SUB, etc.) modify A and F
-- - PUSH/POP modify SP
-- - F register changes indicate flag modifications (Z, N, H, C)
