-- The delays of rtl/ other than the flip-flops', with their race.
SELECT entity, signal, delays, file || ':' || line AS site, race
FROM delay_race
WHERE delays <> 'tco'
ORDER BY entity, signal;
