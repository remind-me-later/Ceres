import Ceres.Gen.Funs
import Ceres.Bisim.Compose
open Aeneas Aeneas.Std Result ceres_core ceres_core.bisim

/-!
# ceres's timer and IF, together, are bisimilar to the netlist's

Kani proves each block's M-cycle for every pair of states: the harnesses
`netlist_timer_bisimulation` and `netlist_if_bisimulation` assert
`timing::check::step_agrees` and `interrupts::check::step_agrees` (ceres-core)
with every argument free, every field of both states included. Lean takes
those as the hypotheses `KaniTimer` and `KaniIf`, on Aeneas's translation of
the same functions (`Ceres.Gen`), proves each block's `Agree` by
coinduction, and wires the timer's interrupt into IF with `agree_wire`.

The netlist's side of the pair is its two steps one after the other, as
NetTimer and NetIf share no state; so is ceres's, which interleaves them
T-cycle by T-cycle (`doc/assumptions.toml`: blocks-in-sequence). The timer's
netlist step runs from B, IF's from E (timer-cut-at-b).
-/

namespace ceres_core.timer_if

abbrev NetTimer := ceres_netlist.timer.NetTimer
abbrev CeresTimer := timing.timer.Clock
abbrev NetIf := ceres_netlist.interrupts.NetIf
abbrev CeresIf := interrupts.Interrupts

/-- An M-cycle's inputs on the netlist: an access to the timer, one to IF,
and the other sources' rises by phase. -/
structure NetIn where
  timer : ceres_netlist.timer.Access
  ifAccess : ceres_netlist.interrupts.Access
  rises : Array Std.U8 8#usize

/-- The same on ceres, which also takes the serial's DIV mask. -/
structure CeresIn where
  timer : ceres_netlist.timer.Access
  mask : Std.U16
  ifAccess : ceres_netlist.interrupts.Access
  rises : Array Std.U8 8#usize

def SameIn (i : NetIn) (j : CeresIn) : Prop :=
  i.timer = j.timer ∧ i.ifAccess = j.ifAccess ∧ i.rises = j.rises

