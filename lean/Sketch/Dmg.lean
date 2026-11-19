import Ceres.Bisim.Feedback
import Ceres.Bisim.Iterate
import Ceres.Bisim.Close
open Aeneas Aeneas.Std Result

/-!
# The whole DMG, speculatively

`Chip.lean` closes the CPU around mode 3 and the M-cycle blocks. This file
goes the rest of the way to the whole DMG, every block the CPU's bus or the
LCD reaches, so that what is missing shows as a hypothesis: each one is a
claim that either has a proof elsewhere (Kani's, or Lean's on Aeneas's
translation) still to be plugged in, or none yet. The composition itself is
proved here, from `Steps.feedback`, `Steps.phases`, `Steps.iterate`,
`Steps.wire` and `Steps.close`, with no `sorry`.

An M-cycle, cut at the netlist's E (`mapping-m-cycle-at-e`):

1. The CPU gives its access (`hOut`), from its state.
2. The video part: the PPU's state gives the CPU's VRAM and OAM locks (a
   Moore output, `hLocks`); the memory (VRAM, OAM, work RAM, the cartridge,
   with the OAM DMA's writes) takes the access under them (`hMem`); the
   PPU's line then runs the M-cycle's four dots (`hDots`) on what the memory
   holds, a dot at a time, in phases (`hScan`, `hMode3`, `hHBlank`,
   `hVBlank`, `hOff`, `hFirst`). A dot gives its pixel and whether mode 3
   ended in it.
3. The M-cycle blocks (LCD timing, timer, IF, OAM DMA: `hPer`, the real
   `blocks_steps` through `Sketch/Instance.lean`) take the access and the
   dots' mode-3 ends.
4. The rest of the bus: HRAM, the boot ROM and the undecoded I/O
   (`hBusSide`), the joypad and the serial port (`hJoySerial`), and the APU
   (`hApu`).
5. The CPU takes what the bus gives back (`hNext`).

The outputs compared are everything the blocks give: the CPU's reads and
the interrupt taken, the LCD's pixels and the APU's samples.
-/

namespace ceres_core.sketch.dmg
open ceres_core.bisim

/-- Where the PPU's line is. -/
inductive Phase
  /-- The OAM scan, the line's first 80 dots. -/
  | scan
  /-- Mode 3's first dot, from the scan's end. -/
  | start
  /-- Mode 3, the pixel pipeline, from its second dot. -/
  | mode3
  /-- HBlank, to the line's end. -/
  | hblank
  /-- VBlank, lines 144 to 153. -/
  | vblank
  /-- The LCD off. -/
  | off
  /-- Line 0 after the LCD is turned on, which has no scan. -/
  | first

section

/-! ### The PPU's line, a dot at a time. A dot's input is what the memory
gives it (VRAM's and OAM's bytes it reads) and the CPU's writes of the PPU's
registers; its output, the pixel it drew if any and whether mode 3 ended. -/

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
variable {OutN OutC : Type}
variable {nextN : Cn → X → OutN → Result Cn} {nextC : Cc → X → OutC → Result Cc}
variable {RC : Cn → Cc → Prop}

/-- The video part: the locks from the PPU's state, the memory under them,
the PPU's dots on what the memory gave. -/
def video {Ls Ms Lk Acc MO DI DO : Type} (locks : Ls → Lk) (dot : Ls → DI → Result (DO × Ls))
    (dots : Acc × MO → List DI) (mem : Ms → Acc × Lk → Result (MO × Ms)) :
    Ls × Ms → Acc → Result ((Lk × MO × List DO) × (Ls × Ms)) :=
  feedback locks (fun s x => iterate dot s (dots x)) mem

/-- The video part steps: the line in phases, a dot at a time over the
M-cycle's dots, around the memory. -/
theorem video_steps
    (hPhase : ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c))
    (hLocks : ∀ n c, (∃ k, RL k n c) → locksN n = locksC c)
    (hMem : Steps memN memC (fun i j => RAcc i.1 j.1 ∧ i.2 = j.2) RMO KMem RM)
    (hDots : ∀ a b, RAcc a.1 b.1 ∧ RMO a.2 b.2 → List.Forall₂ RDI (dotsN a) (dotsC b)) :
    Steps (video locksN dotN dotsN memN) (video locksC dotC dotsC memC) RAcc
      (fun o p => o.1 = p.1 ∧ RMO o.2.1 p.2.1 ∧ List.Forall₂ (fun a b => a = b) o.2.2 p.2.2)
      (feedbackK locksN locksC memN memC
        (fun s t x y => iterateK dotN dotC KDot s t (dotsN x) (dotsC y)) KMem)
      (fun n c => (∃ k, RL k n.1 c.1) ∧ RM n.2 c.2) :=
  Steps.feedback hLocks hMem
    (Steps.comap (Steps.iterate (Steps.phases hPhase)) dotsN dotsC hDots)

