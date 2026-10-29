-- The rules for this model that no fact about it shows: the model follows another's evidence.
SELECT r.status, r.id,
       (SELECT group_concat(model, ' ') FROM rule_model WHERE rule = r.id) AS models,
       (SELECT group_concat(fact, ' ') FROM rule_fact WHERE rule = r.id) AS facts
FROM rule_here r
WHERE NOT EXISTS (SELECT 1 FROM rule_fact rf JOIN fact_here f ON f.id = rf.fact
                  WHERE rf.rule = r.id)
ORDER BY CASE r.status WHEN 'applied' THEN 0 WHEN 'proposed' THEN 1 ELSE 2 END, r.id;
