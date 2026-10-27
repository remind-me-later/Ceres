-- The delays of rtl/ that no race of races.toml covers.
SELECT file, line, entity, signal, delays
FROM delay_race
WHERE race IS NULL
ORDER BY file, line;
