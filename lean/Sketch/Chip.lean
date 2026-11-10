import Sketch.Iterate
import Sketch.Close
open Aeneas Aeneas.Std Result

/-!
# The chip as an outline

Three abstract blocks and the claims about them, and how they compose into
a whole-chip agreement. Nothing here refers to the emulator's types: each
block is a step on types the file takes as parameters, and each claim is
a hypothesis. The file shows the shape of the composition, not a proof of
the hardware.
-/

namespace ceres_core.sketch
open ceres_core.bisim

section

/-! ### Mode 3's pixel pipeline, one dot at a time. Output: whether mode 3
ended in that dot. -/

variable {M3n M3c DotN DotC : Type}
variable (m3Net : M3n → DotN → Result (Bool × M3n))
variable (m3Cer : M3c → DotC → Result (Bool × M3c))
variable (RDot : DotN → DotC → Prop)
variable (K3 : M3n → DotN → Prop)
variable (R3 : M3n → M3c → Prop)

/-! ### The blocks proved one M-cycle at a time (timer, interrupts, DMA, LCD
line timing). Input: the CPU's bus access of the M-cycle, with the
mode-3-ended flags of that M-cycle's dots. Output: what the CPU sees. -/

variable {Pn Pc AccN AccC SeenN SeenC : Type}
variable (perNet : Pn → AccN × List Bool → Result (SeenN × Pn))
variable (perCer : Pc → AccC × List Bool → Result (SeenC × Pc))
variable (RAcc : AccN → AccC → Prop)
variable (RSeen : SeenN → SeenC → Prop)
variable (KP : Pn → AccN × List Bool → Prop)
variable (RP : Pn → Pc → Prop)

/-! ### How an M-cycle's access looks to mode 3, dot by dot. -/

variable (dotsN : AccN → List DotN)
variable (dotsC : AccC → List DotC)

/-! ### The CPU, as a driver: its state and the outside input give the
access; it then takes what it sees back. -/

variable {Cn Cc X : Type}
variable (outN : Cn → X → AccN)
variable (outC : Cc → X → AccC)
variable (nextN : Cn → X → SeenN → Result Cn)
variable (nextC : Cc → X → SeenC → Result Cc)
variable (RC : Cn → Cc → Prop)

/-- The PPU: mode 3's dots of the M-cycle first, then the M-cycle blocks
taking the access and the flags. -/
def ppuNet : M3n × Pn → AccN → Result ((List Bool × SeenN) × (M3n × Pn)) :=
  wire (fun s a => iterate m3Net s (dotsN a)) perNet

/-- The PPU's ceres side, the same wiring. -/
def ppuCer : M3c × Pc → AccC → Result ((List Bool × SeenC) × (M3c × Pc)) :=
  wire (fun s a => iterate m3Cer s (dotsC a)) perCer

/-- The PPU steps, with the known differences of its mode 3 run and of its
M-cycle blocks. -/
theorem ppu_steps
    (hM3 : Steps m3Net m3Cer RDot (fun o p => o = p) K3 R3)
    (hPer : Steps perNet perCer
      (fun i j => RAcc i.1 j.1 ∧ List.Forall₂ (fun o p => o = p) i.2 j.2) RSeen KP RP)
    (hDots : ∀ a b, RAcc a b → List.Forall₂ RDot (dotsN a) (dotsC b)) :
    Steps (ppuNet m3Net perNet dotsN) (ppuCer m3Cer perCer dotsC) RAcc
      (fun o p => List.Forall₂ (fun a b => a = b) o.1 p.1 ∧ RSeen o.2 p.2)
      (wireK (fun s a => iterate m3Net s (dotsN a))
        (fun s a => iterateK m3Net K3 s (dotsN a)) KP)
      (fun n c => R3 n.1 c.1 ∧ RP n.2 c.2) :=
  Steps.wire (Steps.comap (Steps.iterate hM3) dotsN dotsC hDots) hPer

/-- The chip: the CPU closed around the PPU. The CPU takes only the `Seen`
part of the PPU's output. -/
def chipNet : Cn × (M3n × Pn) → X → Result ((List Bool × SeenN) × (Cn × (M3n × Pn))) :=
  close outN (fun d x o => nextN d x o.2) (ppuNet m3Net perNet dotsN)

/-- The chip's ceres side, the same closure. -/
def chipCer : Cc × (M3c × Pc) → X → Result ((List Bool × SeenC) × (Cc × (M3c × Pc))) :=
  close outC (fun d x o => nextC d x o.2) (ppuCer m3Cer perCer dotsC)

