import Ceres.Bisim.Iterate
import Ceres.Gen.Funs
open Aeneas Aeneas.Std Result ceres_core.bisim

/-!
# The OAM scan, window by window

Kani proves the scan's three pieces for every argument
(`ceres-core/src/ppu/display/objects/relation.rs`, `bisim.rs`):

- `KaniScanLeadIn` (`netlist_scan_lead_in`): from the line's end, the
  scan's first windows leave the netlist's side and ceres's `related` at
  window 2;
- `KaniScanStep` (`netlist_scan_step`): from a pair related at a window `j`
  (2 to 38), the netlist's window and ceres's read of sprite `j + 1` leave
  it related at `j + 1`;
- `KaniScanTail` (`netlist_scan_tail`): from a pair related at window 39,
  the netlist's scan stops with ceres's first ten sprites in its slots.

`scan_agrees` carries the step over windows 2 to 38 (`scan_loop`, by
induction) and puts the three together: a whole scan of a visible line,
with any Ys, leaves ceres's first ten sprites in the netlist's slots.
-/

namespace ceres_core.scan

open ceres_core.ppu.display.objects.relation

/-- A spec's value, as an equation. -/
theorem spec_ok_exists {α} {m : Result α} {p : α → Prop} (h : WP.spec m p) :
    ∃ x, m = ok x ∧ p x := by
  cases m with
  | ret x => exact ⟨x, rfl, (WP.spec_ok x).1 h⟩
  | vis e k => exact absurd h (by simp [WP.spec_vis])
  | div => exact absurd h (by simp [WP.spec_div])

/-- What Kani proves of a window. -/
def KaniScanStep : Prop :=
  ∀ net ceres line j y, step_agrees net ceres line j y = ok true

/-- What Kani proves of the scan's start. -/
def KaniScanLeadIn : Prop :=
  ∀ count ns held line (ys : LeadYs), lead_in_agrees count ns held line ys = ok true

/-- What Kani proves of the scan's end. -/
def KaniScanTail : Prop :=
  ∀ net ceres line, tail_agrees net ceres line = ok true

