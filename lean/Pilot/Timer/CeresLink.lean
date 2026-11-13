/-
The hand-written ceres timer (`Cer`) is Aeneas's translation of ceres's
`Clock` as `timing::check` runs it, function by function, on the clocks the
step relates: no DIV-reset delay (`div_cycles = 0`), not stopped, no APU
delay, no late APU bit (`Pinned`), and a reload countdown of at most 8, which
keeps the translation's checked arithmetic from failing where the hand model
wraps (`Cer` keeps the bound).
-/
import Std.Tactic.BVDecide
import Ceres.Gen.Funs
import Pilot.Timer.NetLink
open Aeneas Aeneas.Std Result ceres_core

namespace Pilot.Timer

abbrev Clock := timing.timer.Clock
abbrev Host := timing.check.CheckHost
abbrev inst := timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost

/-- The clock as the hand model sees it. -/
def ofC (c : Clock) : Cer :=
  ⟨c.div.bv, c.tac.bv, c.tima.bv, c.tma.bv, c.tima_reload_pending.bv, c.tima_irq_countdown.bv⟩

/-- `c` with the hand model's fields written into it, the others kept. -/
def withC (c : Clock) (x : Cer) : Clock :=
  { c with div := ⟨x.div⟩, tac := ⟨x.tac⟩, tima := ⟨x.tima⟩, tma := ⟨x.tma⟩,
           tima_reload_pending := ⟨x.reload⟩, tima_irq_countdown := ⟨x.irq⟩ }

theorem ite_clock (c : Prop) [Decidable c] (a b : Clock) :
    (if c then a else b) =
      ⟨if c then a.div else b.div, if c then a.tac else b.tac, if c then a.tima else b.tima,
        if c then a.tma else b.tma, if c then a.tima_reload_pending else b.tima_reload_pending,
        if c then a.tima_irq_countdown else b.tima_irq_countdown,
        if c then a.div_cycles else b.div_cycles, if c then a.div_state else b.div_state,
        if c then a.stopped else b.stopped, if c then a.during_div_write else b.during_div_write,
        if c then a.apu_delay else b.apu_delay, if c then a.apu_late_bit else b.apu_late_bit,
        if c then a.apu_odd else b.apu_odd⟩ := by
  split <;> rfl

