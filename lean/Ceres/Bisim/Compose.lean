import Aeneas
open Aeneas Aeneas.Std Result

/-!
# Blocks that agree, wired together, agree

A block is a step from an input to an output and the next state, as Aeneas
translates one. Two blocks, the netlist's and ceres's, agree from a pair of
states when, for every pair of inputs that `RI` relates, either the
netlist's state and input are a known difference (`K`), or both steps
succeed with outputs that `RO` relates and states that agree again: the
greatest such relation, `Agree`.

A proof of it starts from one step: `Steps` says that from every pair of
states an invariant `R` relates (a block's `related`), a step agrees and
leads to states `R` relates again, which is what Kani proves of a block.
Then

- `Steps.agree`: the states `R` relates agree (by coinduction);
- `Steps.wire`: block A's output feeds block B's input (as the timer's
  interrupt feeds IF); if A steps, and B steps on the inputs that A's
  related outputs give it, the two together step, with the known
  differences of either;
- `Steps.map_cer`: ceres's side can be replaced by a step that does the
  same from the states `R` relates (ceres's blocks as it interleaves them).
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

/-- One step from the states `R` relates: a known difference, or both steps
succeed with related outputs and states `R` relates. -/
def Steps {Sn Sc In Ic On Oc : Type}
    (net : Sn → In → Result (On × Sn)) (cer : Sc → Ic → Result (Oc × Sc))
    (RI : In → Ic → Prop) (RO : On → Oc → Prop) (K : Sn → In → Prop)
    (R : Sn → Sc → Prop) : Prop :=
  ∀ n c, R n c → ∀ i j, RI i j → K n i ∨
    ∃ o n' p c', net n i = ok (o, n') ∧ cer c j = ok (p, c') ∧ RO o p ∧ R n' c'

theorem Steps.agree {Sn Sc In Ic On Oc : Type}
    {net : Sn → In → Result (On × Sn)} {cer : Sc → Ic → Result (Oc × Sc)}
    {RI : In → Ic → Prop} {RO : On → Oc → Prop} {K : Sn → In → Prop}
    {R : Sn → Sc → Prop} (h : Steps net cer RI RO K R) {n : Sn} {c : Sc} (hr : R n c) :
    Agree net cer RI RO K n c :=
  Agree.coinduct _ _ _ _ _ R (fun n c hr i j hij => h n c hr i j hij) n c hr

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

theorem wire_eq_ok {Sa Sb I Oa Ob : Type} {a : Sa → I → Result (Oa × Sa)}
    {b : Sb → I × Oa → Result (Ob × Sb)} {s : Sa × Sb} {i : I} {o : Oa × Ob}
    {s' : Sa × Sb} (h : wire a b s i = ok (o, s')) :
    a s.1 i = ok (o.1, s'.1) ∧ b s.2 (i, o.1) = ok (o.2, s'.2) := by
  simp only [wire] at h
  obtain ⟨⟨oa, sa⟩, ha, h⟩ := bind_eq_ok h
  obtain ⟨⟨ob, sb⟩, hb, h⟩ := bind_eq_ok h
  simp at h
  obtain ⟨rfl, rfl⟩ := h
  exact ⟨ha, hb⟩

/-- The pair's known differences: A's, or B's on the output A gives. -/
def wireK {Sa Sb I Oa : Type} (a : Sa → I → Result (Oa × Sa))
    (KA : Sa → I → Prop) (KB : Sb → I × Oa → Prop) : Sa × Sb → I → Prop :=
  fun s i => KA s.1 i ∨ ∀ o s', a s.1 i = ok (o, s') → KB s.2 (i, o)

theorem Steps.wire {Sna Snb Sca Scb In Ic Ona Onb Oca Ocb : Type}
    {na : Sna → In → Result (Ona × Sna)} {ca : Sca → Ic → Result (Oca × Sca)}
    {nb : Snb → In × Ona → Result (Onb × Snb)} {cb : Scb → Ic × Oca → Result (Ocb × Scb)}
    {RI : In → Ic → Prop} {RA : Ona → Oca → Prop} {RB : Onb → Ocb → Prop}
    {KA : Sna → In → Prop} {KB : Snb → In × Ona → Prop}
    {R1 : Sna → Sca → Prop} {R2 : Snb → Scb → Prop}
    (hA : Steps na ca RI RA KA R1)
    (hB : Steps nb cb (fun i j => RI i.1 j.1 ∧ RA i.2 j.2) RB KB R2) :
    Steps (wire na nb) (wire ca cb) RI (fun o p => RA o.1 p.1 ∧ RB o.2 p.2)
      (wireK na KA KB) (fun n c => R1 n.1 c.1 ∧ R2 n.2 c.2) := by
  rintro ⟨sna, snb⟩ ⟨sca, scb⟩ ⟨hr1, hr2⟩ i j hij
  rcases hA sna sca hr1 i j hij with kA | ⟨oa, sna', pa, sca', hna, hca, hra, hr1'⟩
  · exact Or.inl (Or.inl kA)
  rcases hB snb scb hr2 (i, oa) (j, pa) ⟨hij, hra⟩ with
    kB | ⟨ob, snb', pb, scb', hnb, hcb, hrb, hr2'⟩
  · refine Or.inl (Or.inr ?_)
    intro o s' h
    rw [hna, Result.ok.injEq] at h
    cases h
    exact kB
  · refine Or.inr ⟨(oa, ob), (sna', snb'), (pa, pb), (sca', scb'), ?_, ?_, ⟨hra, hrb⟩,
      hr1', hr2'⟩
    · simp [bisim.wire, hna, hnb]
    · simp [bisim.wire, hca, hcb]

/-- A block that steps for the inputs `RI` relates steps for those a
stronger relation does (a block wired in that does not look at another's
output). -/
theorem Steps.weaken_in {Sn Sc In Ic On Oc : Type}
    {net : Sn → In → Result (On × Sn)} {cer : Sc → Ic → Result (Oc × Sc)}
    {RI RI' : In → Ic → Prop} {RO : On → Oc → Prop} {K : Sn → In → Prop}
    {R : Sn → Sc → Prop} (h : Steps net cer RI RO K R) (w : ∀ i j, RI' i j → RI i j) :
    Steps net cer RI' RO K R :=
  fun n c hr i j hij => h n c hr i j (w i j hij)

/-- Ceres's step replaced by `cer'`, which does what `cer` does from the
states `R` relates, its output mapped by `f`. -/
theorem Steps.map_cer {Sn Sc In Ic On Oc Oc' : Type}
    {net : Sn → In → Result (On × Sn)} {cer : Sc → Ic → Result (Oc × Sc)}
    {cer' : Sc → Ic → Result (Oc' × Sc)}
    {RI : In → Ic → Prop} {RO : On → Oc → Prop} {RO' : On → Oc' → Prop}
    {K : Sn → In → Prop} {R : Sn → Sc → Prop} (h : Steps net cer RI RO K R)
    (f : Oc → Oc')
    (same : ∀ n c i j o p c', R n c → RI i j → RO o p → cer c j = ok (p, c') →
      cer' c j = ok (f p, c'))
    (out : ∀ o p, RO o p → RO' o (f p)) :
    Steps net cer' RI RO' K R := by
  intro n c hr i j hij
  rcases h n c hr i j hij with k | ⟨o, n', p, c', hn, hc, ho, hr'⟩
  · exact Or.inl k
  · exact Or.inr ⟨o, n', f p, c', hn, same n c i j o p c' hr hij ho hc, out o p ho, hr'⟩

end ceres_core.bisim
