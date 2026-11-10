import Ceres.Bisim.Compose
open Aeneas Aeneas.Std Result

/-!
# A block closed with its driver

The CPU decides an M-cycle's bus access from its own state, the blocks on
the bus step with it, and the CPU takes what they give back (the byte read,
the interrupt taken). `close` is that loop, cut at the CPU's state, and
`Steps.close` carries the blocks' one-step claim and the driver's two
claims over to the closed machine.
-/

namespace ceres_core.sketch
open ceres_core.bisim

/-- A block `b` driven by a machine with state `Sd`: the driver's state and
the outside input `x` give the block's input (`out`), the block steps, and
the driver takes the block's output (`next`). -/
def close {Sd Sb X I O : Type} (out : Sd → X → I) (next : Sd → X → O → Result Sd)
    (b : Sb → I → Result (O × Sb)) : Sd × Sb → X → Result (O × (Sd × Sb)) :=
  fun s x => do
    let (o, sb) ← b s.2 (out s.1 x)
    let sd ← next s.1 x o
    ok (o, (sd, sb))

/-- If the block steps (`hB`), related drivers give it related inputs
(`hOut`), and related drivers take related outputs to related drivers
(`hNext`), the closed machines step, with the block's known differences on
the input the driver gives. -/
theorem Steps.close {Sdn Sdc Sbn Sbc Xn Xc In Ic On Oc : Type}
    {outN : Sdn → Xn → In} {outC : Sdc → Xc → Ic}
    {nextN : Sdn → Xn → On → Result Sdn} {nextC : Sdc → Xc → Oc → Result Sdc}
    {nb : Sbn → In → Result (On × Sbn)} {cb : Sbc → Ic → Result (Oc × Sbc)}
    {RX : Xn → Xc → Prop} {RI : In → Ic → Prop} {RO : On → Oc → Prop}
    {K : Sbn → In → Prop} {RD : Sdn → Sdc → Prop} {RB : Sbn → Sbc → Prop}
    (hB : Steps nb cb RI RO K RB)
    (hOut : ∀ dn dc x y, RD dn dc → RX x y → RI (outN dn x) (outC dc y))
    (hNext : ∀ dn dc x y o p, RD dn dc → RX x y → RO o p →
      ∃ dn' dc', nextN dn x o = ok dn' ∧ nextC dc y p = ok dc' ∧ RD dn' dc') :
    Steps (close outN nextN nb) (close outC nextC cb) RX RO
      (fun s x => K s.2 (outN s.1 x)) (fun n c => RD n.1 c.1 ∧ RB n.2 c.2) := by
  rintro ⟨dn, bn⟩ ⟨dc, bc⟩ ⟨hd, hb⟩ x y hxy
  rcases hB bn bc hb (outN dn x) (outC dc y) (hOut dn dc x y hd hxy) with
    k | ⟨o, bn', p, bc', hn, hc, ho, hb'⟩
  · exact Or.inl k
  · obtain ⟨dn', dc', hnn, hnc, hd'⟩ := hNext dn dc x y o p hd hxy ho
    refine Or.inr ⟨o, (dn', bn'), p, (dc', bc'), ?_, ?_, ho, hd', hb'⟩
    · unfold ceres_core.sketch.close
      simp [hn, hnn]
    · unfold ceres_core.sketch.close
      simp [hc, hnc]

end ceres_core.sketch