/-- Equal lists stay equal through a map. -/
theorem forall2_map_eq {α β : Type} (f : α → β) : ∀ {l1 l2 : List α},
    List.Forall₂ (fun a b => a = b) l1 l2 →
    List.Forall₂ (fun o p => o = p) (l1.map f) (l2.map f) := by
  intro l1 l2 h
  induction h with
  | nil => exact List.Forall₂.nil
  | cons hab _ ih => exact List.Forall₂.cons (congrArg f hab) ih

/-- The PPU: the video part, then the M-cycle blocks taking the dots' mode-3
ends. -/
def ppu {Ls Ms Ps Lk Acc MO DI DO Seen : Type} (locks : Ls → Lk) (dot : Ls → DI → Result (DO × Ls))
    (dots : Acc × MO → List DI) (mem : Ms → Acc × Lk → Result (MO × Ms))
    (ended : DO → Bool) (per : Ps → Acc × List Bool → Result (Seen × Ps)) :
    (Ls × Ms) × Ps → Acc → Result (((Lk × MO × List DO) × Seen) × ((Ls × Ms) × Ps)) :=
  wire (video locks dot dots mem) (fun s x => per s (x.1, x.2.2.2.map ended))

theorem ppu_steps
    (hPhase : ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c))
    (hLocks : ∀ n c, (∃ k, RL k n c) → locksN n = locksC c)
    (hMem : Steps memN memC (fun i j => RAcc i.1 j.1 ∧ i.2 = j.2) RMO KMem RM)
    (hDots : ∀ a b, RAcc a.1 b.1 ∧ RMO a.2 b.2 → List.Forall₂ RDI (dotsN a) (dotsC b))
    (hPer : Steps perN perC
      (fun i j => RAcc i.1 j.1 ∧ List.Forall₂ (fun o p => o = p) i.2 j.2) RSeen KP RP) :
    Steps (ppu locksN dotN dotsN memN ended perN) (ppu locksC dotC dotsC memC ended perC) RAcc
      (fun o p => (o.1.1 = p.1.1 ∧ RMO o.1.2.1 p.1.2.1 ∧
        List.Forall₂ (fun a b => a = b) o.1.2.2 p.1.2.2) ∧ RSeen o.2 p.2)
      (wireK (video locksN dotN dotsN memN) (video locksC dotC dotsC memC)
        (feedbackK locksN locksC memN memC
          (fun s t x y => iterateK dotN dotC KDot s t (dotsN x) (dotsC y)) KMem)
        (fun s t x y => KP s t (x.1, x.2.2.2.map ended) (y.1, y.2.2.2.map ended)))
      (fun n c => ((∃ k, RL k n.1.1 c.1.1) ∧ RM n.1.2 c.1.2) ∧ RP n.2 c.2) := by
  refine Steps.wire (video_steps hPhase hLocks hMem hDots) ?_
  refine Steps.comap hPer (fun x : AccN × (Lk × MON × List DO) => (x.1, x.2.2.2.map ended))
    (fun x : AccC × (Lk × MOC × List DO) => (x.1, x.2.2.2.map ended)) ?_
  rintro ⟨i, o⟩ ⟨j, p⟩ ⟨hij, -, -, hl⟩
  refine ⟨hij, ?_⟩
  exact forall2_map_eq ended hl

