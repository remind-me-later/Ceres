# The knowledge base

`knowledge.sh` gathers what the repository knows into tables,
`hdl/build/knowledge/*.tsv`, generated from the sources each time: the model's
instances, its signals and the ones that control its storage, its delays
and its `@hw` tags; the short pulses of the reports of `hdl/find-runts.sh`; the
facts of `facts.toml` and their `@fact` tags across the repository
(through ceres-knowledge); the verdicts of `races.toml`;
the rules of `rules.toml` with their `@rule` tags in `hdl/rtl/`; and what the
proofs assume (below). The checks and the queries on them are in Datalog,
`knowledge.dl`, which also says what each table holds;
[Soufflé](https://souffle-lang.github.io) (2.5 is tested) runs it.

```sh
./knowledge.sh list            # the queries and the checks
./knowledge.sh width-facts     # print a query
./knowledge.sh check           # run the checks
./knowledge.sh run mine.dl     # your own rules, with the program's
```

A check is a relation that must be empty: a delay or a short pulse at a
control without a verdict, a verdict naming what `hdl/rtl/` does not have, a
fact on a width race that no race names, a fact that needs a rule and has
none, an applied rule without its tag, two rules explaining the same facts,
and the proofs' assumptions below. A query is a relation printed as a
table: a first column named `rank` orders its rows, and is not printed. A
file for `run` adds rules to the program's relations and outputs what it
wants to see, as in

```prolog
// The facts that no test shows and no open question asks about.
.decl unasked(fact: symbol)
unasked(f) :- fact_id(f), !fact_test(f, _), !fact_open(f, _).
.output unasked(IO=stdout)
```

The knowledge base was an SQLite database until 2026-10-29. On the same
tables, and on a dozen copies broken so that each check finds something,
the Datalog checks found the same rows as the SQL ones; the queries give
the same rows too (with lists in their cells sorted by name).

## Races and facts, through the signals

`hdl/oracle/rtl.awk` also reads `hdl/rtl/` as a graph of signals: an edge from
each signal an assignment reads (in its expression, or in the conditions
of the ifs and cases around it) to the one it assigns, and through the
port maps, into an instance's inputs and out of its outputs. The CPU's bus
in the chips' tops and the CPU core are left out, since through them (and
through software) every signal reaches every other. A race sits at its
signals and delays, a fact at the signals the statements after its `@fact`
tags assign, and after the `@rule` tags of the rules that explain it. The
knowledge base follows the graph recursively, and `knowledge.sh check`
finds a race and a fact that rest on each other, as `races.toml` says, that
no path connects either way.

That is coarse inside the PPU, a tight cluster where most of a race's signals
reach most facts, and sharp across blocks: a link from the timer's race to a
STAT fact is caught. `./knowledge.sh race-links` gives each
link and its shortest path (the links are 0 to 2 edges long, most other pairs 3
to 40), and `./knowledge.sh race-fact-neighbours` the width
races and facts within an edge of each other that no link joins, to look at.

## What the proofs assume

ceres-core's proofs against the netlist each take some things as given: a
transcription of a block of `hdl/rtl/` (checked by `netlist_replay`), a
mapping of ceres's time onto the netlist's (measured on traces), a
difference the facts record, or a case left out. Each is an entry of
[assumptions.toml](assumptions.toml), with its evidence:
proved, replayed, measured, fact or open. A harness names what it rests on
with `@assumes` tags before its `fn`, and what it proves for the others
with `@discharges` tags. The knowledge base follows them recursively:
`netlist_scan_dma` assumes `dma-runs-is-netlist-run`, which
`netlist_dma_bisimulation` discharges, so the scan's proof also rests on
what the DMA's assumes, such as the open `dma-one-run-per-m-cycle`.

```sh
./knowledge.sh assumptions        # what each proof rests on, and the evidence
./knowledge.sh open-assumptions   # the open ones, and the proofs resting on each
```

`knowledge.sh check` finds the tags and fields that name nothing, a
harness against the netlist with no tag, a proved assumption no harness
proves (or one that is not proved and is discharged), an entry nothing
uses, and a proof that rests, through the others, on what it proves. So
what is not proved is written down: open, measured or a fact.

## The rules beyond the die

[rules.toml](rules.toml) lists what the model needs beyond the die's cells
and wires to do what the hardware does, aiming at the fewest rules that
explain every behaviour a hardware test shows. The die's connectivity is
trusted and its delays are not, so an order race needs no rule; the
outcome of a width race does, and so does what the netlist cannot tell (a
RAM's cells, the power-on state, the electrical side) or gets wrong. A rule
is applied (`hdl/rtl/` has it, tagged `@rule <id>`), proposed (ceres does it
and passes the tests, `hdl/rtl/` not yet) or missing (nothing known passes).
A rule is shown needed the way a race's verdict is: the model without it
fails a hardware test. `./knowledge.sh rules` lists them.

A rule names the models it is stated for, as the facts do (the CGB's cover its
revisions). The CGB follows the DMG where nothing else is known, and the
knowledge base derives it, recursively: a rule holds on the models it names, on
their revisions, and on a model that follows one it holds on, unless its
`not_on` names that model; the evidence then says which CGB test or which rule
of the CGB's says otherwise. `./knowledge.sh followed-rules` lists the rules
that hold on a chip only so. `hdl/rtl/` must agree: a check finds an applied
rule that holds on a chip whose code does not apply it (the chips that reach the
entity of its tag through instances, outside the other chip's `REVISION`
generate), or that the code applies where `not_on` says it does not hold. A race
names the chips whose model has it: the CGB's keeps most of the DMG's code, and
with it its races. `MODELS` says which chips `knowledge.sh` looks at, by default
all of them, as the checks need: `MODELS="cgb cgb-c cgb-e" ./knowledge.sh
borrowed-rules` lists the CGB's rules that no fact about the CGB shows, the ones
it takes from the DMG, and `untested-rules` those that no hardware test shows,
which rest on documents, emulators or a guess.
