import Sketch.Chip
import Sketch.Dmg
import Sketch.DmgReal

/-! The outline's theorems rest on Lean's own axioms and on their
hypotheses (the blocks' claims) only: the build of `Sketch` fails if one
comes to need another (`sorry`). -/

/--
info: 'ceres_core.sketch.chip_agrees' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.sketch.chip_agrees

/--
info: 'ceres_core.sketch.dmg.dmg_agrees' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.sketch.dmg.dmg_agrees

/--
info: 'ceres_core.sketch.dmg.dmg_agrees_real' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.sketch.dmg.dmg_agrees_real
