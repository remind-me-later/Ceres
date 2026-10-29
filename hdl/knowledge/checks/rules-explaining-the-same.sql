-- The rules that explain the same facts as another: one of them is enough.
WITH explained AS (
  SELECT rule, group_concat(fact, ' ') AS facts
  FROM (SELECT * FROM rule_fact ORDER BY rule, fact)
  GROUP BY rule
)
SELECT a.rule, b.rule AS also, a.facts
FROM explained a
JOIN explained b ON a.facts = b.facts AND a.rule < b.rule;
