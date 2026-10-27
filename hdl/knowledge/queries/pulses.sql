-- The short pulses at controls, by signal, with their race if classified.
SELECT entity, signal, kind, roms, pulses, round(shortest_ns, 2) AS shortest_ns, a_rom, race
FROM runt_race
ORDER BY shortest_ns >= 10, entity, signal;
