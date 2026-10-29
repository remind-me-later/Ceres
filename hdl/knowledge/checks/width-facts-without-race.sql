-- The facts about the chips looked at on a width race of the netlist that no race of theirs names.
SELECT f.id, f.title
FROM fact_here f
WHERE f.race = 'width' AND f.netlist <> 'none'
  AND NOT EXISTS (SELECT 1 FROM race_fact rf JOIN race_here r ON r.id = rf.race
                  WHERE rf.fact = f.id);
