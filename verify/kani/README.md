# Running the Kani proofs

`kani.sh` runs ceres-core's Kani harnesses and records what they found. A
verdict holds for the sources it was run on, so `results.tsv` keeps,
per harness, the hash of those sources (ceres-core, ceres-netlist and the
workspace's manifests) with the verdict, the seconds and the solver.

```sh
./kani.sh status    # each harness: verified on these sources, stale or new
./kani.sh run       # run what is not verified, and record it
./kani.sh faults    # the planted faults: each must make its harness fail
```

The runs go in a second worktree, a snapshot of this one with its
uncommitted changes, which the script makes and keeps (`KANI_WORKTREE`), so
the sources here can change during a run. A run is `KANI_JOBS` harnesses at
a time (4), the longest first, under a memory cap where systemd-run is there
(`KANI_MEMORY`, 12G), without Kani's reachability checks of each assertion
(`KANI_REACH_CHECKS=1` for them: CBMC writes a trace for each one reached,
which took most of a short harness's time and memory) and with no concrete
playback: playback multiplies a harness's memory, and only a failing
harness needs it, for its counterexample. CBMC's SAT solver is Kissat tuned
for unsatisfiable formulas (`kissat-unsat`, `KANI_SOLVER` for another): the
mode 3 proofs end in one hard one, which took CBMC's own CaDiCaL up to five
times as long. The CPU's harnesses against the netlist's core run last,
`KANI_SOLO_JOBS` at a time (3). A run of all 126 took 67 minutes: the 64 but
the CPU's in 50 four at a time (mode 3's longest in 14), the CPU's 62 in 16
three at a time.

A harness passes as well when its relation holds of no state, so the
covers (`kani::cover!`) say which states it has. `covers.tsv` keeps each
harness's covers with their status, for the program it ran. A cover has to be
satisfied by a harness that has it, and a harness with covers has to
satisfy one of them, else `run` records it as vacuous. The covers sit in the
helpers the harnesses share, and each harness reaches some of them. A
cover CBMC cannot satisfy in a harness is a proof of its own there: in
mode 3's dot harnesses the covers took about a third of the time, so they
keep only "a related pair", and the cases their
pairs reach are in harnesses of their own that build the pairs without the
dot (`netlist_mode3_dot_covers`, `netlist_mode3_window_covers`). Both
`run` and `status` list any cover no harness satisfies as `uncovered`. Each
harness's full results, the covers among them, are in the worktree's
`result_output_dir/`.

`faults/` holds the planted faults: patches of ceres or of a
transcription, each naming the harness that must fail with it. They show
that a harness would notice the kind of bug it is there for; `faults`
records each as caught or missed.

Before Kani, `cargo test -p ceres-core --features verify cosim` runs the two
sides concretely and checks the proofs' relations, in seconds: ceres's
mode 3 loop and `NetPipeline` on whole lines, at every dot, and ceres's CPU
and the core (`NetCore`) on every opcode from random states, where each
instruction ends. A new rule is fitted there, and Kani then takes every
related state.
