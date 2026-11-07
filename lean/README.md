# Lean proofs

Proofs in [Lean](https://lean-lang.org) that ceres's blocks are bisimilar
to the netlist's, built on ceres-core's Kani proofs. Kani proves one step of
a block, from every pair of states; Lean proves by coinduction that the
step holds forever, and wires blocks together.

## What is proved

Each block's step is one Rust function in ceres-core, `step_agrees`
(`ceres-core/src/timing/check.rs`, `ceres-core/src/interrupts/check.rs`,
`ceres-core/src/memory/dma/check.rs`,
`ceres-core/src/ppu/display/timing/check.rs`):
from a pair of states that `related` matches (ceres's and the netlist's
transcription, `ceres-netlist`), an M-cycle with any input gives the same
outputs on both sides and leaves the pair related, unless it is a known
difference (a fact of `doc/facts.toml`). The netlist's M-cycles are cut
at the start of E, where ceres's begins (the line timing's at the D edge
before it, `h = 0` mod 8 of ceres's half dots). Kani proves each step true
for every argument, every field of both states free
(`netlist_timer_bisimulation`, `netlist_if_bisimulation`,
`netlist_dma_bisimulation`, `netlist_lcd_access`), and also
`check::in_sequence` (`ceres-core/src/check.rs`,
`ceres_timer_and_if_in_sequence`): ceres's M-cycle of the two blocks, run
T-cycle by T-cycle as ceres runs them, is the timer's step and then IF's.
Aeneas translates the same functions to Lean (`Ceres/Gen`), where those are
the hypotheses (`KaniTimer`, `KaniIf`, `KaniDma`, `KaniLcd`, `KaniSeq`).

- `Ceres/Bisim/Compose.lean`: `Agree`, the greatest relation such that,
  for related inputs, both steps succeed with related outputs and states
  that agree again (a coinductive predicate); `Steps`, one step from the
  states an invariant relates, as Kani proves it; `Steps.agree` (the
  coinduction), `Steps.wire` (block A's output wired into block B's input)
  and `Steps.map_cer` (ceres's side replaced by a step that does the same).
- `Ceres/Bisim/TimerIf.lean`: the timer's and IF's steps, wired together,
  carried over to ceres's interleaved M-cycle (`ceresJoint`); from related
  states, every M-cycle gives the same timer read, IF read and interrupt
  taken (`timer_and_if_agree`).
- `Ceres/Bisim/Dma.lean`: the OAM DMA's step, wired beside the two (it
  takes only its own access, `Steps.weaken_in`), ceres's side being their
  M-cycle and then the DMA's run of it (`timer_if_dma_agree`).
- `Ceres/Bisim/Lcd.lean`: the line timing's step (STAT, LY, LYC, LCDC's
  enable and the STAT line), wired in front of the timer and IF, its rises
  put in IF's input, and the DMA after them: from related states, every
  M-cycle gives the same reads of STAT, LY, LYC, the timer, IF and FF46,
  the same interrupt taken and the same OAM write (`blocks_agree`). It
  covers the LCD on, off, turned off and on, and line 0 after it is turned
  on, with ceres's clock ahead until the pixel pipeline resyncs it; it
  leaves out HBlank's source, and turning the LCD on with LYC 0, its
  enable on and the flag clear (Kani's `netlist_lcd_on` pins that case
  down).
- `Ceres/Axioms.lean`: the build fails if a theorem comes to rest on more
  than Lean's own axioms (`sorry`, or the compiler's through
  `native_decide`).

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
(ceres-core's timer, IF, OAM DMA and line timing, ceres-netlist's
transcriptions of them), run

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
`Option` is written out, as Aeneas finds no fixed point for it. Aeneas
prints a bool's `!` as Lean's `¬`, a proposition: Lean takes it back as a
bool under `&&` or `||`, but not alone in a tuple (several fields set in one
branch), so write such a value without `!`. The same goes for a lazy `||`
or `&&` whose last operand is a comparison, kept in a variable: Aeneas
makes it a branch whose last arm is the comparison, a proposition, so
write it with `|` or `&`. Aeneas has no `Option::clone`: a struct with
`Option` fields that the steps copy derives `Copy`.

To move to a newer Aeneas: change the `rev` in `lakefile.toml`, run
`lake update aeneas`, build Charon at that Aeneas's `charon-pin` and the
Aeneas itself, rerun `translate.sh`, and update the versions here and in
`translate.sh`.
