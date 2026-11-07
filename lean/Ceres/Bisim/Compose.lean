import Aeneas
open Aeneas Aeneas.Std Result

/-!
# Blocks that agree, wired together, agree

A block is a step from an input to an output and the next state, as Aeneas
translates one. Two blocks, the netlist's and ceres's, agree from a pair of
states when, for every pair of inputs that `RI` relates, either the
netlist's state and input are a known difference (`K`), or both steps
succeed with outputs that `RO` relates and states that agree again: the
greatest such relation.

`agree_wire`: block A's output feeds block B's input (as the timer's
interrupt feeds IF). If A agrees, and B agrees on the inputs that A's
related outputs give it, the two together agree, with the known differences
of either.
-/

namespace ceres_core.bisim

coinductive Agree {Sn Sc In Ic On Oc : Type}
    (net : Sn → In → Result (On × Sn)) (cer : Sc → Ic → Result (Oc × Sc))
    (RI : In → Ic → Prop) (RO : On → Oc → Prop) (K : Sn → In → Prop) :
    Sn → Sc → Prop where
  | step (n : Sn) (c : Sc) :
      (∀ i j, RI i j → K n i ∨
        ∃ o n' p c', net n i = ok (o, n') ∧ cer c j = ok (p, c') ∧ RO o p ∧
          Agree net cer RI RO K n' c') →
      Agree net cer RI RO K n c

theorem bind_eq_ok {α β} {x : Result α} {f : α → Result β} {v : β}
    (h : (x >>= f) = ok v) : ∃ y, x = ok y ∧ f y = ok v := by
  cases x with
  | ret y => exact ⟨y, rfl, by change (ok y >>= f) = ok v at h; simpa using h⟩
  | vis e k => change (Result.vis e k >>= f) = ok v at h; simp [bind_tc_vis, vis_not_ok] at h
  | div => change (Result.div >>= f) = ok v at h; simp [bind_tc_div, div_not_ok] at h

/-- A then B, B taking the outside input and A's output. -/
def wire {Sa Sb I Oa Ob : Type} (a : Sa → I → Result (Oa × Sa))
    (b : Sb → I × Oa → Result (Ob × Sb)) :
    Sa × Sb → I → Result ((Oa × Ob) × (Sa × Sb)) := fun s i => do
  let (oa, sa) ← a s.1 i
  let (ob, sb) ← b s.2 (i, oa)
  ok ((oa, ob), (sa, sb))

/-- The pair's known differences: A's, or B's on the output A gives. -/
def wireK {Sa Sb I Oa : Type} (a : Sa → I → Result (Oa × Sa))
    (KA : Sa → I → Prop) (KB : Sb → I × Oa → Prop) : Sa × Sb → I → Prop :=
  fun s i => KA s.1 i ∨ ∀ o s', a s.1 i = ok (o, s') → KB s.2 (i, o)

theorem agree_wire {Sna Snb Sca Scb In Ic Ona Onb Oca Ocb : Type}
    {na : Sna → In → Result (Ona × Sna)} {ca : Sca → Ic → Result (Oca × Sca)}
    {nb : Snb → In × Ona → Result (Onb × Snb)} {cb : Scb → Ic × Oca → Result (Ocb × Scb)}
    {RI : In → Ic → Prop} {RA : Ona → Oca → Prop} {RB : Onb → Ocb → Prop}
    {KA : Sna → In → Prop} {KB : Snb → In × Ona → Prop}
    (sna : Sna) (snb : Snb) (sca : Sca) (scb : Scb)
    (hA : Agree na ca RI RA KA sna sca)
    (hB : Agree nb cb (fun i j => RI i.1 j.1 ∧ RA i.2 j.2) RB KB snb scb) :
    Agree (wire na nb) (wire ca cb) RI (fun o p => RA o.1 p.1 ∧ RB o.2 p.2)
      (wireK na KA KB) (sna, snb) (sca, scb) := by
  refine Agree.coinduct _ _ _ _ _
    (fun n c => Agree na ca RI RA KA n.1 c.1 ∧
      Agree nb cb (fun i j => RI i.1 j.1 ∧ RA i.2 j.2) RB KB n.2 c.2)
    ?_ (sna, snb) (sca, scb) ⟨hA, hB⟩
  rintro ⟨sna, snb⟩ ⟨sca, scb⟩ ⟨hA, hB⟩ i j hij
  cases hA with
  | step _ _ hA =>
  rcases hA i j hij with kA | ⟨oa, sna', pa, sca', hna, hca, hra, hA'⟩
  · exact Or.inl (Or.inl kA)
  cases hB with
  | step _ _ hB =>
  rcases hB (i, oa) (j, pa) ⟨hij, hra⟩ with kB | ⟨ob, snb', pb, scb', hnb, hcb, hrb, hB'⟩
  · refine Or.inl (Or.inr ?_)
    intro o s' h
    rw [hna, Result.ok.injEq] at h
    cases h
    exact kB
  · refine Or.inr ⟨(oa, ob), (sna', snb'), (pa, pb), (sca', scb'), ?_, ?_, ⟨hra, hrb⟩, hA', hB'⟩
    · simp [wire, hna, hnb]
    · simp [wire, hca, hcb]

end ceres_core.bisim
