/-
The hand-written netlist timer (`Net`) is Aeneas's translation of
`ceres_netlist::timer::NetTimer`, function by function: what ties the step
proved on the hand models (`Step.lean`) to the Rust that Kani checks.
-/
import Std.Tactic.BVDecide
import Ceres.Gen.Funs
import Pilot.Timer.Models
open Aeneas Aeneas.Std Result ceres_core

namespace Pilot.Timer

abbrev NetTimer := ceres_netlist.timer.NetTimer

theorem bne_bv {ty} (a b : UScalar ty) : (a != b) = (a.bv != b.bv) := by
  cases a; cases b; rfl

theorem eq_bv {ty} (a b : UScalar ty) : (a = b) = (a.bv = b.bv) := by
  cases a; cases b; simp

/-! A branch between whole values is one `bv_decide` cannot see into: these
push it into `ok`, pairs and the timer, down to the fields. -/

theorem cond_ok {α} (c : Bool) (a b : α) :
    (bif c then (ok a : Result α) else ok b) = ok (bif c then a else b) := by
  cases c <;> rfl

theorem ite_ok {α} (c : Prop) [Decidable c] (a b : α) :
    (if c then (ok a : Result α) else ok b) = ok (if c then a else b) := by
  split <;> rfl

theorem cond_pair {α β} (c : Bool) (a b : α) (x y : β) :
    (bif c then (a, x) else (b, y)) = (bif c then a else b, bif c then x else y) := by
  cases c <;> rfl

theorem ite_pair {α β} (c : Prop) [Decidable c] (a b : α) (x y : β) :
    (if c then (a, x) else (b, y)) = (if c then a else b, if c then x else y) := by
  split <;> rfl

theorem cond_bv {ty} (c : Bool) (a b : UScalar ty) :
    (bif c then a else b).bv = bif c then a.bv else b.bv := by
  cases c <;> rfl

theorem ite_bv {ty} (c : Prop) [Decidable c] (a b : UScalar ty) :
    (if c then a else b).bv = if c then a.bv else b.bv := by
  split <;> rfl

theorem cond_timer (c : Bool) (a b : NetTimer) :
    (bif c then a else b) =
      ⟨bif c then a.count else b.count, bif c then a.tima else b.tima,
        bif c then a.tma else b.tma, bif c then a.tac else b.tac,
        bif c then a.int_timer else b.int_timer, bif c then a.bit7_before else b.bit7_before,
        bif c then a.timer_clock else b.timer_clock, bif c then a.write_tma else b.write_tma,
        bif c then a.write_tac else b.write_tac, bif c then a.latched_data else b.latched_data,
        bif c then a.enable_race_counts else b.enable_race_counts⟩ := by
  cases c <;> rfl

theorem ite_timer (c : Prop) [Decidable c] (a b : NetTimer) :
    (if c then a else b) =
      ⟨if c then a.count else b.count, if c then a.tima else b.tima,
        if c then a.tma else b.tma, if c then a.tac else b.tac,
        if c then a.int_timer else b.int_timer, if c then a.bit7_before else b.bit7_before,
        if c then a.timer_clock else b.timer_clock, if c then a.write_tma else b.write_tma,
        if c then a.write_tac else b.write_tac, if c then a.latched_data else b.latched_data,
        if c then a.enable_race_counts else b.enable_race_counts⟩ := by
  split <;> rfl

