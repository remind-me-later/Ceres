-- The delays and signals of the races of this model that rtl/ does not have.
SELECT race, entity, signal, 'delay' AS what
FROM race_delay rd
WHERE rd.race IN (SELECT id FROM race_here)
  AND NOT (rd.entity = '' AND rd.signal = 'tco')
  AND NOT EXISTS (SELECT 1 FROM delay d WHERE d.entity = rd.entity AND d.signal = rd.signal)
UNION ALL
SELECT race, entity, signal, 'signal'
FROM race_signal rs
WHERE rs.race IN (SELECT id FROM race_here)
  AND NOT EXISTS (SELECT 1 FROM signal s WHERE s.entity = rs.entity AND s.name = rs.signal);
