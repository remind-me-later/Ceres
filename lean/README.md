# Lean proofs

Proofs in [Lean](https://lean-lang.org) that ceres's blocks are bisimilar
to the netlist's, built on ceres-core's Kani proofs. Kani proves one step of
a block, from every pair of states; Lean proves by coinduction that the
step holds forever, and wires blocks together.

## What is proved

Each block's step is one Rust function in ceres-core, `step_agrees`
(`ceres-core/src/timing/relation.rs`, `ceres-core/src/interrupts/relation.rs`,
`ceres-core/src/memory/dma/relation.rs`,
`ceres-core/src/ppu/display/timing/relation.rs`): from a pair of states that
`related` matches (ceres's and the netlist's transcription, `ceres-netlist`), an
M-cycle with any input gives the same outputs on both sides and leaves the pair
related, unless it is a known difference (a fact of
`verify/knowledge/facts.toml`). The netlist's M-cycles are cut at the start of
E, where ceres's begins (the line timing's at the D edge before it, `h = 0` mod
8 of ceres's half dots). Kani proves each step true for every argument, every
field of both states free (`netlist_timer_bisimulation`,
`netlist_if_bisimulation`, `netlist_dma_bisimulation`, `netlist_lcd_access`),
and also `timer_if::relation::in_sequence` (`ceres-core/src/timer_if/relation.rs`,
`ceres_timer_and_if_in_sequence`): ceres's M-cycle of the two blocks, run
T-cycle by T-cycle as ceres runs them, is the timer's step and then IF's. Aeneas
translates the same functions to Lean (`Ceres/Gen`), where those are the
hypotheses (`KaniTimer`, `KaniIf`, `KaniDma`, `KaniLcd`, `KaniSeq`).

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
  on, with ceres's clock ahead until the pixel pipeline resyncs it, and
  the HBlank source, with where mode 3 ends as an input (the pixel
  pipeline's, the same on both sides); it leaves out turning the LCD on
  with LYC 0, its enable on and the flag clear (Kani's `netlist_lcd_on`
  pins that case down).
- `Ceres/Bisim/Scan.lean`: the OAM scan, cut into one sprite's window
  (`ceres-core/src/ppu/display/objects/relation.rs`): Kani proves the window
  from every related pair (`netlist_scan_step`, `KaniScanStep`), the
  scan's start from the line's end (`netlist_scan_lead_in`) and its end
  (`netlist_scan_tail`), each in seconds; Lean carries the window over
  windows 2 to 38 (`scan_loop_related`, by induction, with Aeneas's `step`
  tactic) and puts the three together: a whole scan of a visible line,
  with any Ys, leaves ceres's first ten sprites in the netlist's slots, and
  the scan stopped (`scan_agrees`). The window is also a block in the
  framework's form (`scan_steps`, the netlist's window and ceres's read
  apart, a window past 38 a known difference), and `Steps.iterate` runs it
  over windows (`scan_windows`).
- `Ceres/Bisim/Iterate.lean`: `Steps.iterate`, a block that steps one input
  at a time steps over a list of them (the scan over a line's windows; in
  the outline, mode 3's pipeline over an M-cycle's dots).
- `Ceres/Bisim/Close.lean`: `Steps.close`, a block closed with a driver that
  gives its input from its own state and takes its output back (the CPU and
  the blocks on its bus, the loop cut at the CPU's state).
- `Ceres/Bisim/Feedback.lean`: `Steps.feedback`, a block whose output comes from
  its state alone (a Moore output) fed to a block before it, which feeds it
  back (the PPU's VRAM and OAM locks for the memory, whose bytes the PPU's
  dots then read); `Steps.phases`, a block in phases, each proved to step
  into one of them (the PPU's line: scan, mode 3, HBlank, VBlank).
- `Ceres/Bisim/Mode3.lean`: mode 3, a dot at a time
  (`ceres-core/src/ppu/display/background/relation`): Kani's dot harnesses
  together (`KaniMode3Dot`) give `mode3_dot_steps`, the dot as a block, the
  netlist's two edges against ceres's dot, from related states into related
  ones or the line's end (ceres drawn, the netlist's mode 3 stopped at the
  dot's second edge); `netlist_mode3_start` (`KaniMode3Start`) gives
  `mode3_start`, the loop's first dot into related states. The harnesses are
  split by ceres's sleep and the window; `netlist_mode3_dot_split` checks
  that together they take every related pair and every input the guard lets
  through. One thing is taken on trust: the guard (quiet inputs, WX held, the
  lead-in matched) reads ceres's state, so the block's known difference is
  the guard failing for some ceres state related to the netlist's.
- `Ceres/Bisim/Line.lean`: a line's start, the OAM scan before mode 3. The
  scan is the proved one (`scan_agrees`), and mode 3's start a hypothesis
  (`hStart`: from the scan's end, ceres's first ten sprites in the
  netlist's slots, mode 3's states are related; Kani's
  `netlist_mode3_start` for the pipeline): `scan_then_mode3`. The
  outline's `chip_agrees` does not route a line's dots between the two
  blocks yet: its `hM3` still stands for the scan's dots too.
- `Ceres/Axioms.lean`: the build fails if a theorem comes to rest on more
  than Lean's own axioms (`sorry`, or the compiler's through
  `native_decide`).

## The chip in outline

`Sketch/` is the whole chip's proof in outline, kept apart from `Ceres/`
(it is not a default target): how the blocks would compose, with each
block's claim as a hypothesis rather than a proof. Nothing in it names the
emulator's types yet.

- `Sketch/Chip.lean`: mode 3's dots wired in front of the M-cycle blocks, the
  CPU closed around both (`Steps.close`), and `chip_agrees`: the two closed
  machines agree from every related pair of states, until a run leaves the
  blocks' scope. Its hypotheses are what is open: mode 3's step (`hM3`, Kani's
  within its scope), the M-cycle blocks' (`hPer`, `blocks_steps` once it is
  plugged in), how an M-cycle's access looks to mode 3 dot by dot (`hDots`), and
  the SM83's two (`hOut`, `hNext`).
- `Sketch/Instance.lean`: the outline's M-cycle blocks are the real ones.
  `hPer_real` is `hPer` for `netBlocks` and `ceresBlocks`, from
  `blocks_steps` and Kani's five claims, with mode 3's flags put into the
  real input's `mode3End` (`endOf`). It shows what the outline still takes
  for granted of the clock: an M-cycle has four dots, a dot is the edges
  `2d` and `2d + 1` and mode 3 ends at the second (the real input names a
  half dot, and an end at an even one is a known difference there), and
  mode 3 ends at most once in an M-cycle (the real input holds one).
- `Sketch/Dmg.lean`: the whole DMG, speculatively: the CPU closed around
  every block its bus or the LCD reaches (the PPU's line a dot at a time in
  phases, around the memory and its locks; the M-cycle blocks; HRAM's side,
  the joypad and serial port, the APU), proved from one hypothesis a block,
  `dmg_agrees`. Its closing section lists each hypothesis with what stands
  for it today: a proof to plug in, Kani's not yet translated, or nothing.
- `Sketch/DmgReal.lean`: `dmg_agrees_real`, the same with the real M-cycle
  blocks (`hPer_real`), on Kani's five claims.
- `Sketch/DmgMode3.lean`: `dmg_agrees_mode3`, the same with the line's
  mode 3 phase taken from Kani's dot harnesses (`mode3_phase`, on
  `mode3_dot_steps` through `StepsInto.embed`): what stands for it is how
  the line holds mode 3's blocks (its dot is the pipeline's and the loop's
  on its state taken apart and put back, the rest left alone), that
  related dot inputs make the same mode 3 input, that the line's end is in
  HBlank's relation, and that its known difference takes mode 3's.