/-- The whole bus, on the CPU's access and the outside input: the PPU, then
HRAM's side, the joypad and serial port, the APU. -/
def bus {Ls Ms Ps Bs Js As Lk Acc MO DI DO Seen BO JO AO X : Type}
    (locks : Ls → Lk) (dot : Ls → DI → Result (DO × Ls))
    (dots : Acc × MO → List DI) (mem : Ms → Acc × Lk → Result (MO × Ms))
    (ended : DO → Bool) (per : Ps → Acc × List Bool → Result (Seen × Ps))
    (side : Bs → Acc → Result (BO × Bs)) (joy : Js → Acc × X → Result (JO × Js))
    (apu : As → Acc → Result (AO × As)) :=
  wire (wire (wire (fun s (x : Acc × X) => ppu locks dot dots mem ended per s x.1)
    (fun s x => side s x.1.1)) (fun s x => joy s x.1)) (fun s x => apu s x.1.1)

/-- What the CPU sees of the bus: the PPU's (the locks, the memory's, the
pixels and mode 3's ends; the M-cycle blocks'), HRAM's side's, the
joypad's and serial port's, the APU's. -/
def BusOut (RMO : MON → MOC → Prop) (RSeen : SeenN → SeenC → Prop) (RBO : BON → BOC → Prop)
    (RJO : JON → JOC → Prop) (RAO : AON → AOC → Prop)
    (o : ((((Lk × MON × List DO) × SeenN) × BON) × JON) × AON)
    (p : ((((Lk × MOC × List DO) × SeenC) × BOC) × JOC) × AOC) : Prop :=
  ((((o.1.1.1.1.1 = p.1.1.1.1.1 ∧ RMO o.1.1.1.1.2.1 p.1.1.1.1.2.1 ∧
      List.Forall₂ (fun a b => a = b) o.1.1.1.1.2.2 p.1.1.1.1.2.2) ∧
    RSeen o.1.1.1.2 p.1.1.1.2) ∧ RBO o.1.1.2 p.1.1.2) ∧ RJO o.1.2 p.1.2) ∧ RAO o.2 p.2

