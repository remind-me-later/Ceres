import Ceres.Bisim.Lcd
import Ceres.Bisim.Scan
import Ceres.Bisim.Line
import Ceres.Bisim.Close
import Ceres.Bisim.Feedback
import Ceres.Bisim.Mode3
import Ceres.Bisim.ScanDots
import Ceres.Bisim.Rest
import Ceres.Bisim.LineRest

/-! The theorems rest on Lean's own axioms only: the build fails if one
comes to need another (`sorry`, or the compiler's, through `native_decide`
or `bv_decide`). Kani's results are the theorems' hypotheses, not axioms. -/

/--
info: 'ceres_core.timer_if.timer_and_if_agree' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.timer_if.timer_and_if_agree

/--
info: 'ceres_core.dma.timer_if_dma_agree' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.dma.timer_if_dma_agree

/--
info: 'ceres_core.lcd.blocks_agree' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.lcd.blocks_agree

/--
info: 'ceres_core.scan.scan_agrees' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.scan.scan_agrees

/--
info: 'ceres_core.scan.scan_then_mode3' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.scan.scan_then_mode3

/--
info: 'ceres_core.bisim.Steps.close' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.bisim.Steps.close

/--
info: 'ceres_core.bisim.Steps.feedback' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.bisim.Steps.feedback

/--
info: 'ceres_core.bisim.StepsInto.embed' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.bisim.StepsInto.embed

/--
info: 'ceres_core.mode3.mode3_dot_steps' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.mode3.mode3_dot_steps

/--
info: 'ceres_core.mode3.mode3_start' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.mode3.mode3_start

/--
info: 'ceres_core.mode3.mode3_start_steps' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.mode3.mode3_start_steps

/--
info: 'ceres_core.scan.scan_into_mode3' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.scan.scan_into_mode3

/--
info: 'ceres_core.scan.scan_dot_steps' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.scan.scan_dot_steps

/--
info: 'ceres_core.scan.scan_end_saves' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.scan.scan_end_saves

/--
info: 'ceres_core.rest.end_edges' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.rest.end_edges

/--
info: 'ceres_core.rest.pipe_scan_dot' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.rest.pipe_scan_dot

/--
info: 'ceres_core.rest.pipe_idle_dot' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.rest.pipe_idle_dot

/--
info: 'ceres_core.rest.pipe_hblank_dot' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.rest.pipe_hblank_dot

/--
info: 'ceres_core.rest.line_scan_dot' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.rest.line_scan_dot

/--
info: 'ceres_core.rest.line_idle_dot' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.rest.line_idle_dot

/--
info: 'ceres_core.rest.scan_block_dot' depends on axioms: [propext, Classical.choice, Quot.sound]
-/
#guard_msgs in
#print axioms ceres_core.rest.scan_block_dot
