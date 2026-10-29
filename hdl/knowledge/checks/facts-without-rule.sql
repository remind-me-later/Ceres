-- The facts about this model that need a rule of rules.toml for it and have none.
SELECT f.id, f.netlist, f.hardware, f.race, f.title
FROM fact_needing_rule f
WHERE f.id IN (SELECT id FROM fact_here)
  AND NOT EXISTS (SELECT 1 FROM rule_fact rf JOIN rule_here r ON r.id = rf.rule
                  WHERE rf.fact = f.id)
ORDER BY f.id;
