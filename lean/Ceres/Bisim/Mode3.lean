import Ceres.Bisim.Feedback
import Ceres.Gen.Funs
open Aeneas Aeneas.Std Result ceres_core.bisim

/-!
# Mode 3, a dot at a time

Kani proves ceres's mode 3 loop against the netlist's pixel pipeline a dot
at a time (`ceres-core/src/ppu/display/background/relation/mod.rs`,
`step_agrees`; the dot harnesses of `background/bisim.rs`): from every
pair `related` matches, with a ghost `g` of the objects' history, a dot
with any SCX in the mixer and any edges the guard lets through (quiet
inputs, the CPU's writes as `writes` has them, WX held, the lead-in's
match) leaves the pair related, mode 3 going on in both; or, once ceres
has drawn the line, the netlist's mode 3 stops at the dot's second edge.
`KaniMode3Dot` is that claim, which the harnesses make in parts, one per
sleep of ceres's loop and where the window is; `netlist_mode3_dot_split`
checks that the parts take every related pair, with every input the guard
lets through (each state the harnesses build free but for what `related`
pins, and the shapes it requires, `relation::shaped`).

`mode3_dot` takes the claim apart into the two sides' steps, the netlist's two
edges (`netDot`) and ceres's dot (`cerDot`), as a block's step would be.
The guard reads ceres's state as well as the netlist's (the loop's enables
and mixer SCX), where the framework's known differences (`Steps`'s `K`)
see only the netlist's: so the dot is stated with the guard as a
hypothesis, not yet as a `Steps`.
-/

namespace ceres_core.mode3

open ceres_core.ppu.display.background.relation
open ceres_netlist.render_edge

/-- What Kani's dot harnesses prove, together (`netlist_mode3_dot_split`:
they take every related pair). -/
def KaniMode3Dot : Prop :=
  ∀ l net g mixer rising falling, related l net g = ok true →
    ∃ l' net', step_agrees l net g mixer rising falling = ok (true, l', net')

/-- The inputs `step_agrees` takes a dot from (the others it passes over). -/
def guard (l : Mode3Loop) (net : NetPipeline) (mixer : U8) (rising falling : EdgeInputs) :
    Result Bool := do
  let b ← writes.quiet rising l
  let b1 ← writes.quiet falling l
  let b2 ← writes.writes net rising falling l.mixer_scx
  let b3 ← window.holds_wx net.inputs rising
  let b4 ← window.holds_wx rising falling
  let b5 ← window.lead_in_matches l mixer
  ok ((((((rising.clk && (¬ falling.clk)) && b) && b1) && b2) && ((¬
    l.window_enabled) || (b3 && b4))) && b5)

/-- The netlist's dot: its two edges, and whether mode 3 stopped after
each. -/
def netDot (net : NetPipeline) (rising falling : EdgeInputs) :
    Result ((Bool × Bool) × NetPipeline) := do
  let net1 ← NetPipeline.edge net rising
  let s1 ← NetPipeline.stop_mode3 net1
  let net2 ← NetPipeline.edge net1 falling
  let s2 ← NetPipeline.stop_mode3 net2
  ok ((s1, s2), net2)

/-- Ceres's dot, the mixer's SCX `mixer`: its loop, unless the line is
drawn (its `HBlankStart` runs then, outside the loop); and whether it was. -/
def cerDot (l : Mode3Loop) (mixer : U8) : Result (Bool × Mode3Loop) := do
  let d ← drawn l.pace
  if d then ok (true, { l with mixer_scx := mixer })
  else
    let l1 ← ceres_dot { l with mixer_scx := mixer }
    ok (false, l1)

