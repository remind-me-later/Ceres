import Ceres.Bisim.Compose
open Aeneas Aeneas.Std Result

/-!
# A block stepped several times

A block proved a step at a time meets blocks proved over a longer span:
`iterate` runs a step over a list of inputs, and `Steps.iterate` carries
the one-step claim over to the run. The OAM scan, proved a sprite's window
at a time (`Scan.lean`), runs over a line's windows so; in the chip's
outline (`lean/Sketch`), mode 3's pipeline, proved a dot at a time, runs
over an M-cycle's dots.
-/

namespace ceres_core.bisim

/-- A block's step over a list of inputs, in order: the outputs, and the
last state. -/
def iterate {S I O : Type} (step : S → I → Result (O × S)) :
    S → List I → Result (List O × S)
  | s, [] => ok ([], s)
  | s, i :: is => do
      let (o, s') ← step s i
      let (os, s'') ← iterate step s' is
      ok (o :: os, s'')

/-- A run's known differences: one of its steps is one, the two sides
stepped to it. -/
def iterateK {S T I J O P : Type} (step : S → I → Result (O × S))
    (cstep : T → J → Result (P × T)) (K : S → T → I → J → Prop) :
    S → T → List I → List J → Prop
  | s, t, i :: is, j :: js => K s t i j ∨
      ∀ o s' p t', step s i = ok (o, s') → cstep t j = ok (p, t') →
        iterateK step cstep K s' t' is js
  | _, _, _, _ => False

/-- If a step agrees from the states `R` relates, so does a run of steps
over inputs related one by one. -/
theorem Steps.iterate {Sn Sc In Ic On Oc : Type}
    {net : Sn → In → Result (On × Sn)} {cer : Sc → Ic → Result (Oc × Sc)}
    {RI : In → Ic → Prop} {RO : On → Oc → Prop} {K : Sn → Sc → In → Ic → Prop}
    {R : Sn → Sc → Prop} (h : Steps net cer RI RO K R) :
    Steps (iterate net) (iterate cer) (List.Forall₂ RI) (List.Forall₂ RO)
      (iterateK net cer K) R := by
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
      intro o2 s2 p2 t2 h2 hc2
      simp only [hn, Result.ok.injEq, Prod.mk.injEq] at h2
      simp only [hc, Result.ok.injEq, Prod.mk.injEq] at hc2
      obtain ⟨-, rfl⟩ := h2
      obtain ⟨-, rfl⟩ := hc2
      exact hK
    · refine Or.inr ⟨o :: os, n'', p :: ps, c'', ?_, ?_, List.Forall₂.cons ho hos, hr2⟩
      · unfold ceres_core.bisim.iterate
        simp [hn, hn2]
      · unfold ceres_core.bisim.iterate
        simp [hc, hc2]

end ceres_core.bisim
