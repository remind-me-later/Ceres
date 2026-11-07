import Ceres.Bisim.Dma
open Aeneas Aeneas.Std Result ceres_core ceres_core.bisim ceres_core.timer_if ceres_core.dma

/-!
# The line timing, wired into IF

Kani proves the line timing's M-cycle for every pair of states
(`netlist_lcd_access` asserts `ppu::display::timing::check::step_agrees`),
the hypothesis `KaniLcd`: from related states at an M-cycle's boundary
(`h = 0`, mod 8), an access to STAT, LY or LYC gives the same read, and the
STAT line rises alike as IF tells its rises apart, the netlist's by edge,
ceres's by T-cycle (`interrupts::check::same_stat`). The step reads none of
the other blocks' outputs, and IF reads its rises: so it is wired in front
of the timer and IF, its rises put in IF's input (`Steps.comap`), and the
OAM DMA after them.

Ceres's side is the line timing's M-cycle, then the timer's and IF's as
ceres runs them, which take its rises in their T-cycles, then the DMA's.
Ceres runs the PPU after the timer in `Gb::advance_cycles`, but the two
share nothing but IF, whose requests only set its bits, in the T-cycle the
source rises in (`mapping-m-cycle-at-e`, which IF's step assumes).

The step takes LCDC writes too: the related states have the LCD on, both
clocks at the same half dot; or in line 0 after it was turned on, ceres's
clock 2 half dots ahead until the pixel pipeline resyncs it; or the LCD
off. It takes the HBlank source as well: where mode 3 ends is an input,
the same on both sides (`mode3End`; `mapping-mode3-end`), which raises
ceres's HBlank gate and render's mode 0. It leaves out turning the LCD on
with LYC 0, its enable on and the flag clear, when ceres raises the STAT
line an M-cycle before the netlist (`lcd-on-lyc0-invisible`, which Kani's
`netlist_lcd_on` pins down), and mode 3 ending where the pixel pipeline
does not end it.
-/

namespace ceres_core.lcd

/-- The netlist's line timing and LCDC's latches. -/
abbrev NetLcd := ppu.display.timing.check.Net
abbrev CeresLcd := ppu.display.timing.LcdTiming

/-- The netlist's M-cycle: the read, and the STAT line's rises by edge. -/
def netLcd (n : NetLcd) (i : NetIn) : Result ((Option Std.U8 × Array Bool 8#usize) × NetLcd) :=
  ppu.display.timing.check.net_m_cycle n i.lcd i.mode3End

/-- Ceres's M-cycle: the read, and the STAT line's rises by T-cycle. -/
def ceresLcd (c : CeresLcd) (j : CeresIn) :
    Result ((Option Std.U8 × Array Bool 4#usize) × CeresLcd) :=
  ppu.display.timing.check.ceres_m_cycle c j.lcd j.mode3End

/-- The same read, and the rises alike as IF tells them apart. -/
def LcdOut (o : Option Std.U8 × Array Bool 8#usize) (p : Option Std.U8 × Array Bool 4#usize) :
    Prop :=
  o.1 = p.1 ∧ interrupts.check.same_stat o.2 p.2 = ok true

/-- What the step leaves out: turning the LCD on with LYC 0, its enable on
and the flag clear, and mode 3 ending where the pixel pipeline does not
end it. -/
def LcdK (n : NetLcd) (i : NetIn) : Prop :=
  ppu.display.timing.check.outside n i.lcd i.mode3End = ok true

/-- What Kani proves. -/
def KaniLcd : Prop :=
  ∀ net t access mode3End,
    ppu.display.timing.check.step_agrees net t access mode3End = ok true

/-- The line timing's M-cycle, from every related pair (Kani's step). -/
-- @assumes lcd-step-agrees
theorem lcd_steps (kani : KaniLcd) :
    Steps netLcd ceresLcd SameIn LcdOut LcdK
      (fun n c => ppu.display.timing.check.related n c = ok true) := by
  intro n c h i j hij
  obtain ⟨-, -, -, -, hl, -, hm⟩ := hij
  have k := kani n c i.lcd i.mode3End
  unfold ppu.display.timing.check.step_agrees at k
  obtain ⟨b, hb, k⟩ := bind_eq_ok k
  rw [h, Result.ok.injEq] at hb
  subst hb
  obtain ⟨d, hd, k⟩ := bind_eq_ok k
  cases d
  · right
    simp only [Bool.false_eq_true, ↓reduceIte, decide_not, decide_true, Bool.not_true,
      Bool.or_false] at k
    obtain ⟨⟨⟨nr, nrose⟩, n'⟩, hn, k⟩ := bind_eq_ok k
    obtain ⟨⟨⟨cr, crose⟩, c'⟩, hc, k⟩ := bind_eq_ok k
    obtain ⟨s1, hs1, k⟩ := bind_eq_ok k
    obtain ⟨s2, hs2, k⟩ := bind_eq_ok k
    obtain ⟨r, hr, k⟩ := bind_eq_ok k
    simp only [Result.ok.injEq, Bool.and_eq_true] at k
    obtain ⟨⟨e1, e2⟩, e3⟩ := k
    subst e1 e2 e3
    refine ⟨(nr, nrose), n', (cr, crose), c', hn, ?_, ⟨same_read_true hs1, hs2⟩, hr⟩
    simp only [ceresLcd]
    rw [← hl, ← hm]
    exact hc
  · left
    exact hd

/-! The machine's blocks wired together. -/

abbrev LcdNetOut := Option Std.U8 × Array Bool 8#usize
abbrev LcdCeresOut := Option Std.U8 × Array Bool 4#usize
abbrev JointNetOut := (Option Std.U8 × Bool) × (Option Std.U8 × Option Std.U8)
abbrev JointCeresOut := Option Std.U8 × Option Std.U8 × Option Std.U8

/-- IF's input, with the STAT line's rises that the line timing gives. -/
def withStat (x : NetIn × (Option Std.U8 × Array Bool 8#usize)) : NetIn :=
  { x.1 with stat := x.2.2 }

def withStatC (y : CeresIn × (Option Std.U8 × Array Bool 4#usize)) : CeresIn :=
  { y.1 with stat := y.2.2 }

/-- The netlist: the line timing, the timer and IF (the line's rises into
IF), the OAM DMA. -/
def netBlocks := wire (wire netLcd (fun s x => wire netTimer netIf s (withStat x)))
  (fun s x => netDma s (x.1, x.2.2))

/-- Ceres: the line timing's M-cycle, the timer's and IF's as ceres runs
them, the DMA's. -/
def ceresBlocks := wire (wire ceresLcd (fun s y => ceresJoint s (withStatC y)))
  (fun s y => ceresDma s (y.1, y.2.2))

/-- The outputs alike: the line timing's read, the timer's read, IF's read,
the interrupt taken, and the DMA's read, OAM write, run and blocking. -/
def BlocksOut (o : ((Option Std.U8 × Array Bool 8#usize) ×
      ((Option Std.U8 × Bool) × (Option Std.U8 × Option Std.U8))) ×
      (Option Std.U8 × Option (Std.U16 × Std.U8) × Bool × Bool))
    (p : ((Option Std.U8 × Array Bool 4#usize) ×
      (Option Std.U8 × Option Std.U8 × Option Std.U8)) × memory.dma.check.CeresOut) : Prop :=
  (LcdOut o.1.1 p.1.1 ∧ JointOut o.1.2 p.1.2) ∧ DmaOut o.2 p.2

/-- The known differences: the line timing's left-out STAT write, or the
timer's. -/
def BlocksK := wireK (wire netLcd (fun s x => wire netTimer netIf s (withStat x)))
  (wireK netLcd LcdK (fun s x => wireK netTimer TimerK (fun _ _ => False) s (withStat x)))
  (fun (_ : NetDma) _ => False)

/-- Every block's states related. -/
def BlocksRelated (n : (NetLcd × (NetTimer × NetIf)) × NetDma)
    (c : (CeresLcd × (CeresTimer × CeresIf)) × CeresDma) : Prop :=
  (ppu.display.timing.check.related n.1.1 c.1.1 = ok true ∧
    (timing.check.related n.1.2.1 c.1.2.1 = ok true ∧
      interrupts.check.related n.1.2.2 c.1.2.2 = ok true)) ∧
  memory.dma.check.related n.2 c.2 = ok true

-- @assumes lcd-step-agrees
-- @assumes timer-step-agrees
-- @assumes if-step-agrees
-- @assumes blocks-in-sequence
-- @assumes dma-step-agrees
theorem blocks_steps (kl : KaniLcd) (kt : KaniTimer) (ki : KaniIf) (ks : KaniSeq)
    (kd : KaniDma) : Steps netBlocks ceresBlocks SameIn BlocksOut BlocksK BlocksRelated := by
  have ti := (timer_and_if_steps kt ki ks).comap withStat withStatC
    (RI' := fun x y => SameIn x.1 y.1 ∧ LcdOut x.2 y.2) (by
      rintro ⟨i, o⟩ ⟨j, p⟩ ⟨⟨h1, h2, h3, h4, h5, -, h6⟩, -, h7⟩
      exact ⟨h1, h2, h3, h4, h5, h7, h6⟩)
  have d := (dma_steps kd).comap (fun x => (x.1, x.2.2)) (fun y => (y.1, y.2.2))
    (RI' := fun (x : NetIn × (LcdNetOut × JointNetOut))
      (y : CeresIn × (LcdCeresOut × JointCeresOut)) =>
        SameIn x.1 y.1 ∧ (LcdOut x.2.1 y.2.1 ∧ JointOut x.2.2 y.2.2))
    (fun x y hxy => hxy.1.2.2.1)
  exact ((lcd_steps kl).wire ti).wire d

/-- The line timing, the timer, IF and the OAM DMA, as ceres runs them:
from related states, every M-cycle gives the same reads of STAT, LY, LYC,
the timer, IF and FF46, the same interrupt taken (the STAT line's, its
HBlank source included, and the timer's interrupts among those IF takes),
the same OAM write, and the DMA's run and OAM blocking alike, the LCD
turned off and on among them, mode 3 ending where the same input says, unless
it is one of the timer's known differences or what the line timing's step
leaves out. -/
-- @assumes lcd-step-agrees
-- @assumes timer-step-agrees
-- @assumes if-step-agrees
-- @assumes blocks-in-sequence
-- @assumes dma-step-agrees
-- @assumes dma-one-run-per-m-cycle
theorem blocks_agree (kl : KaniLcd) (kt : KaniTimer) (ki : KaniIf) (ks : KaniSeq)
    (kd : KaniDma) (n : (NetLcd × (NetTimer × NetIf)) × NetDma)
    (c : (CeresLcd × (CeresTimer × CeresIf)) × CeresDma) (h : BlocksRelated n c) :
    Agree netBlocks ceresBlocks SameIn BlocksOut BlocksK n c :=
  (blocks_steps kl kt ki ks kd).agree h

end ceres_core.lcd
