import Sketch.Dmg
import Sketch.Instance
open Aeneas Aeneas.Std Result
open ceres_core ceres_core.bisim ceres_core.timer_if ceres_core.dma ceres_core.lcd

/-!
# The whole DMG with the real M-cycle blocks

`dmg_agrees` with its `hPer` given by `hPer_real`: the LCD timing, the
timer, IF and the OAM DMA as Aeneas translates them, on Kani's five claims.
The CPU's access is the real blocks' input (`NetIn`, `CeresIn`) for now:
the other blocks take it as it is.
-/

namespace ceres_core.sketch.dmg
open ceres_core.sketch

theorem dmg_agrees_real
    (kl : KaniLcd) (kt : KaniTimer) (ki : KaniIf) (ks : KaniSeq) (kd : KaniDma)
    {Ln Lc DIn DIc DO Lk Mn Mc MON MOC Bn Bc BON BOC Jn Jc JON JOC X An Ac AON AOC Cn Cc : Type}
    {dotN : Ln → DIn → Result (DO × Ln)} {dotC : Lc → DIc → Result (DO × Lc)}
    {RDI : DIn → DIc → Prop} {KDot : Ln → Lc → DIn → DIc → Prop} {RL : Phase → Ln → Lc → Prop}
    {locksN : Ln → Lk} {locksC : Lc → Lk}
    {memN : Mn → NetIn × Lk → Result (MON × Mn)} {memC : Mc → CeresIn × Lk → Result (MOC × Mc)}
    {RMO : MON → MOC → Prop} {KMem : Mn → Mc → NetIn × Lk → CeresIn × Lk → Prop} {RM : Mn → Mc → Prop}
    {dotsN : NetIn × MON → List DIn} {dotsC : CeresIn × MOC → List DIc} {ended : DO → Bool}
    {busN : Bn → NetIn → Result (BON × Bn)} {busC : Bc → CeresIn → Result (BOC × Bc)}
    {RBO : BON → BOC → Prop} {KBus : Bn → Bc → NetIn → CeresIn → Prop} {RB : Bn → Bc → Prop}
    {joyN : Jn → NetIn × X → Result (JON × Jn)} {joyC : Jc → CeresIn × X → Result (JOC × Jc)}
    {RJO : JON → JOC → Prop} {KJoy : Jn → Jc → NetIn × X → CeresIn × X → Prop} {RJ : Jn → Jc → Prop}
    {apuN : An → NetIn → Result (AON × An)} {apuC : Ac → CeresIn → Result (AOC × Ac)}
    {RAO : AON → AOC → Prop} {KApu : An → Ac → NetIn → CeresIn → Prop} {RA : An → Ac → Prop}
    {outN : Cn → X → NetIn} {outC : Cc → X → CeresIn} {RC : Cn → Cc → Prop}
    {nextN : Cn → X → ((((Lk × MON × List DO) × SeenN) × BON) × JON) × AON → Result Cn}
    {nextC : Cc → X → ((((Lk × MOC × List DO) × SeenC) × BOC) × JOC) × AOC → Result Cc}
    (hPhase : ∀ k, StepsInto dotN dotC RDI (fun o p => o = p) KDot (RL k)
      (fun n c => ∃ k', RL k' n c))
    (hLocks : ∀ n c, (∃ k, RL k n c) → locksN n = locksC c)
    (hMem : Steps memN memC (fun i j => RAccReal i.1 j.1 ∧ i.2 = j.2) RMO KMem RM)
    (hDots : ∀ a b, RAccReal a.1 b.1 ∧ RMO a.2 b.2 → List.Forall₂ RDI (dotsN a) (dotsC b))
    (hBusSide : Steps busN busC RAccReal RBO KBus RB)
    (hJoySerial : Steps joyN joyC (fun i j => RAccReal i.1 j.1 ∧ i.2 = j.2) RJO KJoy RJ)
    (hApu : Steps apuN apuC RAccReal RAO KApu RA)
    (hOut : ∀ dn dc x y, RC dn dc → x = y → RAccReal (outN dn x) (outC dc y))
    (hNext : ∀ dn dc (x y : X) o p, RC dn dc → x = y →
      BusOut RMO BlocksOut RBO RJO RAO o p →
      ∃ dn' dc', nextN dn x o = ok dn' ∧ nextC dc y p = ok dc' ∧ RC dn' dc') :
    ∃ K, ∀ n c, RC n.1 c.1 →
      ((((((∃ k, RL k n.2.1.1.1.1.1 c.2.1.1.1.1.1) ∧ RM n.2.1.1.1.1.2 c.2.1.1.1.1.2) ∧
        BlocksRelated n.2.1.1.1.2 c.2.1.1.1.2) ∧ RB n.2.1.1.2 c.2.1.1.2) ∧
        RJ n.2.1.2 c.2.1.2) ∧ RA n.2.2 c.2.2) →
      Agree
        (close (fun d x => (outN d x, x)) nextN
          (bus locksN dotN dotsN memN ended perNetReal busN joyN apuN))
        (close (fun d x => (outC d x, x)) nextC
          (bus locksC dotC dotsC memC ended perCerReal busC joyC apuC))
        (fun x y => x = y) (BusOut RMO BlocksOut RBO RJO RAO) K n c :=
  dmg_agrees hPhase hLocks hMem hDots (hPer_real kl kt ki ks kd) hBusSide hJoySerial hApu
    hOut hNext

end ceres_core.sketch.dmg
