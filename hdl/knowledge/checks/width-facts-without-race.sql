-- The facts on a width race of the netlist that no race names.
SELECT f.id, f.title
FROM fact f
WHERE f.race = 'width' AND f.netlist <> 'none'
  AND NOT EXISTS (SELECT 1 FROM race_fact rf WHERE rf.fact = f.id);
