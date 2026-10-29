-- The races of the model, with what they cover and the facts on each.
SELECT r.race, r.id,
       (SELECT group_concat(model, ' ') FROM race_model WHERE race = r.id) AS models,
       (SELECT group_concat(entity || '.' || signal, ' ') FROM race_delay WHERE race = r.id) AS delays,
       (SELECT group_concat(entity || '.' || signal, ' ') FROM race_signal WHERE race = r.id) AS signals,
       (SELECT group_concat(fact, ' ') FROM race_fact WHERE race = r.id) AS facts
FROM race r
ORDER BY r.race DESC, r.id;
