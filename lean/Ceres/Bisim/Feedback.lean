import Ceres.Bisim.Compose
open Aeneas Aeneas.Std Result

/-!
# Feedback through a block's state, and a block in phases

`Steps.wire` passes a signal one way within a step: block B sees A's
output, A never sees B's. On the chip some signals go back, but only from
a block's state: the PPU's VRAM and OAM locks, which the memory needs for
the CPU's access before the PPU's dots read the memory, are a function of
where the PPU is, not of what it reads in the M-cycle. `feedback` is that
loop: A gives an output from its state alone (a Moore output), B steps
with it, and A then steps with B's output. `Steps.feedback` carries the
claims over, A's in two parts: related states give related Moore outputs,
and A's step from related states and inputs.

`Steps.phases`: a block whose invariant is one of several (the PPU's line:
the OAM scan, mode 3, HBlank, VBlank), each proved to step into one of
them, steps under their union.
-/

namespace ceres_core.bisim

/-- A's output from its state, B with it, then A with B's output: the
outputs of both, A's Moore output and its step's. -/
def feedback {Sa Sb I Oa Pa Ob : Type} (outA : Sa → Oa)
    (stepA : Sa → I × Ob → Result (Pa × Sa)) (b : Sb → I × Oa → Result (Ob × Sb)) :
    Sa × Sb → I → Result ((Oa × Ob × Pa) × (Sa × Sb)) := fun s i => do
  let oa := outA s.1
  let (ob, sb) ← b s.2 (i, oa)
  let (pa, sa) ← stepA s.1 (i, ob)
  ok ((oa, ob, pa), (sa, sb))

/-- The loop's known differences: B's on A's Moore outputs, or A's on the
outputs B gives. -/
def feedbackK {Sa Sb Ta Tb I J Oa Pa Ob Qb : Type} (outA : Sa → Oa) (outC : Ta → Pa)
    (b : Sb → I × Oa → Result (Ob × Sb)) (cb : Tb → J × Pa → Result (Qb × Tb))
    (KA : Sa → Ta → I × Ob → J × Qb → Prop) (KB : Sb → Tb → I × Oa → J × Pa → Prop) :
    Sa × Sb → Ta × Tb → I → J → Prop :=
  fun s t i j => KB s.2 t.2 (i, outA s.1) (j, outC t.1) ∨
    ∀ o s' p t', b s.2 (i, outA s.1) = ok (o, s') → cb t.2 (j, outC t.1) = ok (p, t') →
      KA s.1 t.1 (i, o) (j, p)

theorem Steps.feedback {Sna Snb Sca Scb In Ic Ona Oca Pna Pca Onb Ocb : Type}
    {outN : Sna → Ona} {outC : Sca → Oca}
    {na : Sna → In × Onb → Result (Pna × Sna)} {ca : Sca → Ic × Ocb → Result (Pca × Sca)}
    {nb : Snb → In × Ona → Result (Onb × Snb)} {cb : Scb → Ic × Oca → Result (Ocb × Scb)}
    {RI : In → Ic → Prop} {RM : Ona → Oca → Prop} {RA : Pna → Pca → Prop}
    {RB : Onb → Ocb → Prop}
    {KA : Sna → Sca → In × Onb → Ic × Ocb → Prop}
    {KB : Snb → Scb → In × Ona → Ic × Oca → Prop}
    {R1 : Sna → Sca → Prop} {R2 : Snb → Scb → Prop}
    (hM : ∀ n c, R1 n c → RM (outN n) (outC c))
    (hB : Steps nb cb (fun i j => RI i.1 j.1 ∧ RM i.2 j.2) RB KB R2)
    (hA : Steps na ca (fun i j => RI i.1 j.1 ∧ RB i.2 j.2) RA KA R1) :
    Steps (feedback outN na nb) (feedback outC ca cb) RI
      (fun o p => RM o.1 p.1 ∧ RB o.2.1 p.2.1 ∧ RA o.2.2 p.2.2)
      (feedbackK outN outC nb cb KA KB) (fun n c => R1 n.1 c.1 ∧ R2 n.2 c.2) := by
  rintro ⟨sna, snb⟩ ⟨sca, scb⟩ ⟨hr1, hr2⟩ i j hij
  have hm := hM sna sca hr1
  rcases hB snb scb hr2 (i, outN sna) (j, outC sca) ⟨hij, hm⟩ with
    kB | ⟨ob, snb', pb, scb', hnb, hcb, hrb, hr2'⟩
  · exact Or.inl (Or.inl kB)
  rcases hA sna sca hr1 (i, ob) (j, pb) ⟨hij, hrb⟩ with
    kA | ⟨oa, sna', pa, sca', hna, hca, hra, hr1'⟩
  · refine Or.inl (Or.inr ?_)
    intro o s' p t' h hc
    rw [hnb, Result.ok.injEq] at h
    rw [hcb, Result.ok.injEq] at hc
    cases h
    cases hc
    exact kA
  · refine Or.inr ⟨(outN sna, ob, oa), (sna', snb'), (outC sca, pb, pa), (sca', scb'), ?_, ?_,
      ⟨hm, hrb, hra⟩, hr1', hr2'⟩
    · simp [bisim.feedback, hna, hnb]
    · simp [bisim.feedback, hca, hcb]

