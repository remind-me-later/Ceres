/-
`KaniTimer`, the hypothesis `Ceres/Bisim/TimerIf.lean` takes from Kani's
`netlist_timer_bisimulation`, proved in Lean: the check's `step_agrees` on
Aeneas's translation, by the links (`NetLink`, `CeresLink`) to the hand
models and the step on those (`Step`).
-/
import Std.Tactic.BVDecide
import Ceres.Bisim.TimerIf
import Pilot.Timer.Step
import Pilot.Timer.CeresLink
open Aeneas Aeneas.Std Result ceres_core

namespace Pilot.Timer

theorem mul16 (a b : U16) : (core.num.U16.wrapping_mul a b).bv = a.bv * b.bv := by
  simp [core.num.U16.wrapping_mul_bv_eq]

/-- What `related` asks of ceres's clock beyond the hand model's fields. -/
def pinned (c : Clock) : Bool :=
  decide (c.div_cycles = 0#i32) && !c.stopped && decide (c.apu_delay = 0#u16) &&
    c.apu_late_bit.isNone

theorem related_link (net : NetTimer) (clock : Clock) :
    timing.check.related net clock = ok (related (ofA net) (ofC clock) && pinned clock) := by
  unfold timing.check.related
  simp only [clock_of, lift, bind_ok, ite_ok, ok.injEq]
  dsimp only [related, pinned, ofA, ofC, clockOf, core.option.Option.is_none]
  by_cases hd : clock.div_cycles = 0#i32 <;>
    simp only [hd, decide_true, decide_false, eq8, eq16, mul16, and8, lit8, lit16, decide_not,
      Bool.decide_eq_true, Bool.and_false, Bool.false_and, Bool.and_true, Bool.true_and]
  bv_decide

theorem known_difference_link (net : NetTimer) (access : ceres_netlist.timer.Access) :
    timing.check.known_difference net access = ok (knownDifference (ofA net) (accOf access)) := by
  rcases access with _ | r | ⟨r, v⟩ <;> (try cases r) <;>
    simp only [timing.check.known_difference, clock_of, lift, bind_ok, uncurry_apply_pair,
      ok.injEq] <;>
    dsimp only [knownDifference, accOf, ofA, clockOf] <;>
    simp only [eq8, and8, lit8, lit16, core.num.U16.wrapping_add_bv_eq, decide_not,
      Bool.decide_eq_true] <;>
    bv_decide

theorem same_read_link (a b : Bool × BitVec 8) :
    timing.check.same_read (readOf a) (readOf' b) = ok (a.1 == b.1 && (!a.1 || a.2 == b.2)) := by
  obtain ⟨fa, va⟩ := a
  obtain ⟨fb, vb⟩ := b
  cases fa <;> cases fb <;> simp [timing.check.same_read, readOf, readOf']
  rfl

theorem same_interrupt_link (n : Bool) (f : Bool × BitVec 8) :
    timing.check.same_interrupt n (optOf f) = ok (bif f.1 then n && f.2 == 2#8 else !n) := by
  obtain ⟨b, v⟩ := f
  cases b <;> simp [timing.check.same_interrupt, optOf, eq8]
  rfl

theorem related_reload (n : Net) (c : Cer) (h : related n c = true) : c.reload ≤ 8#8 := by
  revert h
  simp only [related, clockOf]
  bv_decide

/-- Kani's hypothesis for the timer, proved: every related pair steps to a
related pair, with the same read and interrupt, as `timing::check` asks. -/
theorem kani_timer : timer_if.KaniTimer := by
  intro net clock access mask
  have hstep := step (ofA net) (ofC clock) (accOf access)
  unfold timing.check.step_agrees
  rw [related_link, known_difference_link]
  simp only [bind_ok]
  cases hrel : related (ofA net) (ofC clock) <;> cases hpin : pinned clock <;>
    cases hkd : knownDifference (ofA net) (accOf access) <;>
    simp only [Bool.and_true, Bool.and_false, Bool.or_true,
      Bool.or_false, ↓reduceIte, Bool.false_eq_true] <;>
    try rfl
  simp only [pinned, Bool.and_eq_true, decide_eq_true_eq, Bool.not_eq_true',
    Option.isNone_iff_eq_none] at hpin
  obtain ⟨⟨⟨hd, hs⟩, ha⟩, -⟩ := hpin
  have hr : clock.tima_reload_pending.bv ≤ 8#8 := related_reload _ _ hrel
  obtain ⟨c', h', hm, hofc, hs', hd', ha', hl'⟩ := cer_m_cycle clock mask access hs hd ha hr
  simp only [timing.check.CheckHost.new, bind_ok, m_cycle, hm, uncurry_apply_pair,
    same_read_link, same_interrupt_link, related_link, ofA_toA, hofc]
  simp only [decide_not, decide_true, Bool.not_true, Bool.false_eq_true, ↓reduceIte, pinned, hs',
    hd', ha', hl', Bool.not_false, Option.isNone_none, Bool.and_true, ok.injEq]
  simp only [stepAgrees, hrel, hkd, Bool.not_true, Bool.false_or] at hstep
  generalize (ofA net).mCycle (accOf access) = nm at hstep ⊢
  generalize (ofC clock).mCycle (accOf access) = cm at hstep ⊢
  obtain ⟨⟨r1, r2⟩, irq, n'⟩ := nm
  obtain ⟨⟨s1, s2⟩, ⟨i1, i2⟩, tw, c''⟩ := cm
  simp only at hstep ⊢
  revert hstep
  bv_decide

end Pilot.Timer
