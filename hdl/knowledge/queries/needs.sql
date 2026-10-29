-- What settling the open questions takes, by how many open facts it serves.
SELECT n.need, count(DISTINCT n.fact) AS facts, group_concat(DISTINCT n.fact) AS which
FROM fact_need n
JOIN fact_open o ON o.fact = n.fact
GROUP BY n.need
ORDER BY facts DESC;
