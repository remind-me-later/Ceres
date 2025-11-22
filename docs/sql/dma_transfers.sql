-- DMA Transfer Analysis
-- Tracks all DMA operations (OAM DMA and HDMA)
-- Useful for debugging sprite/tile uploads and timing

SELECT
  CASE 
    WHEN name LIKE '%OAM DMA%' THEN 'OAM DMA'
    WHEN name LIKE '%HDMA%' THEN 'HDMA'
    ELSE 'Unknown'
  END AS dma_type,
  (SELECT string_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'args.src') AS source_addr,
  (SELECT string_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'args.dst') AS dest_addr,
  CAST((SELECT string_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'args.bytes') AS INT) AS bytes_transferred,
  (SELECT string_value FROM args WHERE arg_set_id = slice.arg_set_id AND key = 'args.transfer_type') AS transfer_type,
  ts / 1000000.0 AS timestamp_ms,
  LEAD(ts) OVER (ORDER BY ts) - ts AS time_until_next_us
FROM slice
WHERE cat = 'dma'
ORDER BY ts;

-- INTERPRETATION:
-- OAM DMA:
-- - Transfers 160 bytes from ROM/RAM to OAM ($FE00-$FE9F)
-- - Takes 160 µs (40 M-cycles)
-- - Blocks CPU access to most memory during transfer
-- - Source: $XX00-$XX9F where XX is written to $FF46
--
-- HDMA (CGB only):
-- - General Purpose (GP): Transfers immediately, blocks CPU
-- - HBlank: Transfers 16 bytes per HBlank, doesn't block CPU
-- - Common usage: Upload tiles/maps during VBlank or HBlank
--
-- Look for:
-- - bytes_transferred != 160 for OAM DMA = partial/interrupted transfer
-- - Very frequent HDMA = potential performance impact
-- - HDMA during Drawing mode = should not happen
-- - Source addresses in unusual ranges = possible bug
