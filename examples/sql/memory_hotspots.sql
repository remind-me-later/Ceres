-- Memory Access Hotspots
-- Shows which memory addresses are accessed most frequently
-- Useful for identifying hot data structures and optimization opportunities

WITH memory_accesses AS (
  SELECT
    (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.addr') AS address,
    (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.region') AS region,
    (SELECT string_value FROM args WHERE arg_set_id = s.arg_set_id AND key = 'args.value') AS value,
    s.ts
  FROM slice s
  WHERE s.cat = 'memory'
),
access_stats AS (
  SELECT
    address,
    region,
    COUNT(*) AS access_count,
    COUNT(DISTINCT value) AS unique_values,
    MIN(ts) AS first_access_ts,
    MAX(ts) AS last_access_ts
  FROM memory_accesses
  WHERE address IS NOT NULL
  GROUP BY address, region
),
total_accesses AS (
  SELECT SUM(access_count) AS total FROM access_stats
)
SELECT
  a.address,
  a.region,
  a.access_count,
  a.unique_values,
  CAST(100.0 * a.access_count / t.total AS INT) AS percent_of_accesses,
  CAST((a.last_access_ts - a.first_access_ts) / 1000000.0 AS INT) AS active_duration_ms
FROM access_stats a, total_accesses t
ORDER BY a.access_count DESC
LIMIT 100;

-- INTERPRETATION:
-- VRAM ($8000-$9FFF):
-- - Tile data: $8000-$97FF
-- - Background map: $9800-$9BFF or $9C00-$9FFF
-- - High access_count to tile data = dynamic tiles (animations, text)
-- - High access to map = scrolling or screen updates
--
-- OAM ($FE00-$FE9F):
-- - Sprite attributes: Y, X, Tile, Flags for 40 sprites
-- - High access_count = sprite animation or movement
-- - unique_values = 1 = static sprite
-- - unique_values > 1 = animated sprite
--
-- Look for:
-- - Very high access_count at single address = potential optimization
-- - accesses_per_ms = performance metric
-- - Clustered addresses = related data structure