/-- A window, from a related pair: the step succeeds, and the pair is
related at the next window. -/
-- @assumes scan-step-agrees
theorem window (kani : KaniScanStep) {net ceres line j y}
    (hr : related net ceres line j = ok true) (h2 : 2 ≤ j.val) (h38 : j.val ≤ 38) :
    step net ceres line j y ⦃ (net', ceres') =>
      ∀ j' : U8, j'.val = j.val + 1 → related net' ceres' line j' = ok true ⦄ := by
  have k := kani net ceres line j y
  unfold step_agrees at k
  rw [hr] at k
  have hc : (decide (j ≥ 2#u8) && decide (j ≤ 38#u8) && true) = true := by
    simp only [Bool.and_true, Bool.and_eq_true, decide_eq_true_eq]
    exact ⟨by scalar_tac, by scalar_tac⟩
  simp only [bind_ok, hc, ↓reduceIte] at k
  obtain ⟨⟨net', ceres'⟩, hs, k⟩ := bind_eq_ok k
  obtain ⟨j0, hj, k⟩ := bind_eq_ok k
  rw [hs, WP.spec_ok]
  intro j' hj'
  have h0 : j0.val = j.val + 1 := by
    have := U8.add_spec (x := j) (y := 1#u8) (by scalar_tac)
    rw [hj, WP.spec_ok] at this
    simpa using this
  have : j' = j0 := by scalar_tac
  subst this
  exact k

/-- One turn of `scan`'s loop, by `loop`'s equation. -/
theorem scan_loop_step line ys net ceres (j : U8) :
    scan_loop line ys net ceres j =
      Std.bind (scan_loop.body line ys net ceres j) fun r =>
        match r with
        | .cont (n, c, j) => scan_loop line ys n c j
        | .done y => ok y := by
  unfold scan_loop
  rw [loop]
  congr 1
  funext r
  rcases r with ⟨n, c, j⟩ | y <;> rfl

/-- The loop, from a pair related at any window `j` from 2 to 39: it ends
with the pair related at window 39. -/
theorem scan_loop_related (kani : KaniScanStep) line ys net ceres (j : U8)
    (h2 : 2 ≤ j.val) (h39 : j.val ≤ 39) (hr : related net ceres line j = ok true) :
    scan_loop line ys net ceres j ⦃ (net', ceres') =>
      related net' ceres' line 39#u8 = ok true ⦄ := by
  rw [scan_loop_step]
  unfold scan_loop.body
  by_cases hj : j ≤ 38#u8
  · simp only [hj, ↓reduceIte]
    step as ⟨i, hi⟩
    step as ⟨i1, hi1⟩
    step as ⟨y, hy⟩
    step with window kani hr h2 (by scalar_tac) as ⟨p, hr'⟩
    rcases p with ⟨net', ceres'⟩
    step as ⟨j', hj'⟩
    exact scan_loop_related kani line ys net' ceres' j' (by scalar_tac) (by scalar_tac)
      (hr' j' hj')
  · simp only [hj, ↓reduceIte, bind_ok, WP.spec_ok]
    have : j = 39#u8 := by scalar_tac
    subst this
    exact hr
termination_by 39 - j.val
decreasing_by scalar_tac

/-- The scan's start, from the line's end: the pair is related at window 2. -/
-- @assumes scan-lead-in-agrees
theorem lead_in_related (kani : KaniScanLeadIn) {count ns held line ys net0 ceres0}
    (hc : count ≤ ceres_netlist.oam_scan.SLOTS) (hl : line.ly < 144#u8)
    (hn : net_at_line_end count ns held = ok net0) (hce : ceres_at_line_end held = ok ceres0) :
    lead_in net0 ceres0 line ys ⦃ (net, ceres) => related net ceres line 2#u8 = ok true ⦄ := by
  have k := kani count ns held line ys
  unfold lead_in_agrees at k
  have hcond : (decide (count ≤ ceres_netlist.oam_scan.SLOTS) && decide (line.ly < 144#u8)) = true := by
    simp only [Bool.and_eq_true, decide_eq_true_eq]
    exact ⟨hc, hl⟩
  simp only [hcond, ↓reduceIte, hn, bind_ok, hce] at k
  obtain ⟨⟨net, ceres⟩, hp, k⟩ := bind_eq_ok k
  rw [hp, WP.spec_ok]
  exact k

/-- The scan's end, from a pair related at window 39: the netlist's scan
stops, with ceres's first ten sprites in its slots. -/
-- @assumes scan-tail-agrees
theorem tail_saves (kani : KaniScanTail) {net ceres line}
    (hr : related net ceres line 39#u8 = ok true) :
    tail net line ceres.latches.y ⦃ net' =>
      slots_are net' ceres ceres.found = ok true ∧ ¬ net'.scan.scanning ⦄ := by
  have k := kani net ceres line
  unfold tail_agrees at k
  simp only [hr, bind_ok, ↓reduceIte] at k
  obtain ⟨net', ht, k⟩ := bind_eq_ok k
  obtain ⟨b, hb, k⟩ := bind_eq_ok k
  rw [ht, WP.spec_ok]
  simp only [Result.ok.injEq, Bool.and_eq_true, decide_eq_true_eq] at k
  refine ⟨?_, ?_⟩
  · rw [hb]
    simp only [k]
  · simp [k]

/-- A whole scan of a visible line, with any Ys, from the line before's
scan ended with any count: the netlist's slots hold ceres's first ten
sprites on the line, in index order, and its scan has stopped. -/
theorem scan_agrees (kStep : KaniScanStep) (kLead : KaniScanLeadIn) (kTail : KaniScanTail)
    (count : U8) (ns : Bool) (held : U8) (line : Line) (ys : Std.Array U8 40#usize)
    (hc : count ≤ ceres_netlist.oam_scan.SLOTS) (hl : line.ly < 144#u8) :
    scan count ns held line ys ⦃ (net, ceres) =>
      slots_are net ceres ceres.found = ok true ∧ ¬ net.scan.scanning ⦄ := by
  unfold scan
  obtain ⟨net0, hn⟩ : ∃ n, net_at_line_end count ns held = ok n := ⟨_, rfl⟩
  obtain ⟨ceres0, hce⟩ : ∃ c, ceres_at_line_end held = ok c := ⟨_, rfl⟩
  rw [hn, bind_ok, hce, bind_ok]
  have hs : lead_ys ys ⦃ _ => True ⦄ := by
    unfold lead_ys
    step*
  obtain ⟨ly, hly, -⟩ := spec_ok_exists hs
  rw [hly, bind_ok]
  step with lead_in_related kLead hc hl (by rw [hn]) (by rw [hce]) as ⟨net1, ceres1, hp⟩
  step with scan_loop_related kStep line ys net1 ceres1 2#u8 (by scalar_tac) (by scalar_tac) hp
    as ⟨net2, ceres2, hq⟩
  step with tail_saves kTail hq as ⟨net3, h3, h4⟩
  exact ⟨h3, h4⟩

/-! ## The scan as a block

The same window as the framework's blocks are stated (`Steps`,
`Compose.lean`): the netlist's side and ceres's step apart, the netlist's
carrying the window's index, ceres's its read, each window's input the Y
OAM gives (the same on both sides), no output. The relation is `related`
at the netlist's window, from window 2 on; a window past 38 is outside
Kani's step (the tail's), a known difference. `Steps.iterate` carries it
over the windows. -/

/-- The netlist's window, with its index. -/
def scanNet (line : Line) (s : NetSide × U8) (y : U8) : Result (Unit × (NetSide × U8)) := do
  let n ← net_window s.1 line s.2 y
  let j ← s.2 + 1#u8
  ok ((), (n, j))

/-- Ceres's read of the window's sprite. -/
def scanCer (line : Line) (c : CeresSide) (y : U8) : Result (Unit × CeresSide) := do
  let c' ← ceres_read c line y
  ok ((), c')

/-- The pair is related at the netlist's window, from window 2 on. -/
def ScanRelated (line : Line) (s : NetSide × U8) (c : CeresSide) : Prop :=
  related s.1 c line s.2 = ok true ∧ 2 ≤ s.2.val

/-- Past window 38: the tail's, not the step's. -/
def ScanK (s : NetSide × U8) (_ : CeresSide) (_ _ : U8) : Prop := 38 < s.2.val

/-- The scan's window, as a block. -/
-- @assumes scan-step-agrees
theorem scan_steps (kani : KaniScanStep) (line : Line) :
    Steps (scanNet line) (scanCer line) (fun y y' => y = y') (fun _ _ => True) ScanK
      (ScanRelated line) := by
  rintro ⟨net, j⟩ ceres ⟨hr, h2⟩ y y' rfl
  by_cases h38 : 38 < j.val
  · exact Or.inl h38
  right
  obtain ⟨⟨net', ceres'⟩, hs, hpost⟩ := spec_ok_exists (window kani (y := y) hr h2 (by simp only at h38 h2 ⊢; omega))
  unfold step at hs
  obtain ⟨n1, hn1, hs⟩ := bind_eq_ok hs
  obtain ⟨c1, hc1, hs⟩ := bind_eq_ok hs
  simp only [Result.ok.injEq, Prod.mk.injEq] at hs
  obtain ⟨rfl, rfl⟩ := hs
  obtain ⟨j', hj', hjv⟩ := spec_ok_exists (U8.add_spec (x := j) (y := 1#u8) (by scalar_tac))
  refine ⟨(), (n1, j'), (), c1, ?_, ?_, trivial, ?_, ?_⟩
  · simp only [scanNet, hn1, hj', bind_ok]
  · simp only [scanCer, hc1, bind_ok]
  · exact hpost j' (by simpa using hjv)
  · simp only at hjv ⊢
    scalar_tac

/-- The scan over a run of windows: from a pair related at a window, a
run's windows agree, or one of them is past 38. -/
theorem scan_windows (kani : KaniScanStep) (line : Line) :
    Steps (iterate (scanNet line)) (iterate (scanCer line))
      (List.Forall₂ fun y y' => y = y') (List.Forall₂ fun _ _ => True)
      (iterateK (scanNet line) (scanCer line) ScanK) (ScanRelated line) :=
  (scan_steps kani line).iterate

end ceres_core.scan
