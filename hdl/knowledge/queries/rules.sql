-- The rules beyond the die's connectivity, with their kind, status and facts.
SELECT r.status, r.kind, r.id,
       (SELECT group_concat(model, ' ') FROM rule_model WHERE rule = r.id) AS models,
       (SELECT group_concat(fact, ' ') FROM rule_fact WHERE rule = r.id) AS facts,
       (SELECT group_concat(file || ':' || line, ' ') FROM rule_tag WHERE rule = r.id) AS tags
FROM rule r
ORDER BY CASE r.status WHEN 'applied' THEN 0 WHEN 'proposed' THEN 1 ELSE 2 END, r.kind, r.id;