theorem sys_clk_tac_mux (tac : Std.U8) :
    timing.timer.Clock.sys_clk_tac_mux tac = ok ⟨mux tac.bv⟩ := by
  unfold timing.timer.Clock.sys_clk_tac_mux mux
  simp only [lift, bind_ok, timing.timer.TAC_CLOCK]
  split <;> rename_i h <;>
    simp only [HShiftLeft.hShiftLeft, UScalar.shiftLeft_IScalar, UScalar.shiftLeft] <;>
    rw [ite_eq_left (by decide), ite_eq_left (by decide)] <;>
    simp only [eq8, and8, lit8, lit16, bv8, ok.injEq, UScalar.mk.injEq,
      show (9#i32).toNat = 9 by decide, show (3#i32).toNat = 3 by decide,
      show (5#i32).toNat = 5 by decide, show (7#i32).toNat = 7 by decide] at * <;>
    bv_decide

/-- The check host's hooks: all but the interrupt's do nothing. -/
macro "host_simp" : tactic => `(tactic|
  simp only [inst, timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.is_cgb_mode,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.is_cgb_hardware,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.double_speed,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.serial_div_mask,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.serial_clock_edge,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.apu_m_cycle,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.apu_delayed_envelope_tick,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.apu_envelopes_armed,
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.apu_frame_sequencer_step,
    timing.timer.Clock.is_tac_enabled, timing.timer.TAC_ENABLE_B, timing.apu_div_bit,
    sys_clk_tac_mux])

/-- From the hand model's side: fields, bit vectors and its functions. -/
macro "cer_simp" : tactic => `(tactic|
  simp only [ofC, withC, lift, bind_ok, ite_ok, ite_pair, ite_clock, ↓reduceIte, ite_self,
    ok.injEq, Prod.mk.injEq, timing.timer.Clock.mk.injEq, eq8, eq16, bne8, bne16, lit8, lit16,
    and8, bv8, bv16, ite8, ite16, cond8, cond16, core.num.U8.wrapping_add_bv_eq,
    Cer.incTima, Cer.setDiv, Cer.pick, mux, Bool.false_eq_true, and_true, true_and,
    and_self, Bool.decide_eq_true, decide_not, Bool.not_eq_true])

theorem inc_tima (c : Clock) (host : Host) :
    timing.timer.Clock.inc_tima inst c host = ok (withC c (ofC c).incTima) := by
  obtain ⟨⟨div⟩, ⟨tac⟩, ⟨tima⟩, ⟨tma⟩, ⟨reload⟩, ⟨irq⟩, divCycles, divState, stopped, dw,
    apuDelay, late, odd⟩ := c
  unfold timing.timer.Clock.inc_tima
  host_simp
  cer_simp
  revert div tac tima tma reload irq
  refine fun (div : BitVec 16) (tac tima tma reload irq : BitVec 8) => ?_
  bv_decide

/-- The timer's edges of a DIV write: `Cer.setDiv` before it stores DIV. -/
def edges (c : Cer) (val : BitVec 16) : Cer :=
  Cer.pick ((c.tac &&& 4#8 != 0#8) && (c.div &&& ~~~val &&& mux c.tac != 0#16)) c.incTima c

theorem not16 (a : U16) : (~~~a).bv = ~~~a.bv := rfl

theorem and16 (a b : U16) : (a &&& b).bv = a.bv &&& b.bv := rfl

theorem timer_edges (c : Clock) (host : Host) (val : Std.U16) :
    timing.timer.Clock.timer_edges inst c host val = ok (withC c (edges (ofC c) val.bv), host) := by
  obtain ⟨⟨div⟩, ⟨tac⟩, ⟨tima⟩, ⟨tma⟩, ⟨reload⟩, ⟨irq⟩, divCycles, divState, stopped, dw,
    apuDelay, late, odd⟩ := c
  obtain ⟨val⟩ := val
  unfold timing.timer.Clock.timer_edges
  host_simp
  simp only [inc_tima, edges]
  cer_simp
  simp only [not16, and16, bv16]
  revert div tac tima tma reload irq val
  refine fun (div : BitVec 16) (tac tima tma reload irq : BitVec 8) (val : BitVec 16) => ?_
  bv_decide

/-- `Clock::set_div` on the check host: whatever the APU's state, every
branch stores DIV and takes the late bit, and the APU's hooks do nothing. -/
theorem set_div (c : Clock) (host : Host) (val : Std.U16) :
    timing.timer.Clock.set_div inst c host val
      = ok ({ withC c ((ofC c).setDiv val.bv) with apu_late_bit := none }, host) := by
  unfold timing.timer.Clock.set_div
  host_simp
  simp only [timer_edges, bind_ok]
  rcases h : c.apu_late_bit with _ | late <;>
    simp only [withC, core.option.Option.take, h, ite_self, bind_ok, Bool.false_eq_true,
      ↓reduceIte, uncurry_apply_pair, lift] <;>
    rfl

/-! Checked arithmetic, where it cannot fail, as bit vectors. -/
theorem sub8 (x y : U8) (h : y.val ≤ x.val) : x - y = ok ⟨x.bv - y.bv⟩ := by
  simp only [HSub.hSub, Sub.sub, UScalar.sub]
  rw [ite_eq_right (by omega)]
  congr 2
  apply BitVec.eq_of_toNat_eq
  have := x.bv.isLt; have := y.bv.isLt
  simp only [UScalar.val] at *
  simp [BitVec.toNat_sub, Nat.sub_eq, BitVec.sub_eq] at *
  omega

theorem add8 (x y : U8) (h : x.val + y.val < 256) : x + y = ok ⟨x.bv + y.bv⟩ := by
  simp only [HAdd.hAdd, Add.add, UScalar.add, UScalar.tryMk, UScalar.tryMkOpt]
  rw [dite_eq_left (by simp [UScalar.check_bounds, UScalarTy.numBits] at *; omega)]
  simp only [Result.ofOption]
  congr 2
  have := x.bv.isLt; have := y.bv.isLt
  simp only [UScalar.val] at *
  simp [] at *
  omega

theorem add32 (x y : U32) (h : x.val + y.val < 4294967296) : x + y = ok ⟨x.bv + y.bv⟩ := by
  simp only [HAdd.hAdd, Add.add, UScalar.add, UScalar.tryMk, UScalar.tryMkOpt]
  rw [dite_eq_left (by simp [UScalar.check_bounds, UScalarTy.numBits] at *; omega)]
  simp only [Result.ofOption]
  congr 2
  have := x.bv.isLt; have := y.bv.isLt
  simp only [UScalar.val] at *
  simp [] at *
  omega

/-! The loop of `Clock::run` over one T-cycle: the range `0..1` yields once. -/
theorem next0 : core.iter.range.IteratorRange.next core.iter.range.StepI32
    { start := 0#i32, «end» := 1#i32 } = ok (some 0#i32, { start := 1#i32, «end» := 1#i32 }) := by
  simp only [core.iter.range.IteratorRange.next, core.iter.range.StepI32,
    core.iter.range.IScalarStep, core.iter.range.IScalarStep.forward_checked, core.clone.CloneI32,
    core.cmp.PartialOrdI32, liftFun1, liftFun2, bind_ok, core.clone.impls.CloneI32.clone]
  rw [ite_eq_left_of_eq_true _ _ (eq_true (by decide)), dite_eq_left_of_eq_true (eq_true (by scalar_tac)),
    bind_ok]
  rfl

theorem next1 : core.iter.range.IteratorRange.next core.iter.range.StepI32
    { start := 1#i32, «end» := 1#i32 } = ok (none, { start := 1#i32, «end» := 1#i32 }) := by
  simp only [core.iter.range.IteratorRange.next, core.iter.range.StepI32,
    core.iter.range.IScalarStep, core.cmp.PartialOrdI32, liftFun2, bind_ok]
  rw [ite_eq_right_of_eq_false _ _ (eq_false (by decide))]

/-- One step of the loop, by `loop`'s equation. -/
theorem run_loop_step (iter : core.ops.range.Range Std.I32) (c : Clock) (h : Host) :
    timing.timer.Clock.run_loop inst iter c h =
      Std.bind (timing.timer.Clock.run_loop.body inst iter c h) fun r =>
        match r with
        | .cont (i, c, h) => timing.timer.Clock.run_loop inst i c h
        | .done y => ok y := by
  unfold timing.timer.Clock.run_loop
  rw [loop]
  congr 1
  funext r
  rcases r with ⟨i, c, h⟩ | y <;> rfl

/-- The range spent, the loop is done. -/
theorem run_loop_last (c : Clock) (h : Host) :
    timing.timer.Clock.run_loop inst { start := 1#i32, «end» := 1#i32 } c h = ok (c, h) := by
  rw [run_loop_step]
  unfold timing.timer.Clock.run_loop.body
  rw [next1]
  simp only [bind_ok, uncurry_apply_pair]

/-! Checked arithmetic and comparisons, unconditionally, as bit vectors. -/

theorem lt8 (x y : U8) : (x < y) = (x.bv < y.bv) := by
  rw [BitVec.lt_def, U8.bv_toNat, U8.bv_toNat]; rfl

theorem le8 (x y : U8) : (x ≤ y) = (x.bv ≤ y.bv) := by
  rw [BitVec.le_def, U8.bv_toNat, U8.bv_toNat]; rfl

theorem sub8' (x y : U8) :
    x - y = if y.bv ≤ x.bv then ok ⟨x.bv - y.bv⟩ else fail .integerOverflow := by
  simp only [HSub.hSub, Sub.sub, UScalar.sub]
  have e : (y.bv ≤ x.bv) = (y.val ≤ x.val) := by rw [BitVec.le_def, U8.bv_toNat, U8.bv_toNat]
  by_cases h : y.val ≤ x.val
  · rw [ite_eq_right (by omega), ite_eq_left (e ▸ h)]
    congr 2
    apply BitVec.eq_of_toNat_eq
    have := x.bv.isLt; have := y.bv.isLt
    simp [BitVec.toNat_sub, Nat.sub_eq, BitVec.sub_eq] at *
    omega
  · rw [ite_eq_left (by omega), ite_eq_right (e ▸ h)]

theorem inc8 (x : U8) :
    x + 1#u8 = if x.bv < 255#8 then ok ⟨x.bv + 1#8⟩ else fail .integerOverflow := by
  have v1 : (1#u8).val = 1 := rfl
  have b1 : (1#u8).bv = 1#8 := rfl
  have e : (x.bv < 255#8) ↔ x.val < 255 := by
    rw [BitVec.lt_def, U8.bv_toNat]; simp
  by_cases h : x.bv < 255#8
  · rw [ite_eq_left h, add8 x 1#u8 (by rw [v1]; exact by have := e.mp h; omega)]
    simp only [b1]
  · rw [ite_eq_right h]
    simp only [HAdd.hAdd, Add.add, UScalar.add, UScalar.tryMk, UScalar.tryMkOpt]
    rw [dite_eq_right_of_eq_false (eq_false (by
      simp only [UScalar.check_bounds, UScalarTy.numBits, v1, decide_eq_true_eq,
        Nat.add_eq]
      have := e.not.mp h; omega))]
    rfl

theorem inc32 (x : U32) :
    x + 1#u32 = if x.bv < 0xFFFFFFFF#32 then ok ⟨x.bv + 1#32⟩ else fail .integerOverflow := by
  have v1 : (1#u32).val = 1 := rfl
  have b1 : (1#u32).bv = 1#32 := rfl
  have e : (x.bv < 0xFFFFFFFF#32) ↔ x.val < 0xFFFFFFFF := by
    rw [BitVec.lt_def, U32.bv_toNat]; simp
  by_cases h : x.bv < 0xFFFFFFFF#32
  · rw [ite_eq_left h, add32 x 1#u32 (by rw [v1]; exact by have := e.mp h; omega)]
    simp only [b1]
  · rw [ite_eq_right h]
    simp only [HAdd.hAdd, Add.add, UScalar.add, UScalar.tryMk, UScalar.tryMkOpt]
    rw [dite_eq_right_of_eq_false (eq_false (by
      simp only [UScalar.check_bounds, UScalarTy.numBits, v1, decide_eq_true_eq,
        Nat.add_eq]
      have := e.not.mp h; omega))]
    rfl

theorem bind_ite {α β} (c : Prop) [Decidable c] (a b : Result α) (f : α → Result β) :
    Std.bind (if c then a else b) f = if c then Std.bind a f else Std.bind b f := by
  split <;> rfl

theorem i32_lt_self : (0#i32 < 0#i32) = False := by decide

theorem cond_host (c : Bool) (a b : Host) :
    (bif c then a else b) =
      ⟨bif c then a.serial_div_mask else b.serial_div_mask,
        bif c then a.timer_interrupts else b.timer_interrupts⟩ := by
  cases c <;> rfl

theorem cond32 (c : Bool) (a b : U32) :
    (bif c then a else b).bv = bif c then a.bv else b.bv := by
  cases c <;> rfl

theorem eq32 (a b : U32) : (a = b) = (a.bv = b.bv) := by
  cases a; cases b; simp []

theorem bv32 (x : BitVec UScalarTy.U32.numBits) : U32.bv ⟨x⟩ = x := rfl

/-- A branch the context decides: for a discharger to take. -/
theorem ite_of_true {α} {c : Prop} [Decidable c] (h : c) (a b : α) : (if c then a else b) = a :=
  ite_eq_left h

theorem ite_cont {α β} (c : Prop) [Decidable c] (a b : α) :
    (if c then (ControlFlow.cont a : ControlFlow α β) else .cont b) = .cont (if c then a else b) := by
  split <;> rfl

theorem ite_host (c : Prop) [Decidable c] (a b : Host) :
    (if c then a else b) =
      ⟨if c then a.serial_div_mask else b.serial_div_mask,
        if c then a.timer_interrupts else b.timer_interrupts⟩ := by
  split <;> rfl

theorem ite32 (c : Prop) [Decidable c] (a b : U32) :
    (if c then a else b).bv = if c then a.bv else b.bv := by
  split <;> rfl

/-- The reload countdown of `Clock::run`'s body, at most 8: the hand model's. -/
theorem reload_step (r : U8) (iter : core.ops.range.Range Std.I32) (hr : r.bv ≤ 8#8) :
    (if r > 0#u8 then
      if r ≤ 4#u8 then do
        let i1 ← r - 1#u8
        let i2 ← if i1 = 0#u8 then ok 5#u8 else ok i1
        ok (i2, iter)
      else do
        let i1 ← r + 1#u8
        let i2 ← if i1 > 8#u8 then ok 0#u8 else ok i1
        ok (i2, iter)
    else ok (r, iter)) = ok (⟨Cer.reloadNext r.bv⟩, iter) := by
  simp only [sub8', inc8, lt8, le8, eq8, lit8, bv8, bind_ite, bind_ok, bind_fail, Cer.reloadNext]
  split_ifs <;> (try simp only [ok.injEq, Prod.mk.injEq, eq8, bv8, lit8, and_true]) <;>
    bv_decide

/-- The interrupt's countdown: the hand model's, and the host counts the
request. -/
theorem irq_step (i : U8) (host : Host) (hi : host.timer_interrupts.bv < 0xFFFFFFFF#32) :
    (if i > 0#u8 then do
      let i2 ← i - 1#u8
      let t ← if i2 = 0#u8 then inst.request_timer_interrupt host else ok host
      ok (i2, t)
    else ok (i, host)) =
      ok (⟨bif i.bv != 0#8 then i.bv - 1#8 else i.bv⟩,
        bif i.bv != 0#8 && i.bv - 1#8 == 0#8 then
          { host with timer_interrupts := ⟨host.timer_interrupts.bv + 1#32⟩ }
        else host) := by
  obtain ⟨mask, ti⟩ := host
  simp only [
    timing.check.CheckHost.Insts.Ceres_coreTimingTimerTimerHost.request_timer_interrupt,
    sub8', inc32, lt8, eq8, lit8, bv8, bind_ite, bind_ok, bind_fail] at *
  split_ifs <;>
    (try simp only [ok.injEq, Prod.mk.injEq, eq8, bv8, eq32, bv32, cond_host,
      cond32, timing.check.CheckHost.mk.injEq]) <;>
    bv_decide

/-- The loop's body on `0..1`: ceres's T-cycle. -/
theorem body_first (c : Clock) (host : Host) (hd : c.div_cycles = 0#i32)
    (hr : c.tima_reload_pending.bv ≤ 8#8) (hi : host.timer_interrupts.bv < 0xFFFFFFFF#32) :
    timing.timer.Clock.run_loop.body inst { start := 0#i32, «end» := 1#i32 } c host =
      ok (.cont ({ start := 1#i32, «end» := 1#i32 },
        { withC c (ofC c).tCycle.2 with apu_late_bit := none },
        bif (ofC c).tCycle.1 then { host with timer_interrupts := ⟨host.timer_interrupts.bv + 1#32⟩ }
        else host)) := by
  unfold timing.timer.Clock.run_loop.body
  rw [next0]
  simp only [bind_ok, hd, i32_lt_self, ↓reduceIte, uncurry_apply_pair]
  rw [reload_step _ _ hr, irq_step _ _ hi]
  host_simp
  simp only [bind_ok, uncurry_apply_pair, lift, set_div, ite_self]
  obtain ⟨mask, ti⟩ := host
  simp only [ok.injEq, ControlFlow.cont.injEq, Prod.mk.injEq, true_and,
    timing.timer.Clock.mk.injEq, timing.check.CheckHost.mk.injEq, cond_host, Cer.tCycle,
    Cer.reloadNext, core.num.U16.wrapping_add_bv_eq, lit16, eq32, cond32, bv32]
  cer_simp
  simp only [hd, and_true]
  bv_decide

/-- `Clock::run` for one T-cycle, from a clock the step relates: the hand
model's `tCycle`, and the host counts the interrupt if it requested it. -/
theorem run1 (c : Clock) (host : Host) (hs : c.stopped = false) (hd : c.div_cycles = 0#i32)
    (hr : c.tima_reload_pending.bv ≤ 8#8) (hi : host.timer_interrupts.bv < 0xFFFFFFFF#32) :
    timing.timer.Clock.run inst c host 1#i32 =
      ok ({ withC c (ofC c).tCycle.2 with apu_late_bit := none },
        bif (ofC c).tCycle.1 then { host with timer_interrupts := ⟨host.timer_interrupts.bv + 1#32⟩ }
        else host) := by
  unfold timing.timer.Clock.run
  simp only [hs, Bool.false_eq_true, ↓reduceIte]
  rw [run_loop_step, body_first _ _ hd hr hi]
  simp only [bind_ok]
  rw [run_loop_last]

/-- The first T-cycle with the interrupt, as the translation keeps it. -/
def optOf (f : Bool × BitVec 8) : Option U8 :=
  bif f.1 then some ⟨f.2⟩ else none

theorem ne32_succ (x : U32) : (⟨x.bv + 1#32⟩ : U32) = x ↔ False := by
  rw [eq32, bv32]; constructor
  · intro h; bv_decide
  · intro h; exact h.elim

/-- `check::ceres_t_cycle`: the hand model's `Cer.count`. -/
theorem t_cycle (c : Clock) (host : Host) (t : U8) (first : Bool × BitVec 8) (twice : Bool)
    (hs : c.stopped = false) (hd : c.div_cycles = 0#i32) (hr : c.tima_reload_pending.bv ≤ 8#8)
    (hi : host.timer_interrupts.bv < 0xFFFFFFFF#32) :
    timing.check.ceres_t_cycle c host t (optOf first) twice =
      let (first', twice', x) := Cer.count t.bv (first, twice, ofC c)
      ok ((optOf first', twice'), { withC c x with apu_late_bit := none },
        bif (ofC c).tCycle.1 then { host with timer_interrupts := ⟨host.timer_interrupts.bv + 1#32⟩ }
        else host) := by
  unfold timing.check.ceres_t_cycle
  rw [run1 _ _ hs hd hr hi]
  simp only [bind_ok, uncurry_apply_pair, Cer.count]
  obtain ⟨b, v⟩ := first
  cases hf : (ofC c).tCycle.1 <;> cases b <;>
    simp [optOf, ne32_succ]

theorem withC_ofC (c : Clock) : withC c (ofC c) = c := rfl

theorem or8 (a b : U8) : (a ||| b).bv = a.bv ||| b.bv := rfl

theorem not8 (a : U8) : (~~~a).bv = ~~~a.bv := rfl

theorem some8 (a b : U8) : (some a = some b) = (a.bv = b.bv) := by
  rw [Option.some.injEq, eq8]

/-- The read, as the translation gives it. -/
def readOf' (r : Bool × BitVec 8) : Option U8 :=
  bif r.1 then some ⟨r.2⟩ else none

/-- `check::ceres_access` on a clock with no APU delay: the hand model's
access. A DIV write leaves `during_div_write` clear. -/
theorem access (c : Clock) (host : Host) (acc : ceres_netlist.timer.Access)
    (ha : c.apu_delay = 0#u16) :
    timing.check.ceres_access inst c host acc =
      let (read, x) := (ofC c).access (accOf acc)
      ok (readOf' read,
        match acc with
        | .Write .Div _ =>
          { withC c x with apu_late_bit := none, during_div_write := false }
        | _ => withC c x,
        host) := by
  rcases acc with _ | r | ⟨r, ⟨v⟩⟩ <;> (try cases r) <;>
    simp only [timing.check.ceres_access, accOf, Cer.access, readOf', bind_ok, lift,
      timing.timer.Clock.impl.tima, timing.timer.Clock.impl.tma, timing.timer.Clock.read_tac,
      timing.timer.Clock.write_div, timing.timer.Clock.write_tima, timing.timer.Clock.write_tma,
      timing.timer.Clock.write_tac, timing.timer.TAC_ENABLE_B, timing.timer.TAC_CLOCK, ha,
      sys_clk_tac_mux, inc_tima, set_div, ↓reduceIte, Bool.cond_true, Bool.cond_false,
      withC_ofC, uncurry_apply_pair, ite_ok]
  case Read.Div =>
    simp only [timing.timer.Clock.read_div, HShiftRight.hShiftRight, UScalar.shiftRight_IScalar,
      UScalar.shiftRight]
    rw [ite_eq_left (by decide), ite_eq_left (by decide), show (8#i32).toNat = 8 by decide]
    simp only [bind_ok, UScalar.cast, ofC]
    rfl
  case Write.Div => simp only [withC, ofC, ha, lit16]
  all_goals
    try dsimp only [ofC, withC]
    try simp only [ok.injEq, Prod.mk.injEq, true_and, and_true, some8, ite_clock, ite8, ite16,
      timing.timer.Clock.mk.injEq, or8, not8, and8, and16, lit8, bv8, bv16, le8, bne8, bne16, eq8,
      eq16, Cer.pick, Cer.incTima, mux, ha, lit16]
    try (revert v; refine fun (v : BitVec 8) => ?_)
    try bv_decide

/-- The first T-cycle, with no interrupt yet. -/
theorem t_cycle0 (c : Clock) (host : Host) (t : U8)
    (hs : c.stopped = false) (hd : c.div_cycles = 0#i32) (hr : c.tima_reload_pending.bv ≤ 8#8)
    (hi : host.timer_interrupts.bv < 0xFFFFFFFF#32) :
    timing.check.ceres_t_cycle c host t none false =
      let (first', twice', x) := Cer.count t.bv ((false, 0#8), false, ofC c)
      ok ((optOf first', twice'), { withC c x with apu_late_bit := none },
        bif (ofC c).tCycle.1 then { host with timer_interrupts := ⟨host.timer_interrupts.bv + 1#32⟩ }
        else host) :=
  t_cycle c host t (false, 0#8) false hs hd hr hi

theorem zero32 : (0#u32).bv = 0#32 := rfl

/-- What the steps' side conditions ask: the pinned fields, kept by
definition, and the bounds, from the hand model. -/
macro "side" : tactic => `(tactic| first
  | assumption
  | (simp only [zero32]; decide)
  | (dsimp only [withC, ofC]
     simp only [bv8, bv16, bv32, zero32, cond_host, cond32, Cer.count, Cer.tCycle,
       Cer.reloadNext, Cer.setDiv, Cer.incTima, Cer.pick, Cer.access, mux, accOf, U8.bv,
       Bool.cond_true, Bool.cond_false]
     bv_decide))

set_option maxHeartbeats 2000000 in
/-- `check::ceres_m_cycle` from a clock the step relates, with the host
`timing::check` makes: the hand model's M-cycle, and the clock after is
related again (its pinned fields kept). -/
theorem cer_m_cycle (c : Clock) (mask : U16) (acc : ceres_netlist.timer.Access)
    (hs : c.stopped = false) (hd : c.div_cycles = 0#i32) (ha : c.apu_delay = 0#u16)
    (hr : c.tima_reload_pending.bv ≤ 8#8) :
    let hand := (ofC c).mCycle (accOf acc)
    ∃ c' h', timing.check.ceres_m_cycle c ⟨mask, 0#u32⟩ acc =
        ok ((readOf' hand.1, optOf hand.2.1, hand.2.2.1), c', h') ∧
      ofC c' = hand.2.2.2 ∧ c'.stopped = false ∧ c'.div_cycles = 0#i32 ∧ c'.apu_delay = 0#u16 ∧
      c'.apu_late_bit = none := by
  intro hand
  unfold timing.check.ceres_m_cycle
  rw [access _ _ _ ha]
  rcases acc with _ | r | ⟨r, v⟩ <;> (try cases r) <;>
    simp only [bind_ok, uncurry_apply_pair] <;>
    rw [t_cycle0 _ _ _ (by side) (by side) (by side) (by side)] <;>
    simp only [bind_ok, uncurry_apply_pair] <;>
    rw [t_cycle _ _ _ _ _ (by side) (by side) (by side) (by side)] <;>
    simp only [bind_ok, uncurry_apply_pair] <;>
    rw [t_cycle _ _ _ _ _ (by side) (by side) (by side) (by side)] <;>
    simp only [bind_ok, uncurry_apply_pair] <;>
    rw [t_cycle _ _ _ _ _ (by side) (by side) (by side) (by side)] <;>
    simp only [bind_ok, uncurry_apply_pair] <;>
    exact ⟨_, _, rfl, rfl, hs, hd, ha, rfl⟩

end Pilot.Timer
