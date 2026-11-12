import Ceres.Bisim.Lcd
import Sketch.Chip
open Aeneas Aeneas.Std Result
open ceres_core ceres_core.bisim ceres_core.timer_if ceres_core.dma ceres_core.lcd

/-!
# The outline's M-cycle hypothesis, instantiated by the real blocks

`Chip.lean` takes the M-cycle blocks as `perNet` / `perCer` on
`AccN × List Bool` / `AccC × List Bool`, one flag per dot. The real blocks
(`netBlocks` / `ceresBlocks`, `Ceres/Bisim/Lcd.lean`) take a `NetIn` /
`CeresIn`, whose field `mode3End : Option U8` is not one flag per dot: it is
the half dot of the M-cycle (`k`, from 0 at E, eight of them) where mode 3
ends, or `none`.

`endOf` turns the dots' flags into that field. It reads the first flag set
among the first four dots (the M-cycle has four), and gives the odd half dot
`2d + 1` of that dot, the only half dot `mode3_may_end` lets the pipeline
end mode 3 at (`ceres-core/src/ppu/display/timing/check.rs`, `mode3_may_end`).
An end at the even half dot `2d` is not expressible with a flag; on the real
side it is a known difference (`outside`, which `LcdK` carries), so the
instance cannot state it. Several flags set in one M-cycle: only the first
survives. Both are what the outline still takes for granted of the clock:
that mode 3 ends at a dot's second edge, and at most once in an M-cycle.

The proof obligation of `Steps.comap` (related abstract inputs give related
real inputs) is proved here: equal flags give equal `mode3End`, and the rest
of the input relation is `SameIn` without its `mode3End` clause.
-/

namespace ceres_core.sketch

/-- The half dot of the M-cycle where mode 3 ends, from one flag per dot:
the first dot flagged among the first four, at its odd half dot. -/
def endOf (f : List Bool) : Option Std.U8 :=
  if f.getD 0 false then some 1#u8
  else if f.getD 1 false then some 3#u8
  else if f.getD 2 false then some 5#u8
  else if f.getD 3 false then some 7#u8
  else none

/-- The netlist's input with the flags' mode-3 end. -/
def withEndN (x : NetIn × List Bool) : NetIn := { x.1 with mode3End := endOf x.2 }

/-- Ceres's input with the flags' mode-3 end. -/
def withEndC (y : CeresIn × List Bool) : CeresIn := { y.1 with mode3End := endOf y.2 }

/-- The real input relation of `blocks_steps` (`SameIn`) without its
`mode3End` clause, which the flags supply on both sides. -/
def RAccReal (i : NetIn) (j : CeresIn) : Prop :=
  i.timer = j.timer ∧ i.ifAccess = j.ifAccess ∧ i.dma = j.dma ∧ i.rises = j.rises ∧
    i.lcd = j.lcd ∧ interrupts.check.same_stat i.stat j.stat = ok true

/-- The M-cycle's state and outputs, the real types. -/
abbrev Pn := (NetLcd × (NetTimer × NetIf)) × NetDma
abbrev Pc := (CeresLcd × (CeresTimer × CeresIf)) × CeresDma
abbrev SeenN := ((Option Std.U8 × Array Bool 8#usize) ×
    ((Option Std.U8 × Bool) × (Option Std.U8 × Option Std.U8))) ×
  (Option Std.U8 × Option (Std.U16 × Std.U8) × Bool × Bool)
abbrev SeenC := ((Option Std.U8 × Array Bool 4#usize) ×
    (Option Std.U8 × Option Std.U8 × Option Std.U8)) × memory.dma.check.CeresOut

/-- The netlist's M-cycle, with the flags' mode-3 end. -/
def perNetReal (s : Pn) (x : NetIn × List Bool) : Result (SeenN × Pn) :=
  netBlocks s (withEndN x)

/-- Ceres's M-cycle, with the flags' mode-3 end. -/
def perCerReal (s : Pc) (y : CeresIn × List Bool) : Result (SeenC × Pc) :=
  ceresBlocks s (withEndC y)

/-- The known differences on the flags' input. -/
def BlocksKReal (s : Pn) (x : NetIn × List Bool) : Prop := BlocksK s (withEndN x)

theorem forall2_eq_list {α : Type} : ∀ {l1 l2 : List α},
    List.Forall₂ (fun o p => o = p) l1 l2 → l1 = l2 := by
  intro l1 l2 h
  induction h with
  | nil => rfl
  | cons hab _ ih => exact congrArg₂ List.cons hab ih

/-- The instance: `blocks_steps` through `Steps.comap`. -/
theorem hPer_real (kl : KaniLcd) (kt : KaniTimer) (ki : KaniIf) (ks : KaniSeq) (kd : KaniDma) :
    Steps perNetReal perCerReal
      (fun i j => RAccReal i.1 j.1 ∧ List.Forall₂ (fun o p => o = p) i.2 j.2)
      BlocksOut BlocksKReal BlocksRelated :=
  (blocks_steps kl kt ki ks kd).comap withEndN withEndC
    (RI' := fun i j => RAccReal i.1 j.1 ∧ List.Forall₂ (fun o p => o = p) i.2 j.2)
    (by
      rintro ⟨x, fx⟩ ⟨y, fy⟩ ⟨⟨h1, h2, h3, h4, h5, h6⟩, hf⟩
      have hfe : fx = fy := forall2_eq_list hf
      subst hfe
      exact ⟨h1, h2, h3, h4, h5, h6, rfl⟩)

end ceres_core.sketch
