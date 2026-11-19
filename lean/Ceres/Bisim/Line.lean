import Ceres.Bisim.Scan
import Ceres.Bisim.Mode3
open Aeneas Aeneas.Std Result

/-!
# A line's start: the OAM scan, then mode 3

`Sketch/Chip.lean` takes mode 3's pipeline as one block stepped a dot at a time,
its claim `hM3` a hypothesis over every dot, with its relation `R3` holding
from the start. A line does not start in mode 3: the OAM scan runs first,
over the line's first 80 dots, and mode 3 starts from the sprites the scan
left in the store; Kani's mode 3 harnesses take the store as the scan left
it (`netlist_mode3_start`: "any objects the scan found").

This file puts the scan in front of mode 3's start, with the scan proved
(`Ceres/Bisim/Scan.lean`: `scan_agrees`, from Kani's three windows) and
mode 3's start a hypothesis (`hStart`: from the scan's end, ceres's first
ten sprites in the netlist's slots and the scan stopped, mode 3's states
are related; Kani's `netlist_mode3_start` for the pipeline):
`scan_then_mode3`. With mode 3's blocks made from the store landing where
`start_before` has them (`Starting`), `scan_into_mode3` goes on through
mode 3's first dot (`mode3_start_steps`) into related states. What it does
not do yet is route a line's dots between the two blocks inside
`chip_agrees`: its `hM3` still stands for the scan's dots as well.
-/

namespace ceres_core.scan
open ceres_core.bisim ceres_core.ppu.display.objects.relation

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

/-- The scan, then mode 3's first dot: from the line before's end, the
scan leaves the pipeline and the loop made from the store it left
(`startN`, `startC`) where `start_before` has them (`hStart`, the store's
mapping onto mode 3's blocks), and from there mode 3's first dot, on any
input the start's guard lets through, leaves them related (Kani's
`netlist_mode3_start`, `mode3_start_steps`), mode 3 going on in the
netlist. -/
-- @assumes mapping-mode3-start
theorem scan_into_mode3 (startN : NetSide → ceres_netlist.render_edge.NetPipeline)
    (startC : CeresSide → ceres_core.ppu.display.background.relation.Mode3Loop)
    (kStep : KaniScanStep) (kLead : KaniScanLeadIn) (kTail : KaniScanTail)
    (kStart : ceres_core.mode3.KaniMode3Start)
    (hStart : ∀ net ceres, slots_are net ceres ceres.found = ok true →
      ¬ net.scan.scanning → ceres_core.mode3.Starting (startN net) (startC ceres))
    (count : U8) (ns : Bool) (held : U8) (line : Line) (ys : Std.Array U8 40#usize)
    (hc : count ≤ ceres_netlist.oam_scan.SLOTS) (hl : line.ly < 144#u8) :
    scan count ns held line ys ⦃ (net, ceres) => ∀ i,
      ceres_core.mode3.StartK (startN net) i ∨
      ∃ n' c', ceres_core.mode3.netStep (startN net) i = ok ((false, false), n') ∧
        ceres_core.mode3.cerStart (startC ceres) i = ok (false, c') ∧
        ceres_core.mode3.Related n' c' ⦄ :=
  WP.spec_mono (scan_then_mode3 _ startN startC kStep kLead kTail hStart count ns held line ys hc hl)
    (fun (net, ceres) h i => by
      rcases ceres_core.mode3.mode3_start_steps kStart _ _ h i i rfl with
        k | ⟨o, n', p, c', hn, hcer, ho, hr⟩
      · exact Or.inl k
      · right
        rcases ho with ⟨rfl, -, -⟩ | ⟨rfl, h1, h2⟩
        · unfold ceres_core.mode3.cerStart at hcer
          obtain ⟨_, -, hcer⟩ := bind_eq_ok hcer
          simp at hcer
        · obtain ⟨o1, o2⟩ := o
          simp only at h1 h2
          subst h1 h2
          exact ⟨n', c', hn, hcer, hr⟩)

end ceres_core.scan