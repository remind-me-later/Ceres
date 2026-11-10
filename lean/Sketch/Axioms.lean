import Sketch.Chip

/-! The outline's chip theorem rests on Lean's own axioms and on its
hypotheses (the blocks' claims) only: the build of `Sketch` fails if it
comes to need another (`sorry`). -/

/--
info: 'ceres_core.sketch.chip_agrees' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.sketch.chip_agrees
