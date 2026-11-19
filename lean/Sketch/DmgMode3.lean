import Sketch.Dmg
import Ceres.Bisim.Mode3
open Aeneas Aeneas.Std Result

/-!
# Mode 3 in the outline's line

`dmg_agrees` takes the PPU's line a dot at a time, phase by phase
(`hPhase`), each phase's claim a hypothesis. This file discharges the mode 3
phase's claim from `mode3_dot_steps` (Kani's dot harnesses, through Aeneas's
translation), leaving smaller hypotheses about how the line holds mode 3's
blocks:

- `hN`, `hC`: in mode 3, the line's dot is the pipeline's dot (the
  netlist's two edges) and the mode 3 loop's dot (ceres's), on the line's
  state taken apart (`pn`, `pc`) and put back (`setN`, `setC`, which `pn`
  and `pc` take out again: `hGetN`, `hGetC`), the rest of the line left as
  it was; the dot gives whether mode 3 ended (`endN`).
- `hI`: related dot inputs make the same mode 3 input (the mixer's SCX,
  the two edges' inputs).
- `hRL`: the mode 3 phase's relation is the blocks' (`Related`) with the
  rest of the line's (`F`), which a mode 3 dot leaves alone (`hFrame`).
- `hEnd`: once ceres has drawn the line and the netlist's mode 3 stopped
  (`Ended`), the line is in HBlank, as `RL hblank` has it (Kani's
  `mode3-end-background` for the pipeline; HBlank's relation is open).
- `hK`: the line's known difference takes mode 3's (`DotK`: the guard turns
  the dot's input away).

The dot's output is only whether mode 3 ended: the pixels it draws are not
compared yet.
-/

namespace ceres_core.sketch.dmg
open ceres_core.bisim ceres_core.mode3
open ceres_core.ppu.display.background.relation ceres_netlist.render_edge

section

variable {Ln Lc DIn DIc DO : Type}
variable {dotN : Ln → DIn → Result (DO × Ln)} {dotC : Lc → DIc → Result (DO × Lc)}
variable {RDI : DIn → DIc → Prop} {KDot : Ln → DIn → Prop}
variable {RL : Phase → Ln → Lc → Prop}

/-! ### The CPU's locks on VRAM and OAM, from the PPU's state. -/

variable {Lk : Type} {locksN : Ln → Lk} {locksC : Lc → Lk}

/-! ### The memory: VRAM, OAM, work RAM, the cartridge. -/

variable {Mn Mc AccN AccC MON MOC : Type}
variable {memN : Mn → AccN × Lk → Result (MON × Mn)} {memC : Mc → AccC × Lk → Result (MOC × Mc)}
variable {RAcc : AccN → AccC → Prop} {RMO : MON → MOC → Prop}
variable {KMem : Mn → AccN × Lk → Prop} {RM : Mn → Mc → Prop}

/-! ### The M-cycle's dots, from the access and what the memory gives. -/

variable {dotsN : AccN × MON → List DIn} {dotsC : AccC × MOC → List DIc}
variable {ended : DO → Bool}

/-! ### The M-cycle blocks: LCD timing, timer, IF, OAM DMA. -/

variable {Pn Pc SeenN SeenC : Type}
variable {perN : Pn → AccN × List Bool → Result (SeenN × Pn)}
variable {perC : Pc → AccC × List Bool → Result (SeenC × Pc)}
variable {RSeen : SeenN → SeenC → Prop} {KP : Pn → AccN × List Bool → Prop}
variable {RP : Pn → Pc → Prop}

/-! ### The rest of the bus: HRAM, boot ROM and undecoded I/O; the joypad
and the serial port; the APU. One step an M-cycle each, taking the access. -/

variable {Bn Bc BON BOC : Type}
variable {busN : Bn → AccN → Result (BON × Bn)} {busC : Bc → AccC → Result (BOC × Bc)}
variable {RBO : BON → BOC → Prop} {KBus : Bn → AccN → Prop} {RB : Bn → Bc → Prop}
variable {Jn Jc JON JOC X : Type}
variable {joyN : Jn → AccN × X → Result (JON × Jn)} {joyC : Jc → AccC × X → Result (JOC × Jc)}
variable {RJO : JON → JOC → Prop} {KJoy : Jn → AccN × X → Prop} {RJ : Jn → Jc → Prop}
variable {An Ac AON AOC : Type}
variable {apuN : An → AccN → Result (AON × An)} {apuC : Ac → AccC → Result (AOC × Ac)}
variable {RAO : AON → AOC → Prop} {KApu : An → AccN → Prop} {RA : An → Ac → Prop}

/-! ### The CPU. It takes the outside input `X` (the joypad's buttons, the
serial line), gives its access, and takes back everything the bus gave. -/

variable {Cn Cc : Type}
variable {outN : Cn → X → AccN} {outC : Cc → X → AccC}
variable {RC : Cn → Cc → Prop}


/-- The outputs `DotOut` relates end mode 3 together: the netlist's mode 3
stops at the dot's second edge where ceres had drawn the line. -/
theorem dotOut_ended {s : Bool × Bool} {d : Bool} (h : DotOut s d) : s.2 = d := by
  rcases h with ⟨rfl, -, h⟩ | ⟨rfl, -, h⟩ <;> exact h

/-- The mode 3 phase of the line, from Kani's dot harnesses: a dot from
mode 3's states leaves the line in mode 3 or, the line drawn, in HBlank. -/
-- @assumes mode3-end-background
-- @assumes mode3-window-armed
-- @assumes mode3-window-start
theorem mode3_phase (kani : KaniMode3Dot)
    (pn : Ln → NetPipeline) (pc : Lc → Mode3Loop)
    (setN : Ln → NetPipeline → Ln) (setC : Lc → Mode3Loop → Lc)
    (inN : DIn → DotIn) (inC : DIc → DotIn) (endN : Bool → DO)
    (hN : ∀ n i, dotN n i =
      (do let (o, s) ← netStep (pn n) (inN i); ok (endN o.2, setN n s)))
    (hC : ∀ c j, dotC c j =
      (do let (p, t) ← cerStep (pc c) (inC j); ok (endN p, setC c t)))
    (hGetN : ∀ n s, pn (setN n s) = s) (hGetC : ∀ c t, pc (setC c t) = t)
    (hI : ∀ i j, RDI i j → inN i = inC j)
    (F : Ln → Lc → Prop)
    (hRL : ∀ n c, RL .mode3 n c ↔ Related (pn n) (pc c) ∧ F n c)
    (hFrame : ∀ n c s t, F n c → F (setN n s) (setC c t))
    (hEnd : ∀ n c s t, F n c → Ended s t → RL .hblank (setN n s) (setC c t))
    (hK : ∀ n i, DotK (pn n) (inN i) → KDot n i) :
    StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL .mode3)
      (fun n c => ∃ k', RL k' n c) := by
  have e := StepsInto.embed (mode3_dot_steps kani) pn pc setN setC inN inC
    (fun o => endN o.2) endN hN hC (RI' := RDI) hI (RO' := fun o p => o = p)
    (fun o p h => congrArg endN (dotOut_ended h)) F (fun n c => ∃ k', RL k' n c)
    (by
      rintro n c s t hf (hr | he)
      · exact ⟨.mode3, (hRL _ _).2 ⟨by rw [hGetN, hGetC]; exact hr, hFrame n c s t hf⟩⟩
      · exact ⟨.hblank, hEnd n c s t hf he⟩)
  intro n c hr i j hij
  rcases e n c ((hRL n c).1 hr) i j hij with k | x
  · exact Or.inl (hK n i k)
  · exact Or.inr x

/-- The outline's `hPhase` with its mode 3 phase discharged: the other
phases' claims (`hOther`) and `mode3_phase`'s hypotheses give it all. -/
theorem phases_with_mode3 (kani : KaniMode3Dot)
    (hOther : ∀ k, k ≠ .mode3 → StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c))
    (pn : Ln → NetPipeline) (pc : Lc → Mode3Loop)
    (setN : Ln → NetPipeline → Ln) (setC : Lc → Mode3Loop → Lc)
    (inN : DIn → DotIn) (inC : DIc → DotIn) (endN : Bool → DO)
    (hN : ∀ n i, dotN n i =
      (do let (o, s) ← netStep (pn n) (inN i); ok (endN o.2, setN n s)))
    (hC : ∀ c j, dotC c j =
      (do let (p, t) ← cerStep (pc c) (inC j); ok (endN p, setC c t)))
    (hGetN : ∀ n s, pn (setN n s) = s) (hGetC : ∀ c t, pc (setC c t) = t)
    (hI : ∀ i j, RDI i j → inN i = inC j)
    (F : Ln → Lc → Prop)
    (hRL : ∀ n c, RL .mode3 n c ↔ Related (pn n) (pc c) ∧ F n c)
    (hFrame : ∀ n c s t, F n c → F (setN n s) (setC c t))
    (hEnd : ∀ n c s t, F n c → Ended s t → RL .hblank (setN n s) (setC c t))
    (hK : ∀ n i, DotK (pn n) (inN i) → KDot n i) :
    ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c) := by
  intro k
  cases k with
  | mode3 => exact mode3_phase kani pn pc setN setC inN inC endN hN hC hGetN hGetC hI F hRL hFrame hEnd hK
  | scan => exact hOther _ (by intro h; cases h)
  | hblank => exact hOther _ (by intro h; cases h)
  | vblank => exact hOther _ (by intro h; cases h)
  | off => exact hOther _ (by intro h; cases h)
  | first => exact hOther _ (by intro h; cases h)

/-- `dmg_agrees` with the line's mode 3 phase taken from Kani's dot
harnesses (`phases_with_mode3`): the other phases' claims and
`mode3_phase`'s hypotheses on how the line holds mode 3's blocks stand for
`hPhase`. -/
theorem dmg_agrees_mode3 (kani : KaniMode3Dot)
    (hOther : ∀ k, k ≠ .mode3 → StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c))
    (pn : Ln → NetPipeline) (pc : Lc → Mode3Loop)
    (setN : Ln → NetPipeline → Ln) (setC : Lc → Mode3Loop → Lc)
    (inN : DIn → DotIn) (inC : DIc → DotIn) (endN : Bool → DO)
    (hN : ∀ n i, dotN n i =
      (do let (o, s) ← netStep (pn n) (inN i); ok (endN o.2, setN n s)))
    (hC : ∀ c j, dotC c j =
      (do let (p, t) ← cerStep (pc c) (inC j); ok (endN p, setC c t)))
    (hGetN : ∀ n s, pn (setN n s) = s) (hGetC : ∀ c t, pc (setC c t) = t)
    (hI : ∀ i j, RDI i j → inN i = inC j)
    (F : Ln → Lc → Prop)
    (hRL : ∀ n c, RL .mode3 n c ↔ Related (pn n) (pc c) ∧ F n c)
    (hFrame : ∀ n c s t, F n c → F (setN n s) (setC c t))
    (hEnd : ∀ n c s t, F n c → Ended s t → RL .hblank (setN n s) (setC c t))
    (hK : ∀ n i, DotK (pn n) (inN i) → KDot n i)
    (hLocks : ∀ n c, (∃ k, RL k n c) → locksN n = locksC c)
    (hMem : Steps memN memC (fun i j => RAcc i.1 j.1 ∧ i.2 = j.2) RMO KMem RM)
    (hDots : ∀ a b, RAcc a.1 b.1 ∧ RMO a.2 b.2 → List.Forall₂ RDI (dotsN a) (dotsC b))
    (hPer : Steps perN perC
      (fun i j => RAcc i.1 j.1 ∧ List.Forall₂ (fun o p => o = p) i.2 j.2) RSeen KP RP)
    (hBusSide : Steps busN busC RAcc RBO KBus RB)
    (hJoySerial : Steps joyN joyC (fun i j => RAcc i.1 j.1 ∧ i.2 = j.2) RJO KJoy RJ)
    (hApu : Steps apuN apuC RAcc RAO KApu RA)
    {nextN : Cn → X → ((((Lk × MON × List DO) × SeenN) × BON) × JON) × AON → Result Cn}
    {nextC : Cc → X → ((((Lk × MOC × List DO) × SeenC) × BOC) × JOC) × AOC → Result Cc}
    (hOut : ∀ dn dc x y, RC dn dc → x = y → RAcc (outN dn x) (outC dc y))
    (hNext : ∀ dn dc (x y : X) o p, RC dn dc → x = y → BusOut RMO RSeen RBO RJO RAO o p →
      ∃ dn' dc', nextN dn x o = ok dn' ∧ nextC dc y p = ok dc' ∧ RC dn' dc') :
    ∃ K, ∀ n c, RC n.1 c.1 →
      ((((((∃ k, RL k n.2.1.1.1.1.1 c.2.1.1.1.1.1) ∧ RM n.2.1.1.1.1.2 c.2.1.1.1.1.2) ∧
        RP n.2.1.1.1.2 c.2.1.1.1.2) ∧ RB n.2.1.1.2 c.2.1.1.2) ∧ RJ n.2.1.2 c.2.1.2) ∧
        RA n.2.2 c.2.2) →
      Agree
        (close (fun d x => (outN d x, x)) nextN
          (bus locksN dotN dotsN memN ended perN busN joyN apuN))
        (close (fun d x => (outC d x, x)) nextC
          (bus locksC dotC dotsC memC ended perC busC joyC apuC))
        (fun x y => x = y) (BusOut RMO RSeen RBO RJO RAO) K n c :=
  dmg_agrees (phases_with_mode3 kani hOther pn pc setN setC inN inC endN hN hC hGetN hGetC hI F
    hRL hFrame hEnd hK) hLocks hMem hDots hPer hBusSide hJoySerial hApu hOut hNext

end

end ceres_core.sketch.dmg