/-! The timer: the read and the interrupt (the netlist's at the A edge;
ceres's T-cycle, and whether a second came). -/

def netTimer (n : NetTimer) (i : NetIn) : Result ((Option Std.U8 × Bool) × NetTimer) :=
  n.m_cycle i.timer

def ceresTimer (c : CeresTimer) (j : CeresIn) :
    Result ((Option Std.U8 × Option Std.U8 × Bool) × CeresTimer) := do
  let (o, c', _) ← timing.check.ceres_m_cycle c
    { serial_div_mask := j.mask, timer_interrupts := 0#u32 } j.timer
  ok (o, c')

def TimerOut (o : Option Std.U8 × Bool) (p : Option Std.U8 × Option Std.U8 × Bool) : Prop :=
  o.1 = p.1 ∧ timing.check.same_interrupt o.2 p.2.1 = ok true ∧ p.2.2 = false

def TimerK (n : NetTimer) (i : NetIn) : Prop :=
  timing.check.known_difference n i.timer = ok true

/-! IF: its access, the other sources' rises and the timer's interrupt; the
read and the interrupt taken. -/

def netIf (n : NetIf) (x : NetIn × (Option Std.U8 × Bool)) :
    Result ((Option Std.U8 × Option Std.U8) × NetIf) :=
  interrupts.check.net_step n x.1.ifAccess x.1.rises x.2.2

def ceresIf (c : CeresIf) (x : CeresIn × (Option Std.U8 × Option Std.U8 × Bool)) :
    Result ((Option Std.U8 × Option Std.U8) × CeresIf) :=
  interrupts.check.ceres_step c x.1.ifAccess x.1.rises x.2.2.1

/-- What Kani proves. -/
def KaniTimer : Prop :=
  ∀ net clock access mask, timing.check.step_agrees net clock access mask = ok true

def KaniIf : Prop :=
  ∀ net ints access rises nt ct,
    interrupts.check.step_agrees net ints access rises nt ct = ok true

theorem same_read_true {a b : Option Std.U8}
    (h : timing.check.same_read a b = ok true) : a = b := by
  unfold timing.check.same_read at h
  rcases a with _ | x <;> rcases b with _ | y <;> simp_all

-- @assumes timer-step-agrees
theorem timer_agrees (kani : KaniTimer) (n : NetTimer) (c : CeresTimer)
    (h : timing.check.related n c = ok true) :
    Agree netTimer ceresTimer SameIn TimerOut TimerK n c := by
  refine Agree.coinduct _ _ _ _ _ (fun n c => timing.check.related n c = ok true) ?_ n c h
  intro n c h i j hij
  obtain ⟨ht, -, -⟩ := hij
  have k := kani n c i.timer j.mask
  unfold timing.check.step_agrees at k
  obtain ⟨b, hb, k⟩ := bind_eq_ok k
  rw [h, Result.ok.injEq] at hb
  subst hb
  obtain ⟨d, hd, k⟩ := bind_eq_ok k
  cases d
  · right
    simp only [Bool.false_eq_true, ↓reduceIte, decide_not, decide_false, not_true_eq_false,
      Bool.or_false] at k
    obtain ⟨⟨⟨nr, ni⟩, n'⟩, hn, k⟩ := bind_eq_ok k
    obtain ⟨⟨⟨cr, ct, twice⟩, c', host'⟩, hc, k⟩ := bind_eq_ok k
    obtain ⟨s, hs, k⟩ := bind_eq_ok k
    obtain ⟨si, hsi, k⟩ := bind_eq_ok k
    obtain ⟨r, hr, k⟩ := bind_eq_ok k
    simp only [Result.ok.injEq, Bool.and_eq_true, Bool.not_eq_true', Bool.decide_eq_true] at k
    obtain ⟨⟨⟨hs', hsi'⟩, htw⟩, hr'⟩ := k
    subst hs' hsi' htw hr'
    refine ⟨(nr, ni), n', (cr, ct, false), c', hn, ?_, ⟨same_read_true hs, hsi, rfl⟩, hr⟩
    simp only [ceresTimer]
    rw [← ht, hc]
    simp
  · left
    exact hd

-- @assumes if-step-agrees
theorem if_agrees (kani : KaniIf) (n : NetIf) (c : CeresIf)
    (h : interrupts.check.related n c = ok true) :
    Agree netIf ceresIf (fun x y => SameIn x.1 y.1 ∧ TimerOut x.2 y.2) (· = ·)
      (fun _ _ => False) n c := by
  refine Agree.coinduct _ _ _ _ _ (fun n c => interrupts.check.related n c = ok true) ?_ n c h
  rintro n c h ⟨i, o⟩ ⟨j, p⟩ ⟨⟨-, ha, hr⟩, -, hsi, -⟩
  right
  have k := kani n c i.ifAccess i.rises o.2 p.2.1
  unfold interrupts.check.step_agrees at k
  obtain ⟨b, hb, k⟩ := bind_eq_ok k
  rw [h, Result.ok.injEq] at hb
  subst hb
  obtain ⟨b1, hb1, k⟩ := bind_eq_ok k
  rw [hsi, Result.ok.injEq] at hb1
  subst hb1
  simp only [Bool.false_eq_true, ↓reduceIte, decide_false, not_true_eq_false,
    Bool.or_false] at k
  obtain ⟨⟨⟨nr, nt⟩, n'⟩, hn, k⟩ := bind_eq_ok k
  obtain ⟨⟨⟨cr, ct⟩, c'⟩, hc, k⟩ := bind_eq_ok k
  obtain ⟨s1, hs1, k⟩ := bind_eq_ok k
  obtain ⟨s2, hs2, k⟩ := bind_eq_ok k
  obtain ⟨r, hr', k⟩ := bind_eq_ok k
  simp only [Result.ok.injEq, Bool.and_eq_true] at k
  obtain ⟨⟨e1, e2⟩, e3⟩ := k
  subst e1 e2 e3
  refine ⟨(nr, nt), n', (cr, ct), c', hn, ?_, ?_, hr'⟩
  · simp only [ceresIf]
    rw [← ha, ← hr]
    exact hc
  · rw [same_read_true hs1, same_read_true hs2]

/-- ceres's timer and IF agree with the netlist's from any related states:
every M-cycle gives the same timer read, the same interrupt (none too early
in ceres), the same IF read and the same interrupt taken, unless it is one
of the timer's known differences. -/
-- @assumes timer-step-agrees
-- @assumes if-step-agrees
-- @assumes blocks-in-sequence
-- @assumes timer-cut-at-b
theorem timer_and_if_agree (kt : KaniTimer) (ki : KaniIf)
    (n : NetTimer) (c : CeresTimer) (nif : NetIf) (cif : CeresIf)
    (h : timing.check.related n c = ok true)
    (hif : interrupts.check.related nif cif = ok true) :
    Agree (wire netTimer netIf) (wire ceresTimer ceresIf) SameIn
      (fun o p => TimerOut o.1 p.1 ∧ o.2 = p.2)
      (wireK netTimer TimerK (fun _ _ => False)) (n, nif) (c, cif) :=
  agree_wire n nif c cif (timer_agrees kt n c h) (if_agrees ki nif cif hif)

end ceres_core.timer_if

