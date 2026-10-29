-- The knowledge database (../knowledge.sh): what the repository knows
-- about the hardware, the readable model and ceres, in one place to query.
-- Every table is generated from the sources; nothing here is edited by
-- hand. The hand-written judgement lives in doc/facts.toml (the facts) and
-- hdl/races.toml (the races' verdicts).

-- The readable model (hdl/rtl, knowledge/rtl.awk, oracle/controls.awk).

-- An instance: the entity it is in, its label, its own entity.
CREATE TABLE instance (parent TEXT, label TEXT, entity TEXT);

-- A signal or a port, by the entity that declares it.
CREATE TABLE signal (entity TEXT, name TEXT);

-- A signal that controls storage: it clocks a flip-flop ("edge"), or a
-- latch, an asynchronous set or reset or a process with no clock tests it
-- ("level"). A bit is name(i), i a number or "i" for any.
CREATE TABLE control (entity TEXT, kind TEXT, signal TEXT);

-- An assignment with delays (`after`): where, the signal assigned, and its
-- delays apart by spaces (tco is a flip-flop's clock-to-output).
CREATE TABLE delay (file TEXT, line INTEGER, entity TEXT, signal TEXT, delays TEXT);

-- An `@hw` tag: what the code rests on (die: the die's cells; measured:
-- dmg-sim or hardware; inferred).
CREATE TABLE hw_tag (file TEXT, line INTEGER, entity TEXT, kind TEXT, text TEXT);

-- The short pulses of the model's runs (find-runts.sh): the ROM, the
-- entity and signal (a bit as name(i)), its path, how many, the shortest
-- (ns), when the first began (ms), their level (0, 1, b: both).
CREATE TABLE runt (rom TEXT, entity TEXT, signal TEXT, path TEXT, pulses INTEGER,
                   shortest_ns REAL, first_ms REAL, level TEXT);

-- The facts (doc/facts.toml; its header says what the fields mean).
CREATE TABLE fact (id TEXT PRIMARY KEY, title TEXT, netlist TEXT, hardware TEXT,
                   emulators TEXT, race TEXT, note TEXT);
CREATE TABLE fact_model (fact TEXT, model TEXT);
CREATE TABLE fact_test (fact TEXT, rom TEXT);
CREATE TABLE fact_open (fact TEXT, question TEXT);
CREATE TABLE fact_need (fact TEXT, need TEXT);
-- An `@fact` tag in the code or the documents.
CREATE TABLE fact_tag (fact TEXT, file TEXT, line INTEGER);

-- The races' verdicts (hdl/races.toml): order or width and the evidence;
-- the facts that rest on each; the delays (signal tco: all the
-- flip-flops') and the signals with short pulses that each covers (a
-- vector's name covers every bit).
CREATE TABLE race (id TEXT PRIMARY KEY, title TEXT, race TEXT, evidence TEXT);
CREATE TABLE race_fact (race TEXT, fact TEXT);
CREATE TABLE race_delay (race TEXT, entity TEXT, signal TEXT);
CREATE TABLE race_signal (race TEXT, entity TEXT, signal TEXT);

-- The rules the model needs beyond the die's connectivity (hdl/rules.toml):
-- kind (delay, outcome, logic, ram, state, analogue, unknown), status
-- (applied, proposed, missing) and the evidence; the facts each explains,
-- the width races it settles, and the `@rule` tags of rtl/.
CREATE TABLE rule (id TEXT PRIMARY KEY, title TEXT, kind TEXT, status TEXT, evidence TEXT);
CREATE TABLE rule_fact (rule TEXT, fact TEXT);
CREATE TABLE rule_race (rule TEXT, race TEXT);
CREATE TABLE rule_tag (rule TEXT, file TEXT, line INTEGER);

-- Views.

-- A signal's name without its bit.
CREATE VIEW runt_bit AS
SELECT *, CASE WHEN instr(signal, '(') > 0 THEN substr(signal, 1, instr(signal, '(') - 1)
               ELSE signal END AS base
FROM runt;

-- The short pulses at a control: what a flip-flop or a latch may take or
-- not by the die's physics.
CREATE VIEW runt_at_control AS
SELECT DISTINCT r.*, c.kind
FROM runt_bit r
JOIN control c ON c.entity = r.entity
              AND (c.signal = r.signal OR c.signal = r.base OR c.signal = r.base || '(i)');

-- The same, a row per signal: in how many ROMs, how many pulses, the
-- shortest, and a ROM that shows the shortest.
CREATE VIEW runt_at_control_by_signal AS
SELECT entity, signal, kind, count(DISTINCT rom) AS roms, sum(pulses) AS pulses,
       min(shortest_ns) AS shortest_ns,
       (SELECT min(r2.rom) FROM runt_at_control r2
        WHERE r2.entity = r.entity AND r2.signal = r.signal
          AND r2.shortest_ns = min(r.shortest_ns)) AS a_rom
FROM runt_at_control r
GROUP BY entity, signal, kind;

-- Which race covers a delay.
CREATE VIEW delay_race AS
SELECT d.*, rd.race
FROM delay d
LEFT JOIN race_delay rd
       ON (rd.entity = '' AND rd.signal = 'tco' AND d.delays = 'tco')
       OR (rd.entity = d.entity AND rd.signal = d.signal);

-- Which race covers a short pulse at a control.
CREATE VIEW runt_race AS
SELECT r.*, rs.race
FROM runt_at_control_by_signal r
LEFT JOIN race_signal rs
       ON rs.entity = r.entity
      AND (rs.signal = r.signal
           OR rs.signal = CASE WHEN instr(r.signal, '(') > 0
                               THEN substr(r.signal, 1, instr(r.signal, '(') - 1)
                               ELSE r.signal END);

-- The facts that need a rule: the model misses a hardware test (netlist
-- differs from ceres where ceres matches the hardware, or both miss it), is
-- silent where a test shows the outcome, or rests on a width race that a
-- test shows.
CREATE VIEW fact_needing_rule AS
SELECT *
FROM fact
WHERE (netlist = 'differs' AND hardware = 'same')
   OR (netlist = 'same' AND hardware = 'differs')
   OR (netlist = 'silent' AND hardware <> 'none')
   OR (race = 'width' AND netlist <> 'none' AND hardware <> 'none');