/-- One step from the states `R` relates into those `R'` relates. -/
def StepsInto {Sn Sc In Ic On Oc : Type}
    (net : Sn → In → Result (On × Sn)) (cer : Sc → Ic → Result (Oc × Sc))
    (RI : In → Ic → Prop) (RO : On → Oc → Prop) (K : Sn → Sc → In → Ic → Prop)
    (R R' : Sn → Sc → Prop) : Prop :=
  ∀ n c, R n c → ∀ i j, RI i j → K n c i j ∨
    ∃ o n' p c', net n i = ok (o, n') ∧ cer c j = ok (p, c') ∧ RO o p ∧ R' n' c'

/-- A block in phases: from each phase's states, a step reaches one of the
phases'; then it steps under their union. -/
theorem Steps.phases {Sn Sc In Ic On Oc ι : Type}
    {net : Sn → In → Result (On × Sn)} {cer : Sc → Ic → Result (Oc × Sc)}
    {RI : In → Ic → Prop} {RO : On → Oc → Prop} {K : Sn → Sc → In → Ic → Prop}
    {R : ι → Sn → Sc → Prop}
    (h : ∀ k, StepsInto net cer RI RO K (R k) (fun n c => ∃ k', R k' n c)) :
    Steps net cer RI RO K (fun n c => ∃ k, R k n c) := by
  rintro n c ⟨k, hr⟩ i j hij
  exact h k n c hr i j hij

/-- A block's step inside a larger state: the larger step takes the
block's state out (`pn`, `pc`), steps it on the input made from its own
(`inN`, `inC`), puts it back (`setN`, `setC`) and gives an output made
from the block's (`outN`, `outC`), in the states the block's claim is
about (`hN`, `hC`: elsewhere the larger step may be another block's).
The rest of the state is left as it was: `F`, what relates it, holds of
whatever the block's states become.
Then the larger step carries the block's claim: from states whose blocks
`R` relates and whose rest `F` does, into those `Q'` makes of the
block's `R'`. -/
theorem StepsInto.embed {Sn Sc In Ic On Oc Ln Lc DIn DIc DOn DOc : Type}
    {net : Sn → In → Result (On × Sn)} {cer : Sc → Ic → Result (Oc × Sc)}
    {RI : In → Ic → Prop} {RO : On → Oc → Prop} {K : Sn → Sc → In → Ic → Prop}
    {R R' : Sn → Sc → Prop} (h : StepsInto net cer RI RO K R R')
    {dotN : Ln → DIn → Result (DOn × Ln)} {dotC : Lc → DIc → Result (DOc × Lc)}
    (pn : Ln → Sn) (pc : Lc → Sc) (setN : Ln → Sn → Ln) (setC : Lc → Sc → Lc)
    (inN : DIn → In) (inC : DIc → Ic) (outN : On → DOn) (outC : Oc → DOc)
    (F Q' : Ln → Lc → Prop)
    (hN : ∀ n c i, R (pn n) (pc c) → F n c →
      dotN n i = (do let (o, s) ← net (pn n) (inN i); ok (outN o, setN n s)))
    (hC : ∀ n c j, R (pn n) (pc c) → F n c →
      dotC c j = (do let (p, t) ← cer (pc c) (inC j); ok (outC p, setC c t)))
    {RI' : DIn → DIc → Prop} (hI : ∀ i j, RI' i j → RI (inN i) (inC j))
    {RO' : DOn → DOc → Prop} (hO : ∀ o p, RO o p → RO' (outN o) (outC p))
    (hQ' : ∀ n c s t, F n c → R' s t → Q' (setN n s) (setC c t)) :
    StepsInto dotN dotC RI' RO' (fun n c i j => K (pn n) (pc c) (inN i) (inC j))
      (fun n c => R (pn n) (pc c) ∧ F n c) Q' := by
  rintro n c ⟨hr, hf⟩ i j hij
  rcases h (pn n) (pc c) hr (inN i) (inC j) (hI i j hij) with k | ⟨o, s, p, t, hn, hc, ho, hr'⟩
  · exact Or.inl k
  · refine Or.inr ⟨outN o, setN n s, outC p, setC c t, ?_, ?_, hO o p ho, hQ' n c s t hf hr'⟩
    · simp [hN n c i hr hf, hn]
    · simp [hC n c j hr hf, hc]

/-- A block with a netlist-only block beside it, which goes through and
keeps an invariant (`P` into `P'`): together they step as the first. -/
theorem StepsInto.beside {Sn Sc In Ic On Oc S I2 : Type}
    {net : Sn → In → Result (On × Sn)} {cer : Sc → Ic → Result (Oc × Sc)}
    {RI : In → Ic → Prop} {RO : On → Oc → Prop} {K : Sn → Sc → In → Ic → Prop}
    {R R' : Sn → Sc → Prop} (h : StepsInto net cer RI RO K R R')
    {b : S → I2 → Result S} {P P' : S → Prop}
    (hb : ∀ s, P s → ∀ i, ∃ s', b s i = ok s' ∧ P' s') :
    StepsInto
      (fun (x : Sn × S) (i : In × I2) => do
        let (o, n') ← net x.1 i.1
        let s' ← b x.2 i.2
        ok (o, (n', s')))
      cer (fun i j => RI i.1 j) RO (fun x c i j => K x.1 c i.1 j)
      (fun x c => R x.1 c ∧ P x.2) (fun x c => R' x.1 c ∧ P' x.2) := by
  rintro ⟨n, s⟩ c ⟨hr, hp⟩ ⟨i, i2⟩ j hij
  rcases h n c hr i j hij with k | ⟨o, n', p, c', hn, hc, ho, hr'⟩
  · exact Or.inl k
  · obtain ⟨s', hs, hp'⟩ := hb s hp i2
    exact Or.inr ⟨o, (n', s'), p, c', by simp [hn, hs], hc, ho, hr', hp'⟩

end ceres_core.bisim
