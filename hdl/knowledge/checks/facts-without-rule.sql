-- The facts that need a rule of rules.toml and have none.
SELECT f.id, f.netlist, f.hardware, f.race, f.title
FROM fact_needing_rule f
WHERE NOT EXISTS (SELECT 1 FROM rule_fact rf WHERE rf.fact = f.id)
ORDER BY f.id;
