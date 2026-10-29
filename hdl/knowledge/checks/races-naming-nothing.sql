-- The delays and signals of races.toml that rtl/ does not have.
SELECT race, entity, signal, 'delay' AS what
FROM race_delay rd
WHERE NOT (rd.entity = '' AND rd.signal = 'tco')
  AND NOT EXISTS (SELECT 1 FROM delay d WHERE d.entity = rd.entity AND d.signal = rd.signal)
UNION ALL
SELECT race, entity, signal, 'signal'
FROM race_signal rs
WHERE NOT EXISTS (SELECT 1 FROM signal s WHERE s.entity = rs.entity AND s.name = rs.signal);
