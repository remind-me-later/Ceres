-- Where ceres departs from the netlist, and what each rests on.
SELECT f.id, f.hardware,
       (SELECT count(*) FROM fact_test WHERE fact = f.id) AS tests,
       (SELECT count(*) FROM fact_open WHERE fact = f.id) AS open_questions,
       f.title
FROM fact f
WHERE f.netlist = 'differs'
ORDER BY f.id;
