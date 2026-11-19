import Sketch.DmgScan
open Aeneas Aeneas.Std Result

/-!
# HBlank in the outline's line

`DmgScan.lean` and `DmgMode3.lean` discharge the scan's, mode 3's start's
and mode 3's phases; this file discharges HBlank's (`hblank`) for the
netlist's pixel pipeline, from Kani's claims of its rest (`Rest.lean`):
from mode 3's stop (`Stops`, which mode 3's `Ended` carries) HBlank's first
dot puts the pipeline at rest, and every dot after keeps it there
(`pipe_hblank_dot`); at the line's end, at rest is where the scan's dot
takes it from (`rest_pipe_at`). What stands for it is how the line holds
the pipeline and the rest in HBlank (`HBlankInLine`):

- `hN`, `hC`: in HBlank the line's dot is the pipeline's two edges, on the
  line's state taken apart (`pp`) and put back (`setP`), and the rest of
  the line's dot (`restN`, `restC`: the scan ended, the LCD timing, ceres's
  HBlank; nothing here says what they do); the dot gives `out`, drawing
  nothing.
- `hI`: what the line gives the pipeline in HBlank: a dot's first edge is
  the clock's rise, and no line start, no stop, no reset (the scan ended
  gives neither: Kani's `netlist_scan_ended`; the LCD on).
- `hRH`: HBlank's relation is the pipeline's (`HRest`: at rest, or just
  stopped) with the rest of the line's (`G`).
- `hNext`: after a dot, with the pipeline at rest, the rest of the line is
  still in HBlank (`G`), or the line has ended and is at the scan's start,
  as `RL scan` has it (the scan ended into `net_at_line_end`, ceres's
  search reset, the pipeline at `-8`: `rest_pipe_at`).

HBlank has no known difference: a dot always goes through.
-/

namespace ceres_core.sketch.dmg
open ceres_core.bisim ceres_core.scan ceres_core.rest
open ceres_core.ppu.display.background.relation.rest ceres_netlist.render_edge

/-- How the line holds the netlist's pipeline, and the rest, in HBlank. -/
structure HBlankInLine {Ln Lc DIn DIc DO : Type}
    (dotN : Ln → DIn → Result (DO × Ln)) (dotC : Lc → DIc → Result (DO × Lc))
    (RDI : DIn → DIc → Prop) (RL : Phase → Ln → Lc → Prop) where
  pp : Ln → NetPipeline
  setP : Ln → NetPipeline → Ln
  idleN : DIn → EdgeInputs × EdgeInputs
  restN : Ln → DIn → Ln
  restC : Lc → DIc → Lc
  out : DO
  G : Ln → Lc → Prop
  hGetP : ∀ n p, pp (setP n p) = p
  hRH : ∀ n c, RL .hblank n c ↔ HRest (pp n) ∧ G n c
  hI : ∀ i, (idleN i).1.clk = true ∧ idle_inputs (idleN i).1 = ok true ∧
    idle_inputs (idleN i).2 = ok true
  hN : ∀ n c i, RL .hblank n c →
    dotN n i = (do let (_, p) ← pipeIdle (pp n) (idleN i); ok (out, setP (restN n i) p))
  hC : ∀ n c j, RL .hblank n c → dotC c j = ok (out, restC c j)
  hNext : ∀ n c i j p, RDI i j → G n c → hblank_rest p = ok true →
    G (setP (restN n i) p) (restC c j) ∨ RL .scan (setP (restN n i) p) (restC c j)

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

/-- HBlank's phase of the line: a dot leaves the pipeline at rest, the line
in HBlank or, at its end, at the scan's start. -/
-- @assumes pipeline-rests
theorem hblank_phase (kEnd : KaniRestEnd) (kH : KaniRestHBlank)
    (m : HBlankInLine dotN dotC RDI RL) :
    StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL .hblank)
      (fun n c => ∃ k', RL k' n c) := by
  intro n c hr i j hij
  obtain ⟨hp, hg⟩ := (m.hRH n c).1 hr
  obtain ⟨hc1, h1, h2⟩ := m.hI i
  obtain ⟨p', hp', hr'⟩ := pipe_hblank_dot kEnd kH hp hc1 h1 h2
  refine Or.inr ⟨m.out, m.setP (m.restN n i) p', m.out, m.restC c j, ?_, m.hC n c j hr, rfl, ?_⟩
  · rw [m.hN n c i hr, hp']
    simp
  · rcases m.hNext n c i j p' hij hg hr' with hg' | hs
    · exact ⟨.hblank, (m.hRH _ _).2 ⟨by rw [m.hGetP]; exact Or.inl hr', hg'⟩⟩
    · exact ⟨.scan, hs⟩

/-- The outline's `hPhase` with the scan's, mode 3's and HBlank's phases
discharged: the other phases' claims (`hOther`: VBlank, the LCD off, line
0) and how the line holds the blocks give it all. -/
theorem phases_with_hblank (kStep : KaniScanStep) (kLead : KaniScanLeadIn)
    (kTail : KaniScanTail) (kRest : KaniRestScan) (kEnd : KaniRestEnd) (kH : KaniRestHBlank)
    (kani : ceres_core.mode3.KaniMode3Dot) (kStart : ceres_core.mode3.KaniMode3Start)
    (ms : ScanInLine dotN dotC RDI KDot RL) (m : Mode3InLine dotN dotC RDI KDot RL)
    (mh : HBlankInLine dotN dotC RDI RL)
    (hOther : ∀ k, k ≠ .scan → k ≠ .start → k ≠ .mode3 → k ≠ .hblank →
      StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k) (fun n c => ∃ k', RL k' n c)) :
    ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c) :=
  phases_with_scan kStep kLead kTail kRest kani kStart ms m fun k h1 h2 h3 => by
    cases k with
    | hblank => exact hblank_phase kEnd kH mh
    | scan => exact absurd rfl h1
    | start => exact absurd rfl h2
    | mode3 => exact absurd rfl h3
    | vblank => exact hOther _ h1 h2 h3 (by intro h; cases h)
    | off => exact hOther _ h1 h2 h3 (by intro h; cases h)
    | first => exact hOther _ h1 h2 h3 (by intro h; cases h)

/-- `dmg_agrees` with the line's scan, mode 3 and HBlank phases taken from
Kani's harnesses (`phases_with_hblank`). -/
theorem dmg_agrees_hblank (kStep : KaniScanStep) (kLead : KaniScanLeadIn)
    (kTail : KaniScanTail) (kRest : KaniRestScan) (kEnd : KaniRestEnd) (kH : KaniRestHBlank)
    (kani : ceres_core.mode3.KaniMode3Dot) (kStart : ceres_core.mode3.KaniMode3Start)
    (ms : ScanInLine dotN dotC RDI KDot RL) (m : Mode3InLine dotN dotC RDI KDot RL)
    (mh : HBlankInLine dotN dotC RDI RL)
    (hOther : ∀ k, k ≠ .scan → k ≠ .start → k ≠ .mode3 → k ≠ .hblank →
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
  dmg_agrees (phases_with_hblank kStep kLead kTail kRest kEnd kH kani kStart ms m mh hOther)
    hLocks hMem hDots hPer hBusSide hJoySerial hApu hOut hNext

end

end ceres_core.sketch.dmg
