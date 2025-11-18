-- PPU Mode Timeline Analysis
-- Shows PPU mode transitions and timing per scanline
-- Useful for debugging PPU timing issues and understanding rendering

WITH ppu_events AS (
  SELECT
    s.ts,
    CAST((SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.ly') AS INT) AS scanline,
    (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.mode') AS mode,
    CAST((SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.dots') AS INT) AS dots
  FROM slice s
  WHERE s.cat = 'ppu'
)
SELECT
  scanline,
  mode,
  dots AS dots_remaining,
  COUNT(*) AS occurrence_count,
  CAST(MIN(ts) / 1000.0 AS INT) AS first_seen_us,
  CAST(MAX(ts) / 1000.0 AS INT) AS last_seen_us
FROM ppu_events
WHERE scanline IS NOT NULL AND scanline < 154  -- Only visible scanlines (0-153)
GROUP BY scanline, mode, dots
ORDER BY scanline, first_seen_us
LIMIT 200;

-- INTERPRETATION:
-- Game Boy PPU modes per scanline (normal timing):
-- 1. OAM Scan: ~80 dots (20 µs @ 4.19 MHz)
-- 2. Drawing: 172-289 dots (43-72 µs, varies by scroll)
-- 3. HBlank: 87-204 dots (22-51 µs, varies inversely with Drawing)
-- 4. VBlank: 456 dots per line × 10 lines (1140 µs total)
--
-- Look for:
-- - Abnormal mode durations = timing bug
-- - Missing modes = state machine error
-- - modes_per_line != 4 (except VBlank) = unexpected transitions
