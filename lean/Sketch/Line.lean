import Ceres.Bisim.Scan
import Sketch.Chip
open Aeneas Aeneas.Std Result

/-!
# A line's start: the OAM scan, then mode 3

`Chip.lean` takes mode 3's pipeline as one block stepped a dot at a time,
its claim `hM3` a hypothesis over every dot, with its relation `R3` holding
from the start. A line does not start in mode 3: the OAM scan runs first,
over the line's first 80 dots, and mode 3 starts from the sprites the scan
left in the store; Kani's mode 3 harnesses take the store as the scan left
it (`netlist_mode3_start`: "any objects the scan found").

This file puts the scan in front of mode 3's start, with the scan proved
(`Ceres/Bisim/Scan.lean`: `scan_agrees`, from Kani's three windows) and
mode 3's start a hypothesis (`hStart`: from the scan's end, ceres's first
ten sprites in the netlist's slots and the scan stopped, mode 3's states
are related; Kani's `netlist_mode3_start` for the pipeline). What it does
not do yet is route a line's dots between the two blocks inside
`chip_agrees`: its `hM3` still stands for the scan's dots as well.
-/

namespace ceres_core.sketch
open ceres_core.bisim ceres_core.scan ceres_core.ppu.display.objects.check

/-- A line's start: from the line before's end, the scan, and mode 3's
states, made from the store the scan left (`startN`, `startC`), related. -/
theorem scan_then_mode3 {M3n M3c : Type} (R3 : M3n → M3c → Prop)
    (startN : NetSide → M3n) (startC : CeresSide → M3c)
    (kStep : KaniScanStep) (kLead : KaniScanLeadIn) (kTail : KaniScanTail)
    (hStart : ∀ net ceres, slots_are net ceres ceres.found = ok true →
      ¬ net.scan.scanning → R3 (startN net) (startC ceres))
    (count : U8) (ns : Bool) (held : U8) (line : Line) (ys : Std.Array U8 40#usize)
    (hc : count ≤ ceres_netlist.oam_scan.SLOTS) (hl : line.ly < 144#u8) :
    scan count ns held line ys ⦃ (net, ceres) => R3 (startN net) (startC ceres) ⦄ :=
  WP.spec_mono (scan_agrees kStep kLead kTail count ns held line ys hc hl)
    (fun x h => hStart x.1 x.2 h.1 h.2)

end ceres_core.sketch
