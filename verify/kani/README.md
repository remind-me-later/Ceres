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
a time (2), under a memory cap where systemd-run is there (`KANI_MEMORY`,
9G), with no concrete playback: playback multiplies a harness's memory, and
only a failing harness needs it, for its counterexample. Mode 3's 22
harnesses take about 90 minutes two at a time; the other 27, six.

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