/-- A dot from a related pair, on inputs the guard lets through: both
sides step; then either ceres had drawn the line and the netlist's mode 3
stops at the dot's second edge, not before, or neither had and the pair
is related again, with the ghost moved on. -/
-- @assumes mode3-end-background
-- @assumes mode3-window-armed
-- @assumes mode3-window-start
theorem mode3_dot (kani : KaniMode3Dot) {l net g mixer rising falling}
    (hr : related l net g = ok true) (hg : guard l net mixer rising falling = ok true) :
    ∃ s1 s2 net' d l', netDot net rising falling = ok ((s1, s2), net') ∧
      cerDot l mixer = ok (d, l') ∧
      ((d = true ∧ s1 = false ∧ s2 = true) ∨
        (d = false ∧ s1 = false ∧ s2 = false ∧ ∃ g', related l' net' g' = ok true)) := by
  obtain ⟨l', net', k⟩ := kani l net g mixer rising falling hr
  unfold step_agrees at k
  unfold guard at hg
  obtain ⟨b, hb, hg⟩ := bind_eq_ok hg
  obtain ⟨b1, hb1, hg⟩ := bind_eq_ok hg
  obtain ⟨b2, hb2, hg⟩ := bind_eq_ok hg
  obtain ⟨b3, hb3, hg⟩ := bind_eq_ok hg
  obtain ⟨b4, hb4, hg⟩ := bind_eq_ok hg
  obtain ⟨b5, hb5, hg⟩ := bind_eq_ok hg
  rw [Result.ok.injEq] at hg
  simp only [hb, hb1, hb2, hb3, hb4, hb5, bind_ok, hg, ↓reduceIte] at k
  obtain ⟨d, hd, k⟩ := bind_eq_ok k
  obtain ⟨before, hbefore, k⟩ := bind_eq_ok k
  obtain ⟨l1, hl1, k⟩ := bind_eq_ok k
  obtain ⟨net1, hnet1, k⟩ := bind_eq_ok k
  obtain ⟨s1, hs1, k⟩ := bind_eq_ok k
  obtain ⟨net2, hnet2, k⟩ := bind_eq_ok k
  cases d with
  | true =>
    simp only [↓reduceIte] at k hl1
    obtain ⟨s2, hs2, k⟩ := bind_eq_ok k
    simp only [Result.ok.injEq, Prod.mk.injEq] at k
    obtain ⟨hk, rfl, rfl⟩ := k
    rw [Result.ok.injEq] at hl1
    subst hl1
    simp only [Bool.and_eq_true, decide_eq_true_eq, Bool.not_eq_true] at hk
    refine ⟨s1, s2, net2, true, { l with mixer_scx := mixer }, ?_, ?_, Or.inl ⟨rfl, hk.1, hk.2⟩⟩
    · simp [netDot, hnet1, hs1, hnet2, hs2]
    · simp [cerDot, hd]
  | false =>
    simp only [Bool.false_eq_true, ↓reduceIte] at k hl1
    obtain ⟨s2, hs2, k⟩ := bind_eq_ok k
    obtain ⟨g1, hg1, k⟩ := bind_eq_ok k
    obtain ⟨b7, hb7, k⟩ := bind_eq_ok k
    simp only [Result.ok.injEq, Prod.mk.injEq] at k
    obtain ⟨hk, rfl, rfl⟩ := k
    simp only [Bool.and_eq_true, decide_eq_true_eq, Bool.not_eq_true] at hk
    obtain ⟨⟨h1, h2⟩, h7⟩ := hk
    subst h7
    refine ⟨s1, s2, net2, false, l1, ?_, ?_, Or.inr ⟨rfl, h1, h2, g1, hb7⟩⟩
    · simp [netDot, hnet1, hs1, hnet2, hs2]
    · simp [cerDot, hd, hl1]

/-- The guard's value, once `step_agrees` succeeds: it is the step's
first part. -/
theorem guard_ok {l net g mixer rising falling v}
    (k : step_agrees l net g mixer rising falling = ok v) :
    ∃ b, guard l net mixer rising falling = ok b := by
  unfold step_agrees at k
  obtain ⟨b, hb, k⟩ := bind_eq_ok k
  obtain ⟨b1, hb1, k⟩ := bind_eq_ok k
  obtain ⟨b2, hb2, k⟩ := bind_eq_ok k
  obtain ⟨b3, hb3, k⟩ := bind_eq_ok k
  obtain ⟨b4, hb4, k⟩ := bind_eq_ok k
  obtain ⟨b5, hb5, k⟩ := bind_eq_ok k
  exact ⟨(rising.clk && (¬ falling.clk) && b && b1 && b2 &&
    ((¬ l.window_enabled) || (b3 && b4)) && b5),
    by simp only [guard, hb, hb1, hb2, hb3, hb4, hb5, bind_ok]⟩

/-- A dot's input, the same on both sides: the mixer's SCX (ceres's), and
the dot's two edges (the netlist's). -/
abbrev DotIn := U8 × EdgeInputs × EdgeInputs

/-- The netlist's dot as a block's step. -/
def netStep (net : NetPipeline) (i : DotIn) : Result ((Bool × Bool) × NetPipeline) :=
  netDot net i.2.1 i.2.2

/-- Ceres's dot as a block's step. -/
def cerStep (l : Mode3Loop) (i : DotIn) : Result (Bool × Mode3Loop) :=
  cerDot l i.1

/-- The dot's outputs agree: ceres had drawn the line and the netlist's
mode 3 stops at the dot's second edge, or neither. -/
def DotOut (s : Bool × Bool) (d : Bool) : Prop :=
  (d = true ∧ s.1 = false ∧ s.2 = true) ∨ (d = false ∧ s.1 = false ∧ s.2 = false)

/-- The states mode 3's dots relate: some ghost relates them. -/
def Related (net : NetPipeline) (l : Mode3Loop) : Prop := ∃ g, related l net g = ok true

/-- The line drawn: ceres in `HBlankStart`, the netlist's mode 3 stopped. -/
def Ended (net : NetPipeline) (l : Mode3Loop) : Prop :=
  drawn l.pace = ok true ∧ NetPipeline.stop_mode3 net = ok true

/-- The known differences: the guard turns the input away, for a ceres
state related to the netlist's. (The guard reads ceres's state too, which
`Steps`'s known differences do not see: this is the netlist's side of it,
weaker than the guard on the pair.) -/
def DotK (net : NetPipeline) (i : DotIn) : Prop :=
  ∃ l g, related l net g = ok true ∧ guard l net i.1 i.2.1 i.2.2 = ok false

/-- Mode 3's dot as a block: from related states, a dot leaves them related
or ends the line. -/
-- @assumes mode3-end-background
-- @assumes mode3-window-armed
-- @assumes mode3-window-start
theorem mode3_dot_steps (kani : KaniMode3Dot) :
    StepsInto netStep cerStep (fun i j => i = j) DotOut DotK Related
      (fun n c => Related n c ∨ Ended n c) := by
  rintro net l ⟨g, hr⟩ i j rfl
  obtain ⟨mixer, rising, falling⟩ := i
  obtain ⟨l', net', k⟩ := kani l net g mixer rising falling hr
  obtain ⟨b, hb⟩ := guard_ok k
  cases b with
  | false => exact Or.inl ⟨l, g, hr, hb⟩
  | true =>
    right
    obtain ⟨s1, s2, net2, d, l2, hn, hc, h⟩ := mode3_dot kani hr hb
    refine ⟨(s1, s2), net2, d, l2, hn, hc, ?_, ?_⟩
    · rcases h with ⟨h1, h2, h3⟩ | ⟨h1, h2, h3, -⟩
      · exact Or.inl ⟨h1, h2, h3⟩
      · exact Or.inr ⟨h1, h2, h3⟩
    · rcases h with ⟨rfl, -, rfl⟩ | ⟨rfl, -, -, g', hr'⟩
      · right
        refine ⟨?_, ?_⟩
        · unfold cerDot at hc
          obtain ⟨d0, hd0, hc⟩ := bind_eq_ok hc
          cases d0 with
          | true =>
            simp only [↓reduceIte, Result.ok.injEq, Prod.mk.injEq] at hc
            obtain ⟨-, rfl⟩ := hc
            exact hd0
          | false =>
            simp only [Bool.false_eq_true, ↓reduceIte] at hc
            obtain ⟨l1, -, hc⟩ := bind_eq_ok hc
            simp at hc
        · unfold netDot at hn
          obtain ⟨n1, -, hn⟩ := bind_eq_ok hn
          obtain ⟨t1, -, hn⟩ := bind_eq_ok hn
          obtain ⟨n2, -, hn⟩ := bind_eq_ok hn
          obtain ⟨t2, ht2, hn⟩ := bind_eq_ok hn
          simp only [Result.ok.injEq, Prod.mk.injEq] at hn
          obtain ⟨⟨-, rfl⟩, rfl⟩ := hn
          exact ht2
      · exact Or.inl ⟨g', hr'⟩

/-- What Kani proves of the loop's first dot (`netlist_mode3_start`; its
loop as `start_before` takes it, `netlist_mode3_dot_split`). -/
def KaniMode3Start : Prop :=
  ∀ l net mixer rising falling, ∃ l' net',
    start_agrees l net mixer rising falling = ok (true, l', net')