theorem one_u8_bv : (1#u8).bv = 1#8 := rfl

/-! The conversions at `U8` and `U16`, whose `bv` has a numeral width (the
generic one's, `ty.numBits`, is one `bv_decide` does not take). -/

theorem eq8 (a b : U8) : (a = b) = (a.bv = b.bv) := by
  cases a; cases b; simp []

theorem eq16 (a b : U16) : (a = b) = (a.bv = b.bv) := by
  cases a; cases b; simp []

theorem bne8 (a b : U8) : (a != b) = (a.bv != b.bv) := by
  cases a; cases b; rfl

theorem bne16 (a b : U16) : (a != b) = (a.bv != b.bv) := by
  cases a; cases b; rfl

theorem lit8 (n : Nat) (h) : (UScalar.ofNatCore n h : U8).bv = BitVec.ofNat 8 n := by
  apply BitVec.eq_of_toNat_eq
  simp only [UScalar.ofNatCore]
  exact (Nat.mod_eq_of_lt h).symm

theorem lit16 (n : Nat) (h) : (UScalar.ofNatCore n h : U16).bv = BitVec.ofNat 16 n := by
  apply BitVec.eq_of_toNat_eq
  simp only [UScalar.ofNatCore]
  exact (Nat.mod_eq_of_lt h).symm

theorem and8 (a b : U8) : (a &&& b).bv = a.bv &&& b.bv := rfl

theorem bv8 (x : BitVec UScalarTy.U8.numBits) : U8.bv ⟨x⟩ = x := rfl

theorem bv16 (x : BitVec UScalarTy.U16.numBits) : U16.bv ⟨x⟩ = x := rfl

theorem cond8 (c : Bool) (a b : U8) : (bif c then a else b).bv = bif c then a.bv else b.bv := by
  cases c <;> rfl

theorem ite8 (c : Prop) [Decidable c] (a b : U8) :
    (if c then a else b).bv = if c then a.bv else b.bv := by
  split <;> rfl

theorem cond16 (c : Bool) (a b : U16) : (bif c then a else b).bv = bif c then a.bv else b.bv := by
  cases c <;> rfl

theorem ite16 (c : Prop) [Decidable c] (a b : U16) :
    (if c then a else b).bv = if c then a.bv else b.bv := by
  split <;> rfl

/-- The translation's state as the hand model's. -/
def ofA (a : NetTimer) : Net :=
  ⟨a.count.bv, a.tima.bv, a.tma.bv, a.tac.bv, a.int_timer, a.bit7_before, a.timer_clock,
    a.write_tma, a.write_tac, a.latched_data.bv, a.enable_race_counts⟩

/-- The hand model's state as the translation's. -/
def toA (n : Net) : NetTimer :=
  ⟨⟨n.count⟩, ⟨n.tima⟩, ⟨n.tma⟩, ⟨n.tac⟩, n.intTimer, n.bit7Before, n.timerClock, n.writeTma,
    n.writeTac, ⟨n.latched⟩, n.raceCounts⟩

theorem toA_ofA (a : NetTimer) : toA (ofA a) = a := rfl

theorem clock_of (tac sel : Std.U8) (count : Std.U16) :
    ceres_netlist.timer.NetTimer.clock_of tac sel count = ok (clockOf tac.bv sel.bv count.bv) := by
  unfold ceres_netlist.timer.NetTimer.clock_of clockOf
  simp [lift, ceres_netlist.timer.TAC_CLOCK, ceres_netlist.timer.TAC_ENABLE_B]
  split <;> rename_i h <;>
    simp only [HShiftLeft.hShiftLeft, UScalar.shiftLeft_IScalar, UScalar.shiftLeft] <;>
    simp [bne_bv, eq_bv] at * <;> bv_decide

/-- What the link lemmas unfold: the translation's plumbing, down to fields
and bit vectors. -/
macro "link_simp" : tactic => `(tactic|
  simp only [ofA, toA, lift, bind_ok, bind_tc_ok, ite_ok, cond_ok, ite_pair, cond_pair,
    ite_timer, cond_timer, Result.ok.injEq, Prod.mk.injEq,
    ceres_netlist.timer.NetTimer.mk.injEq, eq8, eq16, bne8, bne16, lit8, lit16, and8, bv8, bv16, cond8, ite8, cond16, ite16,
    core.num.U8.wrapping_add_bv_eq, core.num.U16.wrapping_add_bv_eq, one_u8_bv,
    Net.settle, Net.phase, Net.edgeA, Net.pick, Writes.nothing, clockOf,
    ↓reduceIte, ite_true, ite_false, Bool.cond_true, Bool.cond_false, Bool.false_eq_true,
    core.option.Option.is_some, Option.isSome_some, Option.isSome_none,
    Bool.true_and, Bool.and_true, Bool.false_and, Bool.and_false, Bool.true_or, Bool.or_true,
    Bool.false_or, Bool.or_false, Bool.not_true, Bool.not_false, decide_true, decide_false,
    not_true_eq_false, not_false_eq_true])

theorem settle (a : NetTimer) (writeTima t4 : Bool) (data : Std.U8) (clock : Bool) :
    ceres_netlist.timer.NetTimer.settle a writeTima t4 data clock
      = ok (toA ((ofA a).settle writeTima t4 data.bv clock)) := by
  obtain ⟨⟨count⟩, ⟨tima⟩, ⟨tma⟩, ⟨tac⟩, intTimer, bit7Before, timerClock, writeTma,
    writeTac, ⟨latched⟩, raceCounts⟩ := a
  obtain ⟨data⟩ := data
  -- The widths as numerals, which `bv_decide` needs.
  revert count tima tma tac latched data
  refine fun (count : BitVec 16) (tima tma tac latched data : BitVec 8) => ?_
  unfold ceres_netlist.timer.NetTimer.settle ceres_netlist.timer.NetTimer.load
  link_simp
  bv_decide

theorem one_u16_bv : (1#u16).bv = 1#16 := rfl

theorem edge_a (a : NetTimer) :
    ceres_netlist.timer.NetTimer.edge_a a = ok (toA (ofA a).edgeA) := by
  obtain ⟨⟨count⟩, ⟨tima⟩, ⟨tma⟩, ⟨tac⟩, intTimer, bit7Before, timerClock, writeTma,
    writeTac, ⟨latched⟩, raceCounts⟩ := a
  revert count tima tma tac latched
  refine fun (count : BitVec 16) (tima tma tac latched : BitVec 8) => ?_
  unfold ceres_netlist.timer.NetTimer.edge_a ceres_netlist.timer.NetTimer.load
  simp only [clock_of, settle]
  link_simp
  bv_decide

/-- The access, on the hand model. -/
def accOf : ceres_netlist.timer.Access → Acc
  | .None => .none
  | .Read .Div => .read .div
  | .Read .Tima => .read .tima
  | .Read .Tma => .read .tma
  | .Read .Tac => .read .tac
  | .Write .Div v => .write .div v.bv
  | .Write .Tima v => .write .tima v.bv
  | .Write .Tma v => .write .tma v.bv
  | .Write .Tac v => .write .tac v.bv

/-- What a phase but A writes: the access's write in E, F and G. -/
def writesIn : ceres_netlist.clocks.Phase → ceres_netlist.timer.Access → Writes
  | .E, acc | .F, acc | .G, acc => (accOf acc).writes
  | _, _ => .nothing

/-- The write phase: G and H. -/
def t4In : ceres_netlist.clocks.Phase → Bool
  | .G | .H => true
  | _ => false

/-- Aeneas's own simp set rewrites widths (`UScalarTy.U8_numBits_eq`, the
`bv_mk` simproc), which left a `U8` in a `U16` field: the translation's
plumbing is taken apart with these alone. -/
macro "plumbing_simp" : tactic => `(tactic|
  simp only [ite_ok, ite_pair, bind_ok, uncurry_apply_pair, ↓reduceIte, ite_timer, ite_self,
    Bool.not_eq_true, Bool.decide_eq_false, Bool.and_eq_true, Bool.not_eq_eq_eq_not,
    Bool.not_true, lift, clock_of, Bool.or_eq_true, settle, core.option.Option.is_some,
    Bool.false_eq_true, and_true, Bool.ite_false_right, Bool.decide_or, Bool.decide_eq_true,
    Bool.decide_and, Bool.not_false, Bool.and_true, Option.isSome_some, Option.isSome_none,
    writesIn, Acc.writes, accOf, t4In, ok.injEq])

set_option maxHeartbeats 0 in
theorem phase (a : NetTimer) (p : ceres_netlist.clocks.Phase)
    (acc : ceres_netlist.timer.Access) (h : p ≠ .A) :
    ceres_netlist.timer.NetTimer.phase a p acc
      = ok (toA ((ofA a).phase (writesIn p acc) (t4In p))) := by
  obtain ⟨⟨count⟩, ⟨tima⟩, ⟨tma⟩, ⟨tac⟩, intTimer, bit7Before, timerClock, writeTma,
    writeTac, ⟨latched⟩, raceCounts⟩ := a
  unfold ceres_netlist.timer.NetTimer.phase
  cases p <;> simp only [ne_eq, not_true_eq_false, reduceCtorEq, not_false_eq_true] at h <;>
    rcases acc with _ | r | ⟨r, ⟨v⟩⟩ <;> (try cases r) <;>
    plumbing_simp <;> link_simp <;>
    revert count tima tma tac latched <;>
    refine fun (count : BitVec 16) (tima tma tac latched : BitVec 8) => ?_ <;>
    bv_decide

theorem ofA_toA (n : Net) : ofA (toA n) = n := rfl

theorem toA_int_timer (n : Net) : (toA n).int_timer = n.intTimer := rfl

/-- The register, on the hand model. -/
def regOf : ceres_netlist.timer.Reg → Reg
  | .Div => .div
  | .Tima => .tima
  | .Tma => .tma
  | .Tac => .tac

theorem read (a : NetTimer) (r : ceres_netlist.timer.Reg) :
    ceres_netlist.timer.NetTimer.read a r = ok ⟨(ofA a).read (regOf r)⟩ := by
  obtain ⟨⟨count⟩, ⟨tima⟩, ⟨tma⟩, ⟨tac⟩, intTimer, bit7Before, timerClock, writeTma,
    writeTac, ⟨latched⟩, raceCounts⟩ := a
  cases r <;> simp only [ceres_netlist.timer.NetTimer.read, regOf, Net.read, ofA, bv8, bv16]
  simp only [HShiftRight.hShiftRight, UScalar.shiftRight_IScalar, UScalar.shiftRight]
  rw [ite_eq_left (by decide), ite_eq_left (by decide), show (6#i32).toNat = 6 by decide]
  simp only [bind_ok, UScalar.cast]
  rfl

/-- The read as the translation gives it. -/
def readOf (r : Bool × BitVec 8) : Option U8 :=
  bif r.1 then some ⟨r.2⟩ else none

/-- `NetTimer::m_cycle` is the hand model's M-cycle. -/
theorem m_cycle (a : NetTimer) (acc : ceres_netlist.timer.Access) :
    ceres_netlist.timer.NetTimer.m_cycle a acc =
      let (read, rose, n) := (ofA a).mCycle (accOf acc)
      ok ((readOf read, rose), toA n) := by
  unfold ceres_netlist.timer.NetTimer.m_cycle
  simp only [phase, edge_a, bind_ok, ofA_toA, ne_eq, reduceCtorEq, not_false_eq_true]
  rcases acc with _ | r | ⟨r, v⟩ <;> (try cases r) <;>
    simp only [read, bind_ok, writesIn, t4In, accOf, Net.mCycle, Acc.writes, regOf, readOf,
      ofA_toA, toA_int_timer, Bool.cond_false, Bool.cond_true] <;> simp

end Pilot.Timer