theorem bus_steps
    (hPhase : ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c))
    (hLocks : ∀ n c, (∃ k, RL k n c) → locksN n = locksC c)
    (hMem : Steps memN memC (fun i j => RAcc i.1 j.1 ∧ i.2 = j.2) RMO KMem RM)
    (hDots : ∀ a b, RAcc a.1 b.1 ∧ RMO a.2 b.2 → List.Forall₂ RDI (dotsN a) (dotsC b))
    (hPer : Steps perN perC
      (fun i j => RAcc i.1 j.1 ∧ List.Forall₂ (fun o p => o = p) i.2 j.2) RSeen KP RP)
    (hBusSide : Steps busN busC RAcc RBO KBus RB)
    (hJoySerial : Steps joyN joyC (fun i j => RAcc i.1 j.1 ∧ i.2 = j.2) RJO KJoy RJ)
    (hApu : Steps apuN apuC RAcc RAO KApu RA) :
    ∃ K, Steps (bus locksN dotN dotsN memN ended perN busN joyN apuN)
      (bus locksC dotC dotsC memC ended perC busC joyC apuC)
      (fun i j => RAcc i.1 j.1 ∧ i.2 = j.2) (BusOut RMO RSeen RBO RJO RAO) K
      (fun n c => (((((∃ k, RL k n.1.1.1.1.1 c.1.1.1.1.1) ∧ RM n.1.1.1.1.2 c.1.1.1.1.2) ∧
        RP n.1.1.1.2 c.1.1.1.2) ∧ RB n.1.1.2 c.1.1.2) ∧ RJ n.1.2 c.1.2) ∧ RA n.2 c.2) := by
  have hP := ppu_steps (ended := ended) hPhase hLocks hMem hDots hPer
  have h1 := Steps.wire
    (Steps.comap hP Prod.fst Prod.fst (RI' := fun i j => RAcc i.1 j.1 ∧ i.2 = j.2)
      (fun _ _ h => h.1))
    (Steps.comap hBusSide (fun x : (AccN × X) × _ => x.1.1) (fun x : (AccC × X) × _ => x.1.1) (fun _ _ h => h.1.1))
  have h2 := Steps.wire h1
    (Steps.comap hJoySerial (fun x : (AccN × X) × _ => x.1) (fun x : (AccC × X) × _ => x.1) (fun _ _ h => h.1))
  have h3 := Steps.wire h2
    (Steps.comap hApu (fun x : (AccN × X) × _ => x.1.1) (fun x : (AccC × X) × _ => x.1.1) (fun _ _ h => h.1.1))
  exact ⟨_, h3⟩

/-- The two DMGs, the netlist's and ceres's: the CPU closed around the bus.
From states related block by block they agree for ever, every output
compared (the CPU's reads and interrupts, the pixels, the APU's), but
where a block's known difference comes up. -/
theorem dmg_agrees
    (hPhase : ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c))
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
        (fun x y => x = y) (BusOut RMO RSeen RBO RJO RAO) K n c := by
  obtain ⟨K, hB⟩ := bus_steps (ended := ended) hPhase hLocks hMem hDots hPer hBusSide
    hJoySerial hApu
  refine ⟨_, fun n c hc hb => Steps.agree (Steps.close hB ?_ hNext) ⟨hc, hb⟩⟩
  intro dn dc x y hd hxy
  exact ⟨hOut dn dc x y hd hxy, hxy⟩

end

/-! ## What is open

`dmg_agrees` is proved from its hypotheses; `dmg_agrees_real`
(`DmgReal.lean`) puts the real M-cycle blocks in for `hPer`, which leaves
Kani's five claims (`KaniLcd`, `KaniTimer`, `KaniIf`, `KaniSeq`,
`KaniDma`) and these, each with what stands for it today:

