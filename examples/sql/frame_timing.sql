-- Frame Timing Analysis
-- Analyzes frame-to-frame timing using VBlank events
-- Useful for identifying frame drops, stuttering, or timing issues

WITH vblank_events AS (
  SELECT
    s.ts,
    ROW_NUMBER() OVER (ORDER BY s.ts) AS frame_number
  FROM slice s
  WHERE s.cat = 'ppu'
    AND (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.mode') = '"VBlank"'
    AND CAST((SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.ly') AS INT) = 144
),
frame_durations AS (
  SELECT
    frame_number,
    ts,
    ts - LAG(ts) OVER (ORDER BY ts) AS frame_duration_ns
  FROM vblank_events
)
SELECT
  frame_number,
  CAST(ts / 1000000.0 AS INT) AS frame_start_ms,
  CAST(frame_duration_ns / 1000000.0 AS INT) AS frame_duration_ms,
  CAST(1000.0 / (frame_duration_ns / 1000000.0) AS INT) AS fps,
  CASE
    WHEN frame_duration_ns / 1000000.0 BETWEEN 16.0 AND 17.0 THEN 'Normal'
    WHEN frame_duration_ns / 1000000.0 < 16.0 THEN 'Fast'
    WHEN frame_duration_ns / 1000000.0 > 17.0 THEN 'Slow'
    ELSE 'Unknown'
  END AS timing_status
FROM frame_durations
WHERE frame_duration_ns IS NOT NULL
ORDER BY frame_number;

-- INTERPRETATION:
-- Normal Game Boy timing:
-- - Frame rate: ~59.73 Hz (59.7275 Hz to be exact)
-- - Frame duration: ~16.74 ms (16.742706 ms)
-- - CPU speed: 4.194304 MHz (DMG/CGB single speed)
-- - Cycles per frame: 70224 (456 dots × 154 lines)
--
-- Timing status:
-- - Normal: Within 1ms of target (acceptable variance)
-- - Fast: Frame completed early (possible timing bug)
-- - Slow: Frame took too long (performance issue or HALT)
--
-- Look for:
-- - Consistent deviation = systematic timing error
-- - Variable frame_duration_ms = frame drops or stuttering
-- - fps < 59 = performance issues
-- - fps > 60 = emulator running too fast
