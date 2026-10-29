-- The rules applied in rtl/ that no `@rule` tag shows, and the tags that name no rule.
SELECT r.id AS rule, 'no tag' AS problem
FROM rule r
WHERE r.status = 'applied'
  AND NOT EXISTS (SELECT 1 FROM rule_tag t WHERE t.rule = r.id)
UNION ALL
SELECT t.rule, 'tag at ' || t.file || ':' || t.line || ' of a rule not applied'
FROM rule_tag t
WHERE NOT EXISTS (SELECT 1 FROM rule r WHERE r.id = t.rule AND r.status = 'applied');