- `hPhase`, the PPU's line a dot at a time, phase by phase:
  - `scan`: plugged in, `scan_phase` in `dmg_agrees_scan` (`DmgScan.lean`),
    a dot at a time on Kani's three scan claims (`scan_dot_steps`,
    `Ceres/Bisim/ScanDots.lean`), from the line's end (`scan_starts`) to
    its last edge, `162` (`ScanEnd`, where the netlist's slots hold ceres's
    first ten sprites). What stands for it (`ScanInLine`): the line's state
    as the scan's blocks for the line being scanned (`hN`, `hC`, `hFrame`,
    the get-put laws), the dot's Ys made from the M-cycle's (`hI`), and the
    known difference, OAM giving a dot a Y other than its window's (`hK`).
  - `scan` into `start`: `hEnd` in `ScanInLine`, the store's mapping onto
    mode 3's pipeline and loop (`Starting`), given the pipeline as
    `before_start` has it (proved: the pipeline steps beside the scan,
    `pipe_scan_dot`, fed the outputs Kani shows the scan gives); for a
    whole scan the same hypothesis gives `scan_into_mode3` (`Line.lean`).
  - `start`, mode 3's first dot: plugged in, `start_phase`
    (`DmgMode3.lean`), on Kani's `netlist_mode3_start` through
    `mode3_start_steps`, into `mode3`.
  - `mode3`: plugged in, `mode3_phase` in `dmg_agrees_mode3`
    (`DmgMode3.lean`): Kani's dot harnesses (`KaniMode3Dot`, every related
    pair: `netlist_mode3_dot_split`) through `mode3_dot_steps`, within
    their scope (`lead-in-objects`, `wx0-lead-in-matches` open; WX and
    LCDC's window and object bits held; the pixels' values not compared:
    the dot gives only whether mode 3 ended). What stands for it and
    `start`'s (`Mode3InLine`): the line's state as the pipeline and the
    loop, in those phases, with the rest left alone (`hN`, `hC`, `hCs`,
    `hFrame`, the get-put laws), the dot's mode 3 input made from the
    M-cycle's (`hI`: the mixer's SCX, the two edges' inputs), and the known
    difference, the guard failing on the pair (`hK`, `hKs`: the line's
    takes mode 3's). The rest left alone is not the netlist's line yet:
    its scan, ended, steps in mode 3 too; Kani shows it stays ended and
    gives the pipeline nothing (`netlist_scan_ended`), which `Mode3InLine`
    does not carry.
  - `mode3` into `hblank`: Kani's (`mode3-end-background`); in
    `dmg_agrees_mode3`, `hEnd`: the ended pair (with the dot it came from,
    `Stops`) is in HBlank's relation.
  - `hblank`: plugged in for the netlist's pipeline, `hblank_phase` in
    `dmg_agrees_hblank` (`DmgHBlank.lean`): from mode 3's stop at rest
    (`pipe_hblank_dot`, on Kani's `netlist_rest_*`); what stands for it
    (`HBlankInLine`) is the rest of the line in HBlank (the scan ended,
    ceres's HBlank: nothing compares them), the idle inputs the line gives
    the pipeline, and the line's end into the scan's start (`hNext`: the
    scan's lead-in from there is Kani's, `netlist_scan_lead_in`, and the
    pipeline at rest is where the scan's dot takes it from).
  - `vblank`, `off`: no claim (the pipeline rests in VBlank as in HBlank:
    `netlist_rest_hblank` takes VBlank's inputs).
  - `first` (line 0 after the LCD is turned on): no claim for the PPU (the
    LCD timing's is in `hPer`).
  - The pixel each dot gives: nothing compares the colours (the background's
    and window's tiles, the objects' rows and priority, the palettes).
- `hLocks`: ceres's locks are proved from its own code against measured
  positions (`cpu_locks_*`, `dmg-lines-not-cut` and
  `locks-with-wx-166-glitch` open), not against the netlist's.
- `hMem`: nothing. VRAM, OAM, work RAM and the cartridge under the locks;
  the OAM DMA's reads and writes reach it only once the DMA is wired in front
  of it (its source address is a Moore output: `Steps.feedback` again),
  where `hPer` now gives its OAM write out.
- `hDots`: the clock between an M-cycle and its four dots (the measured
  `mapping-*` assumptions), and `Instance.lean`'s two: mode 3 ends at a
  dot's second edge, at most once in an M-cycle.
- `hBusSide`: Kani's `netlist_bus_side` (HRAM, the boot ROM, FF50, the
  undecoded I/O), not translated.
- `hJoySerial`: nothing; the netlist's side is transcribed (`NetBus`). Their
  interrupt requests do not reach IF's rises in `hPer` yet.
- `hApu`: nothing; the netlist's APU is transcribed, ceres's has no relation.
- `hOut`, `hNext`: the SM83's Kani harnesses (`sm83-not-stopped`,
  `sm83-requests-at-e` open), an instruction at a time with the memory
  answering anything. The closure here is an M-cycle at a time: it needs a
  driver that gives several accesses, each after the bytes read before
  (a `close` over instructions), the harnesses' claim as a function Aeneas
  can take, and which block answers an address as a lemma (now the test
  `memory::tests::dmg_map_is_the_netlists`).
- The states it starts from: `dmg_agrees` holds from related states; that
  the two power on related (or are after the boot ROM) is not stated.

All on the DMG; the transcriptions are replayed against the model, not
proved, and the model is held to dmg-sim by `make check`.
-/

end ceres_core.sketch.dmg
