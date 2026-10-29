-- The pulses under 10 ns at a control that no race of races.toml covers.
SELECT entity, signal, kind, roms, pulses, shortest_ns, a_rom
FROM runt_race
WHERE race IS NULL AND shortest_ns < 10
ORDER BY entity, signal;