/-- The inputs `start_agrees` takes the first dot from (the loop has the
mixer's SCX in the dot before). -/
def startGuard (l : Mode3Loop) (net : NetPipeline) (rising falling : EdgeInputs) :
    Result Bool := do
  let b ← start_before l net
  let b1 ← writes.quiet rising l
  let b2 ← writes.quiet falling l
  let b3 ← writes.writes net rising falling l.mixer_scx
  let b4 ← window.holds_wx net.inputs rising
  let b5 ← window.holds_wx rising falling
  ok ((((((b && rising.clk) && (¬ falling.clk)) && b1) && b2) && b3) &&
    ((¬ l.window_enabled) || (b4 && b5)))

/-- The loop's first dot, from the netlist's state at the end of the dot
before (`before_start`, `mapping-mode3-start`) and the scan's objects,
with the mixer's SCX `mixer`: both sides step, mode 3 going on in the
netlist, and the pair is related, with some ghost. -/
-- @assumes mapping-mode3-start
theorem mode3_start (kani : KaniMode3Start) {l net mixer rising falling}
    (hg : startGuard l net rising falling = ok true) :
    ∃ l' net', ceres_first_dot { l with mixer_scx := mixer } = ok l' ∧
      netDot net rising falling = ok ((false, false), net') ∧ Related net' l' := by
  obtain ⟨l', net', k⟩ := kani l net mixer rising falling
  unfold start_agrees at k
  unfold startGuard at hg
  obtain ⟨b, hb, hg⟩ := bind_eq_ok hg
  obtain ⟨b1, hb1, hg⟩ := bind_eq_ok hg
  obtain ⟨b2, hb2, hg⟩ := bind_eq_ok hg
  obtain ⟨b3, hb3, hg⟩ := bind_eq_ok hg
  obtain ⟨b4, hb4, hg⟩ := bind_eq_ok hg
  obtain ⟨b5, hb5, hg⟩ := bind_eq_ok hg
  rw [Result.ok.injEq] at hg
  simp only [hb, hb1, hb2, hb3, hb4, hb5, bind_ok, Bool.and_assoc] at k hg
  simp only [hg, ↓reduceIte] at k
  obtain ⟨snap, -, k⟩ := bind_eq_ok k
  obtain ⟨l1, hl1, k⟩ := bind_eq_ok k
  obtain ⟨n1, hn1, k⟩ := bind_eq_ok k
  obtain ⟨s1, hs1, k⟩ := bind_eq_ok k
  obtain ⟨n2, hn2, k⟩ := bind_eq_ok k
  obtain ⟨g0, -, k⟩ := bind_eq_ok k
  obtain ⟨s2, hs2, k⟩ := bind_eq_ok k
  obtain ⟨g1, -, k⟩ := bind_eq_ok k
  obtain ⟨b7, hb7, k⟩ := bind_eq_ok k
  simp only [Result.ok.injEq, Prod.mk.injEq] at k
  obtain ⟨hk, rfl, rfl⟩ := k
  cases s1 <;> cases s2 <;> simp at hk
  subst hk
  exact ⟨l1, n2, hl1, by simp [netDot, hn1, hs1, hn2, hs2], g1, hb7⟩

/-- Ceres's first dot of the loop as a block's step: mode 3 goes on. -/
def cerStart (l : Mode3Loop) (i : DotIn) : Result (Bool × Mode3Loop) := do
  let l1 ← ceres_first_dot { l with mixer_scx := i.1 }
  ok (false, l1)

/-- The states the first dot starts from: the scan's end as `start_before`
has it. -/
def Starting (net : NetPipeline) (l : Mode3Loop) : Prop := start_before l net = ok true

/-- The first dot's known differences: the guard turns the input away, for
a ceres state at the start with the netlist's (the netlist's side of it, as
`DotK`). -/
def StartK (net : NetPipeline) (i : DotIn) : Prop :=
  ∃ l, Starting net l ∧ startGuard l net i.2.1 i.2.2 = ok false

/-- The first dot as a block: from the scan's end, the netlist's dot (the
same as in the rest of mode 3, `netStep`) and ceres's first (`cerStart`)
leave the pair in mode 3's relation, neither side's mode 3 ending. -/
-- @assumes mapping-mode3-start
theorem mode3_start_steps (kani : KaniMode3Start) :
    StepsInto netStep cerStart (fun i j => i = j) DotOut StartK Starting Related := by
  rintro net l hs i j rfl
  obtain ⟨mixer, rising, falling⟩ := i
  obtain ⟨l', net', k⟩ := kani l net mixer rising falling
  have hg : ∃ b, startGuard l net rising falling = ok b := by
    unfold start_agrees at k
    obtain ⟨b, hb, k⟩ := bind_eq_ok k
    obtain ⟨b1, hb1, k⟩ := bind_eq_ok k
    obtain ⟨b2, hb2, k⟩ := bind_eq_ok k
    obtain ⟨b3, hb3, k⟩ := bind_eq_ok k
    obtain ⟨b4, hb4, k⟩ := bind_eq_ok k
    obtain ⟨b5, hb5, k⟩ := bind_eq_ok k
    exact ⟨_, by simp only [startGuard, hb, hb1, hb2, hb3, hb4, hb5, bind_ok]; rfl⟩
  obtain ⟨b, hb⟩ := hg
  cases b with
  | false => exact Or.inl ⟨l, hs, hb⟩
  | true =>
    obtain ⟨l2, net2, hc, hn, hr⟩ := mode3_start kani (mixer := mixer) hb
    exact Or.inr ⟨(false, false), net2, false, l2, hn, by simp [cerStart, hc],
      Or.inr ⟨rfl, rfl, rfl⟩, hr⟩

end ceres_core.mode3
