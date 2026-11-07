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

Both blocks' netlist steps are cut at the start of E, where ceres's
M-cycle begins, and the netlist's side of the pair is the two steps one
after the other: NetTimer and NetIf share no state but the interrupt's
wire. IF also takes the STAT line's rises, by phase on the netlist and by
T-cycle on ceres, as inputs (`SameIn` relates them as IF tells them apart),
which the line timing's step gives when it is wired in front
(`Ceres/Bisim/Lcd.lean`). Ceres runs the two T-cycle by T-cycle (`check::ceres_m_cycle_joint`);
Kani proves that this is the two steps in sequence (`check::in_sequence`,
`ceres_timer_and_if_in_sequence`), the hypothesis `KaniSeq`, so the main
theorem, `timer_and_if_agree`, is about ceres's M-cycle as it runs it.
-/

namespace ceres_core.timer_if

abbrev NetTimer := ceres_netlist.timer.NetTimer
abbrev CeresTimer := timing.timer.Clock
abbrev NetIf := ceres_netlist.interrupts.NetIf
abbrev CeresIf := interrupts.Interrupts

/-- An M-cycle's inputs on the netlist: an access to the timer, one to IF,
one to the OAM DMA, the other sources' rises by phase, an access to the
line timing, the STAT line's rises by phase (which the line timing's
step gives, `Ceres/Bisim/Lcd.lean`), and the half dot of the M-cycle in
which mode 3 ends, if it does (the pixel pipeline's, which the line
timing's HBlank source takes). -/
structure NetIn where
  timer : ceres_netlist.timer.Access
  ifAccess : ceres_netlist.interrupts.Access
  dma : memory.dma.check.Access
  rises : Array Std.U8 8#usize
  lcd : ppu.display.timing.check.Access
  stat : Array Bool 8#usize
  mode3End : Option Std.U8

/-- The same on ceres, which also takes the serial's DIV mask, and has the
STAT line's rises by T-cycle. -/
structure CeresIn where
  timer : ceres_netlist.timer.Access
  mask : Std.U16
  ifAccess : ceres_netlist.interrupts.Access
  dma : memory.dma.check.Access
  rises : Array Std.U8 8#usize
  lcd : ppu.display.timing.check.Access
  stat : Array Bool 4#usize
  mode3End : Option Std.U8

def SameIn (i : NetIn) (j : CeresIn) : Prop :=
  i.timer = j.timer ∧ i.ifAccess = j.ifAccess ∧ i.dma = j.dma ∧ i.rises = j.rises ∧
    i.lcd = j.lcd ∧ interrupts.check.same_stat i.stat j.stat = ok true ∧
    i.mode3End = j.mode3End

/-! The timer: the read and the interrupt (the netlist's at the A edge;
ceres's T-cycle, and whether a second came). -/

def netTimer (n : NetTimer) (i : NetIn) : Result ((Option Std.U8 × Bool) × NetTimer) :=
  n.m_cycle i.timer

def ceresTimer (c : CeresTimer) (j : CeresIn) :
    Result ((Option Std.U8 × Option Std.U8 × Bool) × CeresTimer) := do
  let host ← timing.check.CheckHost.new j.mask
  let (o, c', _) ← timing.check.ceres_m_cycle c host j.timer
  ok (o, c')

def TimerOut (o : Option Std.U8 × Bool) (p : Option Std.U8 × Option Std.U8 × Bool) : Prop :=
  o.1 = p.1 ∧ timing.check.same_interrupt o.2 p.2.1 = ok true ∧ p.2.2 = false

def TimerK (n : NetTimer) (i : NetIn) : Prop :=
  timing.check.known_difference n i.timer = ok true

/-! IF: its access, the other sources' rises, the STAT line's and the
timer's interrupt; the read and the interrupt taken. -/

def netIf (n : NetIf) (x : NetIn × (Option Std.U8 × Bool)) :
    Result ((Option Std.U8 × Option Std.U8) × NetIf) :=
  interrupts.check.net_step n x.1.ifAccess x.1.rises x.2.2 x.1.stat

def ceresIf (c : CeresIf) (x : CeresIn × (Option Std.U8 × Option Std.U8 × Bool)) :
    Result ((Option Std.U8 × Option Std.U8) × CeresIf) :=
  interrupts.check.ceres_step c x.1.ifAccess x.1.rises x.2.2.1 x.1.stat

/-! Ceres's M-cycle of the two, as it runs them: the timer's read, IF's
read and the interrupt taken. -/

def ceresJoint (s : CeresTimer × CeresIf) (j : CeresIn) :
    Result ((Option Std.U8 × Option Std.U8 × Option Std.U8) × (CeresTimer × CeresIf)) := do
  let host ← check.JointHost.new j.mask s.2
  let (o, c', h') ← check.ceres_m_cycle_joint s.1 host j.timer j.ifAccess j.rises j.stat
  ok (o, (c', h'.ints))

/-- The pair's outputs alike: the timer's read, IF's read, the interrupt
taken (the timer's interrupt itself goes into IF). -/
def JointOut (o : (Option Std.U8 × Bool) × (Option Std.U8 × Option Std.U8))
    (p : Option Std.U8 × Option Std.U8 × Option Std.U8) : Prop :=
  o.1.1 = p.1 ∧ o.2.1 = p.2.1 ∧ o.2.2 = p.2.2

/-- What Kani proves. -/
def KaniTimer : Prop :=
  ∀ net clock access mask, timing.check.step_agrees net clock access mask = ok true

def KaniIf : Prop :=
  ∀ net ints access rises nt ct ns cs,
    interrupts.check.step_agrees net ints access rises nt ct ns cs = ok true

def KaniSeq : Prop :=
  ∀ net clock clock' ints ints' ta mask ia rises stat,
    check.in_sequence net clock clock' ints ints' ta mask ia rises stat = ok true

theorem same_read_true {a b : Option Std.U8}
    (h : timing.check.same_read a b = ok true) : a = b := by
  unfold timing.check.same_read at h
  rcases a with _ | x <;> rcases b with _ | y <;> simp_all

theorem same_clock_refl (c : CeresTimer) : timing.check.same_clock c c = ok true := by
  unfold timing.check.same_clock
  rcases c.apu_late_bit with _ | x <;> simp

theorem same_clock_eq {a b : CeresTimer}
    (h : timing.check.same_clock a b = ok true) : a = b := by
  unfold timing.check.same_clock at h
  obtain ⟨l, hl, h⟩ := bind_eq_ok h
  simp only [Result.ok.injEq, Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨⟨⟨⟨⟨⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩, h8⟩, h9⟩, h10⟩, h11⟩, h12⟩, h13⟩ := h
  subst h12
  have hlb : a.apu_late_bit = b.apu_late_bit := by
    rcases ha : a.apu_late_bit with _ | x <;> rcases hb : b.apu_late_bit with _ | y <;>
      simp_all
  cases a
  cases b
  simp_all

theorem same_ints_refl (c : CeresIf) : interrupts.check.same_ints c c = ok true := by
  simp [interrupts.check.same_ints]

theorem same_ints_eq {a b : CeresIf}
    (h : interrupts.check.same_ints a b = ok true) : a = b := by
  simp only [interrupts.check.same_ints, Result.ok.injEq, Bool.and_eq_true,
    decide_eq_true_eq] at h
  cases a
  cases b
  simp_all

/-- The timer's M-cycle, from every related pair (Kani's step). -/
-- @assumes timer-step-agrees
theorem timer_steps (kani : KaniTimer) :
    Steps netTimer ceresTimer SameIn TimerOut TimerK
      (fun n c => timing.check.related n c = ok true) := by
  intro n c h i j hij
  obtain ⟨ht, -, -, -⟩ := hij
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
    obtain ⟨host, hh, k⟩ := bind_eq_ok k
    obtain ⟨⟨⟨nr, ni⟩, n'⟩, hn, k⟩ := bind_eq_ok k
    obtain ⟨⟨⟨cr, ct, twice⟩, c', host'⟩, hc, k⟩ := bind_eq_ok k
    obtain ⟨s, hs, k⟩ := bind_eq_ok k
    obtain ⟨si, hsi, k⟩ := bind_eq_ok k
    obtain ⟨r, hr, k⟩ := bind_eq_ok k
    simp only [Result.ok.injEq, Bool.and_eq_true, Bool.not_eq_true', Bool.decide_eq_true] at k
    obtain ⟨⟨⟨hs', hsi'⟩, htw⟩, hr'⟩ := k
    subst hs' hsi' htw hr'
    refine ⟨(nr, ni), n', (cr, ct, false), c', hn, ?_, ⟨same_read_true hs, hsi, rfl⟩, hr⟩
    simp [ceresTimer, ← ht, hh, hc]
  · left
    exact hd

/-- IF's M-cycle, from every related pair, with the timer's interrupt as
the timer's step gives it on both sides (Kani's step). -/
-- @assumes if-step-agrees
theorem if_steps (kani : KaniIf) :
    Steps netIf ceresIf (fun x y => SameIn x.1 y.1 ∧ TimerOut x.2 y.2) (· = ·)
      (fun _ _ => False) (fun n c => interrupts.check.related n c = ok true) := by
  intro n c h x y hxy
  right
  obtain ⟨i, o⟩ := x
  obtain ⟨j, p⟩ := y
  obtain ⟨⟨-, ha, -, hr, -, hst, -⟩, -, hsi, -⟩ := hxy
  have k := kani n c i.ifAccess i.rises o.2 p.2.1 i.stat j.stat
  unfold interrupts.check.step_agrees at k
  obtain ⟨b, hb, k⟩ := bind_eq_ok k
  rw [h, Result.ok.injEq] at hb
  subst hb
  obtain ⟨b1, hb1, k⟩ := bind_eq_ok k
  rw [hsi, Result.ok.injEq] at hb1
  subst hb1
  obtain ⟨b2, hb2, k⟩ := bind_eq_ok k
  rw [hst, Result.ok.injEq] at hb2
  subst hb2
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

/-- Ceres's M-cycle of the two is the timer's and then IF's. -/
-- @assumes blocks-in-sequence
theorem joint_in_sequence (kani : KaniSeq) (n : NetTimer) (c c' : CeresTimer)
    (cif cif' : CeresIf) (j : CeresIn) (p : Option Std.U8 × Option Std.U8 × Bool)
    (p2 : Option Std.U8 × Option Std.U8)
    (h : timing.check.related n c = ok true) (hc : ceresTimer c j = ok (p, c'))
    (hc2 : ceresIf cif (j, p) = ok (p2, cif')) (htw : p.2.2 = false) :
    ceresJoint (c, cif) j = ok ((p.1, p2.1, p2.2), (c', cif')) := by
  obtain ⟨r1, r2, r3⟩ := p
  obtain ⟨q1, q2⟩ := p2
  simp only at htw
  subst htw
  have k := kani n c c cif cif j.timer j.mask j.ifAccess j.rises j.stat
  unfold check.in_sequence at k
  obtain ⟨b, hb, k⟩ := bind_eq_ok k
  rw [h, Result.ok.injEq] at hb
  subst hb
  obtain ⟨b1, hb1, k⟩ := bind_eq_ok k
  rw [same_clock_refl, Result.ok.injEq] at hb1
  subst hb1
  obtain ⟨b2, hb2, k⟩ := bind_eq_ok k
  rw [same_ints_refl, Result.ok.injEq] at hb2
  subst hb2
  simp only [Bool.false_eq_true, ↓reduceIte, decide_false, not_true_eq_false,
    Bool.or_false] at k
  -- The timer's step and IF's, as `hc` and `hc2` give them.
  simp only [ceresTimer] at hc
  obtain ⟨host, hh, hc⟩ := bind_eq_ok hc
  obtain ⟨⟨p', c'', host'⟩, hm, hc⟩ := bind_eq_ok hc
  simp at hc
  obtain ⟨rfl, rfl⟩ := hc
  obtain ⟨host1, hh1, k⟩ := bind_eq_ok k
  rw [hh, Result.ok.injEq] at hh1
  subst hh1
  obtain ⟨x, hx, k⟩ := bind_eq_ok k
  rw [hm, Result.ok.injEq] at hx
  subst hx
  obtain ⟨x2, hx2, k⟩ := bind_eq_ok k
  simp only [ceresIf] at hc2
  rw [hc2, Result.ok.injEq] at hx2
  subst hx2
  -- The joint M-cycle.
  obtain ⟨jh, hjh, k⟩ := bind_eq_ok k
  obtain ⟨⟨⟨jr, ji, jt⟩, cj, hj⟩, hjm, k⟩ := bind_eq_ok k
  obtain ⟨s3, hs3, k⟩ := bind_eq_ok k
  obtain ⟨s4, hs4, k⟩ := bind_eq_ok k
  obtain ⟨s5, hs5, k⟩ := bind_eq_ok k
  obtain ⟨s6, hs6, k⟩ := bind_eq_ok k
  obtain ⟨s7, hs7, k⟩ := bind_eq_ok k
  simp only [Result.ok.injEq, Bool.false_or, Bool.and_eq_true] at k
  obtain ⟨⟨⟨⟨e3, e4⟩, e5⟩, e6⟩, e7⟩ := k
  subst e3 e4 e5 e6 e7
  rw [same_read_true hs3, same_read_true hs4, same_read_true hs5, same_clock_eq hs6,
    same_ints_eq hs7]
  simp [ceresJoint, hjh, hjm]

/-- The two blocks wired together, as ceres runs them, T-cycle by T-cycle:
from every related pair, the M-cycle gives the same timer read, IF read
and interrupt taken, and related states, but for the timer's known
differences. -/
-- @assumes timer-step-agrees
-- @assumes if-step-agrees
-- @assumes blocks-in-sequence
theorem timer_and_if_steps (kt : KaniTimer) (ki : KaniIf) (ks : KaniSeq) :
    Steps (wire netTimer netIf) ceresJoint SameIn JointOut
      (wireK netTimer TimerK (fun _ _ => False))
      (fun n c => timing.check.related n.1 c.1 = ok true ∧
        interrupts.check.related n.2 c.2 = ok true) := by
  refine ((timer_steps kt).wire (if_steps ki)).map_cer (fun p => (p.1.1, p.2.1, p.2.2)) ?_ ?_
  · rintro ⟨n, nif⟩ ⟨c, cif⟩ i j o ⟨p, p2⟩ ⟨c', cif'⟩ ⟨h, -⟩ - ⟨⟨-, -, htw⟩, -⟩ hc
    obtain ⟨hc1, hc2⟩ := wire_eq_ok hc
    exact joint_in_sequence ks n c c' cif cif' j p p2 h hc1 hc2 htw
  · rintro o ⟨p, p2⟩ ⟨⟨h1, -, -⟩, rfl⟩
    exact ⟨h1, rfl, rfl⟩

/-- ceres's timer and IF agree with the netlist's from any related states,
as ceres runs them, T-cycle by T-cycle: every M-cycle gives the same timer
read, the same IF read and the same interrupt taken, unless it is one of
the timer's known differences. -/
-- @assumes timer-step-agrees
-- @assumes if-step-agrees
-- @assumes blocks-in-sequence
theorem timer_and_if_agree (kt : KaniTimer) (ki : KaniIf) (ks : KaniSeq)
    (n : NetTimer) (c : CeresTimer) (nif : NetIf) (cif : CeresIf)
    (h : timing.check.related n c = ok true)
    (hif : interrupts.check.related nif cif = ok true) :
    Agree (wire netTimer netIf) ceresJoint SameIn JointOut
      (wireK netTimer TimerK (fun _ _ => False)) (n, nif) (c, cif) :=
  (timer_and_if_steps kt ki ks).agree (n := (n, nif)) (c := (c, cif)) ⟨h, hif⟩

end ceres_core.timer_if
