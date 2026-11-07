# Lean proofs

Proofs in [Lean](https://lean-lang.org) that ceres's blocks are bisimilar
to the netlist's, built on ceres-core's Kani proofs. Kani proves one step of
a block, from every pair of states; Lean proves by coinduction that the
step holds forever, and wires blocks together.

## What is proved

Each block's step is one Rust function in ceres-core, `step_agrees`
(`ceres-core/src/timing/check.rs`, `ceres-core/src/interrupts/check.rs`):
from a pair of states that `related` matches (ceres's and the netlist's
transcription, `ceres-netlist`), an M-cycle with any input gives the same
outputs on both sides and leaves the pair related, unless it is a known
difference (a fact of `doc/facts.toml`). Kani proves it true for every
argument, every field of both states free (`netlist_timer_bisimulation`,
`netlist_if_bisimulation`); Aeneas translates the same function to Lean
(`Ceres/Gen`), where that is the hypothesis (`KaniTimer`, `KaniIf`).

- `Ceres/Bisim/Compose.lean`: `Agree`, the greatest relation such that,
  for related inputs, both steps succeed with related outputs and states
  that agree again (a coinductive predicate), and `agree_wire`: if block A
  agrees, and block B agrees on the inputs A's related outputs give it, A
  wired into B agrees.
- `Ceres/Bisim/TimerIf.lean`: the timer and IF each agree from any related
  states, and so does the pair, the timer's interrupt wired into IF
  (`timer_and_if_agree`).
- `Ceres/Axioms.lean`: the build fails if a theorem comes to rest on more
  than Lean's own axioms (`sorry`, or the compiler's through
  `native_decide`).

What the theorems rest on is in `doc/assumptions.toml`, as for the Kani
harnesses: each theorem names its assumptions with `@assumes` tags, and
`hdl/knowledge.sh assumptions` follows them through the harnesses
(`hdl/knowledge.sh open-assumptions` lists the open ones). Beyond those,
the proofs trust Lean, Kani, and the translation by Charon and Aeneas.

## Building

With [elan](https://github.com/leanprover/elan) (which installs the Lean of
`lean-toolchain`):

```sh
cd lean
lake exe cache get   # Mathlib's built files, instead of building it
lake build
```

Lake fetches Aeneas's Lean library at the nightly that `lakefile.toml`
names, and downloads its built files when that release has them.

## Translating the Rust

`Ceres/Gen` is generated, and committed: building the proofs needs no Rust
tools. After a change to the `check` modules, or to what they call
(ceres-core's timer and IF, ceres-netlist's `timer` and `interrupts`), run

```sh
CHARON=.../charon AENEAS=.../aeneas lean/translate.sh
```

and `lean/translate.sh check` to see that `Ceres/Gen` is the translation
of the sources as they are. It needs [Charon](https://github.com/AeneasVerif/charon)
at commit `8cae8fd1` and [Aeneas](https://github.com/AeneasVerif/aeneas) at
`nightly-2026.11.07-c5eb4a2` (the Aeneas whose Lean library the lakefile
requires; `charon-pin` in Aeneas names the Charon commit), each built as
its README says (Charon's `crates/minirust` is a submodule; Aeneas's opam
switch needs `ppx_deriving_hash`). Charon builds ceres-core with its own
nightly Rust, through rustup, in `target/charon`. The translation must
leave nothing as an axiom: the script fails if it does.

The Rust that Aeneas takes is written as ceres-netlist's is (its crate
documentation): booleans combine with `&` and `|`, loops are `while` over
indices, `Option`s are taken apart with `match`; a loop that carries an
`Option` is written out, as Aeneas finds no fixed point for it.

To move to a newer Aeneas: change the `rev` in `lakefile.toml`, run
`lake update aeneas`, build Charon at that Aeneas's `charon-pin` and the
Aeneas itself, rerun `translate.sh`, and update the versions here and in
`translate.sh`.