- `Sketch/Axioms.lean`: the outline rests on its hypotheses and Lean's own
  axioms, with no `sorry`.

`lake build Sketch` builds it.

## The timer without Kani (a pilot)

`Pilot/` proves `KaniTimer` in Lean, to compare with Kani, which takes 2 s
for it (`netlist_timer_bisimulation`). It is not a default target, and not
under `Ceres/Axioms.lean`: its proofs use `bv_decide`, whose certificates
are checked by compiled code (one native axiom per call).

- `Pilot/Timer/Models.lean`: the timer's two sides written by hand in plain
  `BitVec` and `Bool`, the netlist's `NetTimer` and ceres's `Clock` on the
  DMG, and the check's `related`, `known_difference` and `step_agrees`.
- `Pilot/Timer/Step.lean`: the step on the hand models, by `bv_decide`, one
  goal per shape of the access.
- `Pilot/Timer/NetLink.lean`: the hand `Net` is Aeneas's translation of
  `NetTimer`, function by function (`clock_of`, `settle`, `edge_a`,
  `phase`, `read`, `m_cycle`).
- `Pilot/Timer/CeresLink.lean`: the hand `Cer` is the translation of
  `ceres_m_cycle`, on the clocks the step relates (no DIV-reset delay, not
  stopped, no APU delay, a reload countdown of at most 8): `Clock`'s
  functions, `run`'s loop over one T-cycle, the check host's hooks.
- `Pilot/Timer/Kani.lean`: `kani_timer : timer_if.KaniTimer`, from the
  links and the hand step.

`lake build Pilot` builds it, from clean in 113 s at 0.9 GB: 5 s for the
step, 43 s and 65 s for the two links. What the pilot found:

- The step itself is as fast in Lean as in Kani, once the models are
  written for `bv_decide`: every `bif` picks a value, never a whole state
  (a field-by-field `pick` does that). Written the obvious way, the build
  had not finished after an hour: unfolding one of its nine goals alone
  ran past ten minutes.
- The links are what costs, in the writing more than the checking. Each
  needs the translation taken apart by hand: checked arithmetic made
  unconditional (`if` no overflow `then ok … else fail`), comparisons and
  equalities brought to `BitVec` at numeral widths (`U8`, not
  `UScalarTy.U8.numBits`), branches between whole values pushed down to
  the fields, `Result`'s binds and Aeneas's `uncurry` rewritten (neither
  reduces by `rfl`). Aeneas's own simp set rewrites widths in a way that
  once built an ill-typed term, so the links use `simp only` with lists of
  their own.
- Kani needs none of that, and checks the Rust itself rather than a
  translation of it. Lean's proof composes: `KaniTimer` stops being a
  hypothesis of `timer_and_if_agree`.

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
tools. After a change to the `relation` modules, or to what they call
(ceres-core's timer, IF, OAM DMA, line timing, OAM scan and mode 3 loop,
ceres-netlist's transcriptions of them), run

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
A comparison as the last expression of an `if`'s arm, or alone in a
tuple, is a proposition too: mode 3's relation wraps it in `bit`
(`relation::pipeline`), whose argument Lean takes as a bool. Nor does Aeneas
take a `return` inside a loop (mode 3's loop runs while `running`),
`i16::from` of a byte (`wide`), `trailing_zeros`, a range's `contains`,
`mem::take`, or a struct holding a `&mut` whose trait methods change it
(mode 3's hooks own a copy of the fetcher).

To move to a newer Aeneas: change the `rev` in `lakefile.toml`, run
`lake update aeneas`, build Charon at that Aeneas's `charon-pin` and the
Aeneas itself, rerun `translate.sh`, and update the versions here and in
`translate.sh`.
