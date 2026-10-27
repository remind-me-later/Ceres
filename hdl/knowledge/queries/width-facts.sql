-- The facts on a width race, with its delays, signals and the fact's tags.
SELECT f.id AS fact, r.id AS race,
       (SELECT group_concat(entity || '.' || signal, ' ')
        FROM (SELECT entity, signal FROM race_delay WHERE race = r.id
              UNION SELECT entity, signal FROM race_signal WHERE race = r.id)) AS model,
       (SELECT group_concat(file || ':' || line, ' ') FROM fact_tag WHERE fact = f.id) AS tags
FROM fact f
LEFT JOIN race_fact rf ON rf.fact = f.id
LEFT JOIN race r ON r.id = rf.race
WHERE f.race = 'width'
ORDER BY f.id;