/-- The chip steps: the CPU closed around the PPU. -/
theorem chip_steps
    (hOut : ∀ dn dc x y, RC dn dc → x = y → RAcc (outN dn x) (outC dc y))
    (hNext : ∀ dn dc x y o p, RC dn dc → x = y → RSeen o p →
      ∃ dn' dc', nextN dn x o = ok dn' ∧ nextC dc y p = ok dc' ∧ RC dn' dc')
    (hM3 : Steps m3Net m3Cer RDot (fun o p => o = p) K3 R3)
    (hPer : Steps perNet perCer
      (fun i j => RAcc i.1 j.1 ∧ List.Forall₂ (fun o p => o = p) i.2 j.2) RSeen KP RP)
    (hDots : ∀ a b, RAcc a b → List.Forall₂ RDot (dotsN a) (dotsC b)) :
    Steps (chipNet m3Net perNet dotsN outN nextN) (chipCer m3Cer perCer dotsC outC nextC)
      (fun x y => x = y)
      (fun o p => List.Forall₂ (fun a b => a = b) o.1 p.1 ∧ RSeen o.2 p.2)
      (fun s x => wireK (fun s a => iterate m3Net s (dotsN a))
        (fun s a => iterateK m3Net K3 s (dotsN a)) KP s.2 (outN s.1 x))
      (fun n c => RC n.1 c.1 ∧ (R3 n.2.1 c.2.1 ∧ RP n.2.2 c.2.2)) :=
  Steps.close (ppu_steps m3Net m3Cer RDot K3 R3 perNet perCer RAcc RSeen KP RP dotsN dotsC
      hM3 hPer hDots)
    hOut
    (fun dn dc x y o p h hx ho => hNext dn dc x y o.2 p.2 h hx ho.2)

/-- The two closed machines, the netlist's and ceres's, agree from any
related pair of states. -/
theorem chip_agrees
    (hOut : ∀ dn dc x y, RC dn dc → x = y → RAcc (outN dn x) (outC dc y))
    (hNext : ∀ dn dc x y o p, RC dn dc → x = y → RSeen o p →
      ∃ dn' dc', nextN dn x o = ok dn' ∧ nextC dc y p = ok dc' ∧ RC dn' dc')
    (hM3 : Steps m3Net m3Cer RDot (fun o p => o = p) K3 R3)
    (hPer : Steps perNet perCer
      (fun i j => RAcc i.1 j.1 ∧ List.Forall₂ (fun o p => o = p) i.2 j.2) RSeen KP RP)
    (hDots : ∀ a b, RAcc a b → List.Forall₂ RDot (dotsN a) (dotsC b))
    {n : Cn × (M3n × Pn)} {c : Cc × (M3c × Pc)}
    (hr : RC n.1 c.1 ∧ (R3 n.2.1 c.2.1 ∧ RP n.2.2 c.2.2)) :
    Agree (chipNet m3Net perNet dotsN outN nextN) (chipCer m3Cer perCer dotsC outC nextC)
      (fun x y => x = y)
      (fun o p => List.Forall₂ (fun a b => a = b) o.1 p.1 ∧ RSeen o.2 p.2)
      (fun s x => wireK (fun s a => iterate m3Net s (dotsN a))
        (fun s a => iterateK m3Net K3 s (dotsN a)) KP s.2 (outN s.1 x)) n c :=
  Steps.agree (chip_steps m3Net m3Cer RDot K3 R3 perNet perCer RAcc RSeen KP RP dotsN dotsC
    outN outC nextN nextC RC hOut hNext hM3 hPer hDots) hr

end

/-! ## What is still open

Every proof in this file is closed except as noted below. The file takes
four hypotheses as arguments; none is proved here.

- `hM3`: Kani's mode 3 harnesses (`mode3-end-background`,
  `mode3-window-armed`, `mode3-window-start` in doc/assumptions.toml).
  `K3` is the scope they leave out (the known differences of the dot-by-dot
  pipeline).
- `hPer`: the Lean theorem `ceres_core.lcd.blocks_steps`
  (lean/Ceres/Bisim/Lcd.lean), to be instantiated with the M-cycle blocks.
- `hDots`: the clock relation between an M-cycle and its dots, which is not
  stated anywhere yet.
- `hOut`, `hNext`: the SM83 core against the netlist's sequencer; no proof
  yet.

No proof in this file uses `sorry`. `ppu_steps`, `chip_steps` and
`chip_agrees` rest on `Steps.iterate` (Iterate.lean) and `Steps.close`
(Close.lean), which contain no `sorry` either.
-/

end ceres_core.sketch
