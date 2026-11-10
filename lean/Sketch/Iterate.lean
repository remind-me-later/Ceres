import Ceres.Bisim.Compose
open Aeneas Aeneas.Std Result

/-!
# A block stepped several times

A block proved a dot at a time (mode 3's pipeline) meets blocks proved an
M-cycle at a time: `iterate` runs a step over a list of inputs, and
`Steps.iterate` carries the one-step claim over to the run.
-/

namespace ceres_core.sketch
open ceres_core.bisim

/-- A block's step over a list of inputs, in order: the outputs, and the
last state. -/
def iterate {S I O : Type} (step : S → I → Result (O × S)) :
    S → List I → Result (List O × S)
  | s, [] => ok ([], s)
  | s, i :: is => do
      let (o, s') ← step s i
      let (os, s'') ← iterate step s' is
      ok (o :: os, s'')

/-- A run's known differences: one of its steps is one. -/
def iterateK {S I O : Type} (step : S → I → Result (O × S)) (K : S → I → Prop) :
    S → List I → Prop
  | _, [] => False
  | s, i :: is => K s i ∨ ∀ o s', step s i = ok (o, s') → iterateK step K s' is

/-- If a step agrees from the states `R` relates, so does a run of steps
over inputs related one by one. -/
theorem Steps.iterate {Sn Sc In Ic On Oc : Type}
    {net : Sn → In → Result (On × Sn)} {cer : Sc → Ic → Result (Oc × Sc)}
    {RI : In → Ic → Prop} {RO : On → Oc → Prop} {K : Sn → In → Prop}
    {R : Sn → Sc → Prop} (h : Steps net cer RI RO K R) :
    Steps (iterate net) (iterate cer) (List.Forall₂ RI) (List.Forall₂ RO)
      (iterateK net K) R := by
  intro n c hr is js hij
  induction hij generalizing n c with
  | nil =>
    refine Or.inr ⟨[], n, [], c, ?_, ?_, List.Forall₂.nil, hr⟩
    · rfl
    · rfl
  | @cons i j is js hab hl ih =>
    rcases h n c hr i j hab with hk | ⟨o, n', p, c', hn, hc, ho, hr'⟩
    · exact Or.inl (Or.inl hk)
    rcases ih n' c' hr' with hK | ⟨os, n'', ps, c'', hn2, hc2, hos, hr2⟩
    · refine Or.inl (Or.inr ?_)
      intro o2 s2 h2
      simp only [hn, Result.ok.injEq, Prod.mk.injEq] at h2
      obtain ⟨-, rfl⟩ := h2
      exact hK
    · refine Or.inr ⟨o :: os, n'', p :: ps, c'', ?_, ?_, List.Forall₂.cons ho hos, hr2⟩
      · unfold ceres_core.sketch.iterate
        simp [hn, hn2]
      · unfold ceres_core.sketch.iterate
        simp [hc, hc2]

end ceres_core.sketch
