import Ceres.Bisim.TimerIf

/-! The theorems rest on Lean's own axioms only: the build fails if one
comes to need another (`sorry`, or the compiler's, through `native_decide`
or `bv_decide`). Kani's results are the theorems' hypotheses, not axioms. -/

/--
info: 'ceres_core.timer_if.timer_and_if_agree' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.timer_if.timer_and_if_agree
