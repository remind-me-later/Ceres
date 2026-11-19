import Sketch.Dmg
import Ceres.Bisim.Mode3
import Ceres.Bisim.LineRest
open Aeneas Aeneas.Std Result

/-!
# Mode 3 in the outline's line

`dmg_agrees` takes the PPU's line a dot at a time, phase by phase
(`hPhase`), each phase's claim a hypothesis. This file discharges two of
them from Kani's harnesses, through Aeneas's translation: mode 3's first
dot (`start`, from the scan's end: `mode3_start_steps`, on
`netlist_mode3_start`) and the rest of mode 3 (`mode3`: `mode3_dot_steps`,
on the dot harnesses). What stands for them is how the line holds mode 3's
blocks (`Mode3InLine`):

- `hN`, `hC`, `hCs`: in those phases, the netlist's dot is the pipeline's
  dot (its two edges, the same in both phases) with the scan's beside it
  (`netBeside`: the scan, ended, stays ended, Kani's `netlist_scan_ended`),
  and ceres's is the mode 3 loop's dot (its first, or the loop's), on the
  line's state taken apart (`pn`, `pc`) and put back (`setN`, `setC`,
  which `pn` and `pc` take out again: `hGetN`, `hGetC`), the rest of the
  line left as it was; the dot gives whether mode 3 ended (`endN`).
- `hI`: related dot inputs make the same mode 3 input (the mixer's SCX,
  the two edges' inputs; the scan's Ys are the netlist's alone).
- `hRS`, `hRL`: the phases' relations are the blocks' (`Starting`: the scan's
  end as `start_before` has it; `Related`) and the scan's (`ScanIdle`, at
  `163` for the first dot) with the rest of the line's (`F`), which a mode
  3 dot leaves alone (`hFrame`).
- `hCut`: mode 3's states are short of the line's last dot (`903`), the
  open `dmg-lines-not-cut`.
- `hEnd`: once ceres has drawn the line and the netlist's mode 3 stopped
  (`Ended`), the line is in HBlank, as `RL hblank` has it (Kani's
  `mode3-end-background` for the pipeline; HBlank's relation is
  `DmgHBlank.lean`'s).
- `hK`, `hKs`: the line's known difference takes mode 3's (`DotK`,
  `StartK`: the guard turns the dot's input away).

The scan's side of the join, that the scan's last dot leaves the line in
`start`'s relation, is the scan's phase's claim (`scan_then_mode3` gives
it for the whole scan with `R3 := Starting`; a dot's cut of the scan is
open). The dot's output is only whether mode 3 ended: the pixels it draws
are not compared yet.
-/

namespace ceres_core.sketch.dmg
open ceres_core.bisim ceres_core.mode3
open ceres_core.ppu.display.background.relation ceres_netlist.render_edge

open ceres_core.ppu.display.objects.relation ceres_netlist.oam_scan in
/-- The netlist's dot in mode 3: the pipeline's, with the scan's, ended,
beside it (`scanBlock`). -/
def netBeside (x : NetPipeline × (Line × (NetScan × I32))) (i : DotIn × (U8 × U8)) :
    Result ((Bool × Bool) × (NetPipeline × (Line × (NetScan × I32)))) := do
  let (o, n') ← netStep x.1 i.1
  let s' ← ceres_core.rest.scanBlock x.2 i.2
  ok (o, (n', s'))

/-- How the line holds mode 3's blocks, the netlist's pipeline and ceres's
loop, in the phases they run (`start`, `mode3`). -/
structure Mode3InLine {Ln Lc DIn DIc DO : Type}
    (dotN : Ln → DIn → Result (DO × Ln)) (dotC : Lc → DIc → Result (DO × Lc))
    (RDI : DIn → DIc → Prop) (KDot : Ln → Lc → DIn → DIc → Prop) (RL : Phase → Ln → Lc → Prop) where
  pn : Ln → NetPipeline × (ppu.display.objects.relation.Line × (ceres_netlist.oam_scan.NetScan × I32))
  pc : Lc → Mode3Loop
  setN : Ln → NetPipeline × (ppu.display.objects.relation.Line ×
    (ceres_netlist.oam_scan.NetScan × I32)) → Ln
  setC : Lc → Mode3Loop → Lc
  inN : DIn → DotIn × (U8 × U8)
  inC : DIc → DotIn
  endN : Bool → DO
  F : Ln → Lc → Prop
  hGetN : ∀ n s, pn (setN n s) = s
  hGetC : ∀ c t, pc (setC c t) = t
  hI : ∀ i j, RDI i j → (inN i).1 = inC j
  hRS : ∀ n c, RL .start n c ↔ Starting (pn n).1 (pc c) ∧
    (ceres_core.rest.ScanIdle (pn n).2 ∧ (pn n).2.2.2.val = 163) ∧ F n c
  hRL : ∀ n c, RL .mode3 n c ↔ Related (pn n).1 (pc c) ∧ ceres_core.rest.ScanIdle (pn n).2 ∧ F n c
  hFrame : ∀ n c s t, F n c → F (setN n s) (setC c t)
  hN : ∀ n c i, RL .start n c ∨ RL .mode3 n c →
    dotN n i = (do let (o, s) ← netBeside (pn n) (inN i); ok (endN o.2, setN n s))
  hCs : ∀ n c j, RL .start n c →
    dotC c j = (do let (p, t) ← cerStart (pc c) (inC j); ok (endN p, setC c t))
  hC : ∀ n c j, RL .mode3 n c →
    dotC c j = (do let (p, t) ← cerStep (pc c) (inC j); ok (endN p, setC c t))
  hEnd : ∀ n c s t, F n c → Ended s.1 t → ceres_core.rest.ScanIdle s.2 →
    RL .hblank (setN n s) (setC c t)
  hCut : ∀ n c, RL .mode3 n c → (pn n).2.2.2.val ≤ 901
  hKs : ∀ n c i j, StartK (pn n).1 (pc c) (inN i).1 (inC j) → KDot n c i j
  hK : ∀ n c i j, DotK (pn n).1 (pc c) (inN i).1 (inC j) → KDot n c i j

section

variable {Ln Lc DIn DIc DO : Type}
variable {dotN : Ln → DIn → Result (DO × Ln)} {dotC : Lc → DIc → Result (DO × Lc)}
variable {RDI : DIn → DIc → Prop} {KDot : Ln → Lc → DIn → DIc → Prop}
variable {RL : Phase → Ln → Lc → Prop}

/-! ### The CPU's locks on VRAM and OAM, from the PPU's state. -/

variable {Lk : Type} {locksN : Ln → Lk} {locksC : Lc → Lk}

/-! ### The memory: VRAM, OAM, work RAM, the cartridge. -/

variable {Mn Mc AccN AccC MON MOC : Type}
variable {memN : Mn → AccN × Lk → Result (MON × Mn)} {memC : Mc → AccC × Lk → Result (MOC × Mc)}
variable {RAcc : AccN → AccC → Prop} {RMO : MON → MOC → Prop}
variable {KMem : Mn → Mc → AccN × Lk → AccC × Lk → Prop} {RM : Mn → Mc → Prop}

/-! ### The M-cycle's dots, from the access and what the memory gives. -/

variable {dotsN : AccN × MON → List DIn} {dotsC : AccC × MOC → List DIc}
variable {ended : DO → Bool}

/-! ### The M-cycle blocks: LCD timing, timer, IF, OAM DMA. -/

variable {Pn Pc SeenN SeenC : Type}
variable {perN : Pn → AccN × List Bool → Result (SeenN × Pn)}
variable {perC : Pc → AccC × List Bool → Result (SeenC × Pc)}
variable {RSeen : SeenN → SeenC → Prop} {KP : Pn → Pc → AccN × List Bool → AccC × List Bool → Prop}
variable {RP : Pn → Pc → Prop}

/-! ### The rest of the bus: HRAM, boot ROM and undecoded I/O; the joypad
and the serial port; the APU. One step an M-cycle each, taking the access. -/

variable {Bn Bc BON BOC : Type}
variable {busN : Bn → AccN → Result (BON × Bn)} {busC : Bc → AccC → Result (BOC × Bc)}
variable {RBO : BON → BOC → Prop} {KBus : Bn → Bc → AccN → AccC → Prop} {RB : Bn → Bc → Prop}
variable {Jn Jc JON JOC X : Type}
variable {joyN : Jn → AccN × X → Result (JON × Jn)} {joyC : Jc → AccC × X → Result (JOC × Jc)}
variable {RJO : JON → JOC → Prop} {KJoy : Jn → Jc → AccN × X → AccC × X → Prop} {RJ : Jn → Jc → Prop}
variable {An Ac AON AOC : Type}
variable {apuN : An → AccN → Result (AON × An)} {apuC : Ac → AccC → Result (AOC × Ac)}
variable {RAO : AON → AOC → Prop} {KApu : An → Ac → AccN → AccC → Prop} {RA : An → Ac → Prop}

/-! ### The CPU. It takes the outside input `X` (the joypad's buttons, the
serial line), gives its access, and takes back everything the bus gave. -/

variable {Cn Cc : Type}
variable {outN : Cn → X → AccN} {outC : Cc → X → AccC}
variable {RC : Cn → Cc → Prop}


/-- The outputs `DotOut` relates end mode 3 together: the netlist's mode 3
stops at the dot's second edge where ceres had drawn the line. -/
theorem dotOut_ended {s : Bool × Bool} {d : Bool} (h : DotOut s d) : s.2 = d := by
  rcases h with ⟨rfl, -, h⟩ | ⟨rfl, -, h⟩ <;> exact h

/-- Mode 3's first dot in the line, from Kani's start harness: from the
scan's end, a dot leaves the line in mode 3. -/
-- @assumes mapping-mode3-start
theorem start_phase (kStart : KaniMode3Start) (kE : ceres_core.rest.KaniScanEnded)
    (m : Mode3InLine dotN dotC RDI KDot RL) :
    StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL .start)
      (fun n c => ∃ k', RL k' n c) := by
  have b := StepsInto.beside (mode3_start_steps kStart) (b := ceres_core.rest.scanBlock)
    (P := fun s => ceres_core.rest.ScanIdle s ∧ s.2.2.val = 163) (P' := ceres_core.rest.ScanIdle)
    (fun s hs y => ceres_core.rest.scan_block_dot kE hs.1 (le_of_eq_of_le hs.2 (by decide)) y)
  have e := StepsInto.embed b m.pn m.pc m.setN m.setC m.inN m.inC
    (fun o => m.endN o.2) m.endN m.F (fun n c => ∃ k', RL k' n c)
    (fun n c i hr hf => m.hN n c i (Or.inl ((m.hRS n c).2 ⟨hr.1, hr.2, hf⟩)))
    (fun n c j hr hf => m.hCs n c j ((m.hRS n c).2 ⟨hr.1, hr.2, hf⟩))
    (RI' := RDI) m.hI (RO' := fun o p => o = p)
    (fun o p h => congrArg m.endN (dotOut_ended h))
    (fun n c s t hf hr => ⟨.mode3, (m.hRL _ _).2
      ⟨by rw [m.hGetN, m.hGetC]; exact hr.1, by rw [m.hGetN]; exact hr.2,
        m.hFrame n c s t hf⟩⟩)
  intro n c hr i j hij
  obtain ⟨h1, h2, h3⟩ := (m.hRS n c).1 hr
  rcases e n c ⟨⟨h1, h2⟩, h3⟩ i j hij with k | x
  · exact Or.inl (m.hKs n c i j k)
  · exact Or.inr x

/-- The mode 3 phase of the line, from Kani's dot harnesses: a dot from
mode 3's states leaves the line in mode 3 or, the line drawn, in HBlank. -/
-- @assumes mode3-end-background
-- @assumes mode3-window-armed
-- @assumes mode3-window-start
-- @assumes dmg-lines-not-cut
theorem mode3_phase (kani : KaniMode3Dot) (kE : ceres_core.rest.KaniScanEnded)
    (m : Mode3InLine dotN dotC RDI KDot RL) :
    StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL .mode3)
      (fun n c => ∃ k', RL k' n c) := by
  have b := StepsInto.beside (mode3_dot_steps kani) (b := ceres_core.rest.scanBlock)
    (P := fun s => ceres_core.rest.ScanIdle s ∧ s.2.2.val ≤ 901) (P' := ceres_core.rest.ScanIdle)
    (fun s hs y => ceres_core.rest.scan_block_dot kE hs.1 hs.2 y)
  have e := StepsInto.embed b m.pn m.pc m.setN m.setC m.inN m.inC
    (fun o => m.endN o.2) m.endN m.F (fun n c => ∃ k', RL k' n c)
    (fun n c i hr hf => m.hN n c i (Or.inr ((m.hRL n c).2 ⟨hr.1, hr.2.1, hf⟩)))
    (fun n c j hr hf => m.hC n c j ((m.hRL n c).2 ⟨hr.1, hr.2.1, hf⟩))
    (RI' := RDI) m.hI (RO' := fun o p => o = p)
    (fun o p h => congrArg m.endN (dotOut_ended h))
    (by
      rintro n c s t hf ⟨hr | he, hs⟩
      · exact ⟨.mode3, (m.hRL _ _).2
          ⟨by rw [m.hGetN, m.hGetC]; exact hr, by rw [m.hGetN]; exact hs, m.hFrame n c s t hf⟩⟩
      · exact ⟨.hblank, m.hEnd n c s t hf he hs⟩)
  intro n c hr i j hij
  obtain ⟨h1, h2, h3⟩ := (m.hRL n c).1 hr
  rcases e n c ⟨⟨h1, h2, m.hCut n c hr⟩, h3⟩ i j hij with k | x
  · exact Or.inl (m.hK n c i j k)
  · exact Or.inr x

/-- The outline's `hPhase` with mode 3's phases discharged: the other
phases' claims (`hOther`) and how the line holds mode 3's blocks give it
all. -/
theorem phases_with_mode3 (kani : KaniMode3Dot) (kStart : KaniMode3Start)
    (kE : ceres_core.rest.KaniScanEnded)
    (m : Mode3InLine dotN dotC RDI KDot RL)
    (hOther : ∀ k, k ≠ .start → k ≠ .mode3 →
      StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k) (fun n c => ∃ k', RL k' n c)) :
    ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c) := by
  intro k
  cases k with
  | start => exact start_phase kStart kE m
  | mode3 => exact mode3_phase kani kE m
  | scan => exact hOther _ (by intro h; cases h) (by intro h; cases h)
  | hblank => exact hOther _ (by intro h; cases h) (by intro h; cases h)
  | vblank => exact hOther _ (by intro h; cases h) (by intro h; cases h)
  | off => exact hOther _ (by intro h; cases h) (by intro h; cases h)
  | first => exact hOther _ (by intro h; cases h) (by intro h; cases h)

/-- `dmg_agrees` with the line's mode 3 phases taken from Kani's harnesses
(`phases_with_mode3`): the other phases' claims and how the line holds
mode 3's blocks (`Mode3InLine`) stand for `hPhase`. -/
theorem dmg_agrees_mode3 (kani : KaniMode3Dot) (kStart : KaniMode3Start)
    (kE : ceres_core.rest.KaniScanEnded)
    (m : Mode3InLine dotN dotC RDI KDot RL)
    (hOther : ∀ k, k ≠ .start → k ≠ .mode3 →
      StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k) (fun n c => ∃ k', RL k' n c))
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
  dmg_agrees (phases_with_mode3 kani kStart kE m hOther) hLocks hMem hDots hPer hBusSide
    hJoySerial hApu hOut hNext

end

end ceres_core.sketch.dmg
