/-
The timer's step on the hand-written models, by `bv_decide`: what Kani's
`netlist_timer_bisimulation` proves on the Rust (2 s). One goal per shape of
the access, each a closed formula over the two states' bits.
-/
import Std.Tactic.BVDecide
import Pilot.Timer.Models

namespace Pilot.Timer

set_option maxHeartbeats 0 in
theorem step (n : Net) (c : Cer) (acc : Acc) : stepAgrees n c acc = true := by
  obtain ⟨count, tima, tma, tac, intTimer, bit7Before, timerClock, writeTma, writeTac,
    latched, raceCounts⟩ := n
  obtain ⟨div, ctac, ctima, ctma, reload, irq⟩ := c
  cases acc with
  | none =>
    simp only [stepAgrees, related, knownDifference, Net.mCycle, Net.phase, Net.settle,
      Net.edgeA, Net.pick, Acc.writes, Writes.nothing, Cer.mCycle, Cer.count,
      Cer.access, Cer.tCycle, Cer.reloadNext, Cer.setDiv, Cer.incTima, Cer.pick, clockOf, mux]
    bv_decide
  | read r =>
    cases r <;>
    simp only [stepAgrees, related, knownDifference, Net.mCycle, Net.phase, Net.settle,
      Net.edgeA, Net.read, Net.pick, Acc.writes, Writes.nothing, Cer.mCycle, Cer.count,
      Cer.access, Cer.tCycle, Cer.reloadNext, Cer.setDiv, Cer.incTima, Cer.pick, clockOf, mux] <;>
    bv_decide
  | write r v =>
    cases r <;>
    simp only [stepAgrees, related, knownDifference, Net.mCycle, Net.phase, Net.settle,
      Net.edgeA, Net.pick, Acc.writes, Writes.nothing, Cer.mCycle, Cer.count,
      Cer.access, Cer.tCycle, Cer.reloadNext, Cer.setDiv, Cer.incTima, Cer.pick, clockOf, mux] <;>
    bv_decide

end Pilot.Timer
