import Sketch.DmgMode3
import Ceres.Bisim.ScanDots
open Aeneas Aeneas.Std Result

/-!
# The OAM scan in the outline's line

`DmgMode3.lean` discharges mode 3's phases (`start`, `mode3`); this file
discharges the scan's (`scan`) from `scan_dot_steps` (Kani's three scan
claims, a dot at a time: `Ceres/Bisim/ScanDots.lean`), leaving how the line
holds the scan's blocks (`ScanInLine`):

- `hN`, `hC`: in the scan, the line's dot is the scan's (the netlist's two
  edges, ceres's read at the dots that have one) for the line being
  scanned (`lineN`, `lineC`: LY and LCDC bit 2, the same on both sides,
  `hLine`), on the line's state taken apart (`pn`, `pc`) and put back
  (`setN`, `setC`: `hGetN`, `hGetC`, which keep the line, `hKeepN`,
  `hKeepC`), the rest left alone (`hFrame`); the dot draws nothing and
  mode 3 does not end in it (`out`).
- `hI`: related dot inputs make the same Ys, OAM's at the scan's index.
- `hRS`: the scan phase's relation is the scan's (`ScanRel`) with the rest
  of the line's (`F`).
- `hEnd`: once the scan's last edge has run (`ScanEnd`: the netlist's slots
  hold ceres's first ten sprites, `scan_end_saves`), the line is at mode
  3's start, as `RL start` has it: the store's mapping onto mode 3's
  pipeline and loop (`scan_into_mode3`'s `hStart`).
- `hK`: the line's known difference takes the scan's (`ScanDotK`: OAM gives
  a dot a Y that is not its window's).

Where the scan starts, the line's end (`scan_starts`), is HBlank's or
VBlank's phase to reach, still open.
-/

namespace ceres_core.sketch.dmg
open ceres_core.bisim ceres_core.scan
open ceres_core.ppu.display.objects.relation

/-- How the line holds the scan's blocks, the netlist's scan and OAM
latches and ceres's search, in the scan phase. -/
structure ScanInLine {Ln Lc DIn DIc DO : Type}
    (dotN : Ln → DIn → Result (DO × Ln)) (dotC : Lc → DIc → Result (DO × Lc))
    (RDI : DIn → DIc → Prop) (KDot : Ln → Lc → DIn → DIc → Prop) (RL : Phase → Ln → Lc → Prop) where
  lineN : Ln → Line
  lineC : Lc → Line
  pn : Ln → NetSide × I32
  pc : Lc → CeresSide × I32
  setN : Ln → NetSide × I32 → Ln
  setC : Lc → CeresSide × I32 → Lc
  inN : DIn → ScanIn
  inC : DIc → ScanIn
  out : DO
  F : Ln → Lc → Prop
  hGetN : ∀ n s, pn (setN n s) = s
  hGetC : ∀ c t, pc (setC c t) = t
  hKeepN : ∀ n s, lineN (setN n s) = lineN n
  hKeepC : ∀ c t, lineC (setC c t) = lineC c
  hLine : ∀ n c, F n c → lineN n = lineC c
  hI : ∀ i j, RDI i j → inN i = inC j
  hRS : ∀ n c, RL .scan n c ↔ ScanRel (lineN n) (pn n) (pc c) ∧ F n c
  hFrame : ∀ n c s t, F n c → F (setN n s) (setC c t)
  hN : ∀ n c i, RL .scan n c →
    dotN n i = (do let (_, s) ← netScan (lineN n) (pn n) (inN i); ok (out, setN n s))
  hC : ∀ n c j, RL .scan n c →
    dotC c j = (do let (_, t) ← cerScan (lineC c) (pc c) (inC j); ok (out, setC c t))
  hEnd : ∀ n c s t, F n c → ScanEnd (lineN n) s t → RL .start (setN n s) (setC c t)
  hK : ∀ n c i j, ScanDotK (lineN n) (pn n) (pc c) (inN i) (inC j) → KDot n c i j

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

/-- The scan phase of the line, from Kani's scan claims a dot at a time:
a dot from the scan's states leaves the line in the scan or, its last edge
run, at mode 3's start. -/
-- @assumes scan-lead-in-agrees
-- @assumes scan-step-agrees
-- @assumes scan-tail-agrees
theorem scan_phase (kStep : KaniScanStep) (kLead : KaniScanLeadIn) (kTail : KaniScanTail)
    (m : ScanInLine dotN dotC RDI KDot RL) :
    StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL .scan)
      (fun n c => ∃ k', RL k' n c) := by
  intro n c hr i j hij
  obtain ⟨hs, hf⟩ := (m.hRS n c).1 hr
  rcases scan_dot_steps kStep kLead kTail (m.lineN n) (m.pn n) (m.pc c) hs (m.inN i) (m.inC j)
      (m.hI i j hij) with k | ⟨o, s, p, t, hn, hc, -, hr'⟩
  · exact Or.inl (m.hK n c i j k)
  · refine Or.inr ⟨m.out, m.setN n s, m.out, m.setC c t, ?_, ?_, rfl, ?_⟩
    · rw [m.hN n c i hr, hn]
      simp
    · rw [m.hC n c j hr, ← m.hLine n c hf, hc]
      simp
    · rcases hr' with hr' | he
      · refine ⟨.scan, (m.hRS _ _).2 ⟨?_, m.hFrame n c s t hf⟩⟩
        rw [m.hGetN, m.hGetC, m.hKeepN]
        exact hr'
      · exact ⟨.start, m.hEnd n c s t hf he⟩

/-- The outline's `hPhase` with the scan's and mode 3's phases discharged:
the other phases' claims (`hOther`: HBlank, VBlank, the LCD off, line 0)
and how the line holds the scan's and mode 3's blocks give it all. -/
theorem phases_with_scan (kStep : KaniScanStep) (kLead : KaniScanLeadIn) (kTail : KaniScanTail)
    (kani : ceres_core.mode3.KaniMode3Dot) (kStart : ceres_core.mode3.KaniMode3Start)
    (ms : ScanInLine dotN dotC RDI KDot RL) (m : Mode3InLine dotN dotC RDI KDot RL)
    (hOther : ∀ k, k ≠ .scan → k ≠ .start → k ≠ .mode3 →
      StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k) (fun n c => ∃ k', RL k' n c)) :
    ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c) :=
  phases_with_mode3 kani kStart m fun k h1 h2 => by
    cases k with
    | scan => exact scan_phase kStep kLead kTail ms
    | start => exact absurd rfl h1
    | mode3 => exact absurd rfl h2
    | hblank => exact hOther _ (by intro h; cases h) h1 h2
    | vblank => exact hOther _ (by intro h; cases h) h1 h2
    | off => exact hOther _ (by intro h; cases h) h1 h2
    | first => exact hOther _ (by intro h; cases h) h1 h2

/-- `dmg_agrees` with the line's scan and mode 3 phases taken from Kani's
harnesses (`phases_with_scan`). -/
theorem dmg_agrees_scan (kStep : KaniScanStep) (kLead : KaniScanLeadIn) (kTail : KaniScanTail)
    (kani : ceres_core.mode3.KaniMode3Dot) (kStart : ceres_core.mode3.KaniMode3Start)
    (ms : ScanInLine dotN dotC RDI KDot RL) (m : Mode3InLine dotN dotC RDI KDot RL)
    (hOther : ∀ k, k ≠ .scan → k ≠ .start → k ≠ .mode3 →
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
  dmg_agrees (phases_with_scan kStep kLead kTail kani kStart ms m hOther) hLocks hMem hDots hPer
    hBusSide hJoySerial hApu hOut hNext

end

end ceres_core.sketch.dmg
