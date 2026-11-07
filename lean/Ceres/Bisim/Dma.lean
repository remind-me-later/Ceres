import Ceres.Bisim.TimerIf
open Aeneas Aeneas.Std Result ceres_core ceres_core.bisim ceres_core.timer_if

/-!
# The OAM DMA, beside the timer and IF

Kani proves the DMA's M-cycle for every pair of states
(`netlist_dma_bisimulation` asserts `memory::dma::check::step_agrees`), the
hypothesis `KaniDma`. The DMA takes its own access and none of the timer's
or IF's outputs, so it is wired after them (`Steps.wire`), looking only at
the inputs (`Steps.weaken_in`). Ceres's side of the pair is the timer's and
IF's M-cycle as ceres runs them, then the DMA's run of the M-cycle's four
T-cycles, which is how `Gb::run_dma` runs it (`doc/assumptions.toml`:
dma-one-run-per-m-cycle).
-/

namespace ceres_core.dma

abbrev NetDma := ceres_netlist.dma.NetDma
abbrev CeresDma := memory.dma.check.CeresDma

/-- The netlist's M-cycle: a read of FF46, the OAM write, and the run
before and after the A edge. -/
def netDma (n : NetDma) (x : NetIn × ((Option Std.U8 × Bool) × (Option Std.U8 × Option Std.U8))) :
    Result ((Option Std.U8 × Option (Std.U16 × Std.U8) × Bool × Bool) × NetDma) :=
  memory.dma.check.net_m_cycle n x.1.dma

/-- Ceres's M-cycle: what it shows (`CeresOut`). -/
def ceresDma (c : CeresDma) (y : CeresIn × (Option Std.U8 × Option Std.U8 × Option Std.U8)) :
    Result (memory.dma.check.CeresOut × CeresDma) :=
  memory.dma.check.ceres_m_cycle c y.1.dma

/-- The same read and OAM write; ceres's transfer runs as the netlist's
after the access and after the M-cycle, and blocks the CPU's OAM accesses
when the netlist's runs. -/
def DmaOut (o : Option Std.U8 × Option (Std.U16 × Std.U8) × Bool × Bool)
    (p : memory.dma.check.CeresOut) : Prop :=
  o.1 = p.read ∧ o.2.1 = p.written ∧ p.runs_after_access = o.2.2.1 ∧ p.runs = o.2.2.2 ∧
    p.blocks_oam = o.2.2.2

/-- What Kani proves. -/
def KaniDma : Prop :=
  ∀ net ceres access, memory.dma.check.step_agrees net ceres access = ok true

theorem same_write_true {a b : Option (Std.U16 × Std.U8)}
    (h : memory.dma.check.same_write a b = ok true) : a = b := by
  unfold memory.dma.check.same_write at h
  rcases a with _ | ⟨x1, x2⟩ <;> rcases b with _ | ⟨y1, y2⟩ <;> simp_all

theorem same_read_true' {a b : Option Std.U8}
    (h : memory.dma.check.same_read a b = ok true) : a = b := by
  unfold memory.dma.check.same_read at h
  rcases a with _ | x <;> rcases b with _ | y <;> simp_all

/-- The DMA's M-cycle, from every related pair (Kani's step). -/
-- @assumes dma-step-agrees
theorem dma_steps (kani : KaniDma) :
    Steps netDma ceresDma (fun x y => x.1.dma = y.1.dma) DmaOut (fun _ _ => False)
      (fun n c => memory.dma.check.related n c = ok true) := by
  intro n c h x y hxy
  right
  have k := kani n c x.1.dma
  unfold memory.dma.check.step_agrees at k
  obtain ⟨b, hb, k⟩ := bind_eq_ok k
  rw [h, Result.ok.injEq] at hb
  subst hb
  simp only [↓reduceIte] at k
  obtain ⟨⟨⟨nr, nw, rb, ra⟩, n'⟩, hn, k⟩ := bind_eq_ok k
  obtain ⟨⟨out, c'⟩, hc, k⟩ := bind_eq_ok k
  obtain ⟨s1, hs1, k⟩ := bind_eq_ok k
  obtain ⟨s2, hs2, k⟩ := bind_eq_ok k
  obtain ⟨r, hr, k⟩ := bind_eq_ok k
  simp only [Result.ok.injEq, Bool.and_eq_true, decide_eq_true_eq] at k
  obtain ⟨⟨⟨⟨⟨⟨e1, e2⟩, e3⟩, e4⟩, e5⟩, -⟩, e7⟩ := k
  subst e1 e2 e7
  refine ⟨(nr, nw, rb, ra), n', out, c', hn, ?_, ⟨same_read_true' hs1, same_write_true hs2,
    e3, e4, e5⟩, hr⟩
  simp only [ceresDma]
  rw [← hxy]
  exact hc

/-- The timer, IF and the OAM DMA, as ceres runs them: from related states,
every M-cycle gives the same timer read, IF read, interrupt taken, FF46
read and OAM write, and the DMA's run and OAM blocking alike, unless it is
one of the timer's known differences. -/
-- @assumes timer-step-agrees
-- @assumes if-step-agrees
-- @assumes blocks-in-sequence
-- @assumes dma-step-agrees
-- @assumes dma-one-run-per-m-cycle
theorem timer_if_dma_agree (kt : KaniTimer) (ki : KaniIf) (ks : KaniSeq) (kd : KaniDma)
    (n : NetTimer) (c : CeresTimer) (nif : NetIf) (cif : CeresIf) (nd : NetDma)
    (cd : CeresDma)
    (h : timing.check.related n c = ok true)
    (hif : interrupts.check.related nif cif = ok true)
    (hd : memory.dma.check.related nd cd = ok true) :
    Agree (wire (wire netTimer netIf) netDma) (wire ceresJoint ceresDma) SameIn
      (fun o p => JointOut o.1 p.1 ∧ DmaOut o.2 p.2)
      (wireK (wire netTimer netIf) (wireK netTimer TimerK (fun _ _ => False))
        (fun _ _ => False)) ((n, nif), nd) ((c, cif), cd) := by
  have steps := (timer_and_if_steps kt ki ks).wire
    ((dma_steps kd).weaken_in (fun x y hxy => hxy.1.2.2.1))
  refine steps.agree (n := ((n, nif), nd)) (c := ((c, cif), cd)) ?_
  exact ⟨⟨h, hif⟩, hd⟩

end ceres_core.dma
