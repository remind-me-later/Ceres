/-
The timer's two sides written by hand for `bv_decide`, for the pilot that
compares Lean's step proof with Kani's (`netlist_timer_bisimulation`).

`Net` is the netlist's timer, `ceres_netlist::timer::NetTimer` line by line;
`Cer` is ceres's `Clock` on the DMG in single speed, with the fields that
`timing::check::related` holds fixed left out (no DIV-reset delay, not
stopped, no APU delay, no late APU bit: none of them changes in an M-cycle
from a related pair). Plain `BitVec` and `Bool`, and no `Result`: what
`bv_decide` takes as it is.

Every `bif` picks a value, never a whole state: a state chosen by `bif` is
one `simp` cannot take a field of, so the first version, written that way,
was still unfolding one goal after ten minutes. A conditional update goes
through `pick`, field by field.
-/

namespace Pilot.Timer

inductive Reg where
  | div | tima | tma | tac

inductive Acc where
  | none
  | read (r : Reg)
  | write (r : Reg) (v : BitVec 8)

/-- What an access writes, as the timer's write strobes see it. -/
structure Writes where
  div : Bool
  tima : Bool
  tma : Bool
  tac : Bool
  data : BitVec 8

def Writes.nothing : Writes := ⟨false, false, false, false, 0#8⟩

def Acc.writes : Acc → Writes
  | .none => .nothing
  | .read _ => .nothing
  | .write .div v => ⟨true, false, false, false, v⟩
  | .write .tima v => ⟨false, true, false, false, v⟩
  | .write .tma v => ⟨false, false, true, false, v⟩
  | .write .tac v => ⟨false, false, false, true, v⟩

structure Net where
  count : BitVec 16
  tima : BitVec 8
  tma : BitVec 8
  tac : BitVec 8
  intTimer : Bool
  bit7Before : Bool
  timerClock : Bool
  writeTma : Bool
  writeTac : Bool
  latched : BitVec 8
  raceCounts : Bool

/-- `a` where `c`, else `b`, field by field. -/
def Net.pick (c : Bool) (a b : Net) : Net where
  count := bif c then a.count else b.count
  tima := bif c then a.tima else b.tima
  tma := bif c then a.tma else b.tma
  tac := bif c then a.tac else b.tac
  intTimer := bif c then a.intTimer else b.intTimer
  bit7Before := bif c then a.bit7Before else b.bit7Before
  timerClock := bif c then a.timerClock else b.timerClock
  writeTma := bif c then a.writeTma else b.writeTma
  writeTac := bif c then a.writeTac else b.writeTac
  latched := bif c then a.latched else b.latched
  raceCounts := bif c then a.raceCounts else b.raceCounts

/-- `NetTimer::clock_of`: the divider's bit that TAC's select picks, if TAC
enables the timer. -/
def clockOf (tac sel : BitVec 8) (count : BitVec 16) : Bool :=
  let s := sel &&& 3#8
  let bit :=
    bif s == 0#8 then count.getLsbD 7
    else bif s == 1#8 then count.getLsbD 1
    else bif s == 2#8 then count.getLsbD 3
    else count.getLsbD 5
  (tac &&& 4#8 != 0#8) && bit

/-- `NetTimer::settle`. -/
def Net.settle (n : Net) (writeTima t4 : Bool) (data : BitVec 8) (clock : Bool) : Net :=
  let load := n.intTimer || (writeTima && !t4)
  let falls := n.timerClock && !clock
  { n with
    tima :=
      bif load then (bif writeTima then data else n.tma)
      else bif falls then n.tima + 1#8 else n.tima
    bit7Before := !load && n.bit7Before
    timerClock := clock }

/-- `NetTimer::phase` for B to H: `w` is what the access writes in this
phase (nothing outside E, F and G), `t4` (G, H) the write phase. -/
def Net.phase (n : Net) (w : Writes) (t4 : Bool) : Net :=
  let n := { n with count := bif w.div then 0#16 else n.count }
  let n := { n with
    tma := bif n.writeTma && !w.tma then n.latched else n.tma
    writeTma := w.tma }
  let tacFalls := n.writeTac && !w.tac
  let new := n.latched &&& 7#8
  let between := clockOf new n.tac n.count
  let n := Net.pick (tacFalls && (n.raceCounts || n.timerClock))
    (n.settle w.tima t4 w.data between) n
  let n := { n with tac := bif tacFalls then new else n.tac }
  let clock := clockOf n.tac n.tac n.count
  let n := { n with
    writeTac := w.tac
    latched := bif w.div || w.tima || w.tma || w.tac then w.data else n.latched }
  n.settle w.tima t4 w.data clock

/-- `NetTimer::edge_a`. -/
def Net.edgeA (n : Net) : Net :=
  let overflow := n.bit7Before && (n.tima &&& 0x80#8 == 0#8)
  let n := { n with
    bit7Before := bif n.intTimer then n.bit7Before else n.tima &&& 0x80#8 != 0#8 }
  let n := { n with intTimer := overflow, count := n.count + 1#16 }
  n.settle false false 0#8 (clockOf n.tac n.tac n.count)

/-- `NetTimer::read`. -/
def Net.read (n : Net) : Reg → BitVec 8
  | .div => (n.count >>> 6).truncate 8
  | .tima => n.tima
  | .tma => n.tma
  | .tac => n.tac

/-- `NetTimer::m_cycle`: whether there was a read and what it gave, whether
the interrupt rose, and the timer after. -/
def Net.mCycle (n : Net) (acc : Acc) : (Bool × BitVec 8) × Bool × Net :=
  let w := acc.writes
  let n := n.phase w false
  let n := n.phase w false
  let n := n.phase w true
  let n := n.phase .nothing true
  let read : Bool × BitVec 8 :=
    match acc with
    | .read r => (true, n.read r)
    | _ => (false, 0#8)
  let before := n.intTimer
  let n := n.edgeA
  let rose := !before && n.intTimer
  let n := n.phase .nothing false
  let n := n.phase .nothing false
  let n := n.phase .nothing false
  (read, rose, n)

/-- Ceres's `Clock`, as the step uses it. -/
structure Cer where
  div : BitVec 16
  tac : BitVec 8
  tima : BitVec 8
  tma : BitVec 8
  reload : BitVec 8
  irq : BitVec 8

/-- `a` where `c`, else `b`, field by field. -/
def Cer.pick (c : Bool) (a b : Cer) : Cer where
  div := bif c then a.div else b.div
  tac := bif c then a.tac else b.tac
  tima := bif c then a.tima else b.tima
  tma := bif c then a.tma else b.tma
  reload := bif c then a.reload else b.reload
  irq := bif c then a.irq else b.irq

/-- `Clock::sys_clk_tac_mux`. -/
def mux (tac : BitVec 8) : BitVec 16 :=
  let s := tac &&& 3#8
  bif s == 0#8 then 0x200#16 else bif s == 1#8 then 0x8#16
  else bif s == 2#8 then 0x20#16 else 0x80#16

/-- `Clock::inc_tima` on the DMG. -/
def Cer.incTima (c : Cer) : Cer :=
  let tima := c.tima + 1#8
  let over := tima == 0#8
  { c with
    tima := bif over then c.tma else tima
    reload := bif over then 4#8 else c.reload
    irq := bif over then 3#8 else c.irq }

/-- `Clock::set_div`, with no APU delay (its APU hooks do nothing here). -/
def Cer.setDiv (c : Cer) (val : BitVec 16) : Cer :=
  let triggers := c.div &&& ~~~val
  let c := Cer.pick ((c.tac &&& 4#8 != 0#8) && (triggers &&& mux c.tac != 0#16)) c.incTima c
  { c with div := val }

/-- The reload countdown in a T-cycle of `Clock::run`. -/
def Cer.reloadNext (r : BitVec 8) : BitVec 8 :=
  let down := r - 1#8
  let up := r + 1#8
  bif r == 0#8 then r
  else bif r ≤ 4#8 then (bif down == 0#8 then 5#8 else down)
  else bif 8#8 < up then 0#8 else up

/-- A T-cycle of `Clock::run`: whether the interrupt is requested in it. -/
def Cer.tCycle (c : Cer) : Bool × Cer :=
  let counting := c.irq != 0#8
  let irq := bif counting then c.irq - 1#8 else c.irq
  let fired := counting && irq == 0#8
  (fired, Cer.setDiv { c with reload := Cer.reloadNext c.reload, irq } (c.div + 1#16))

/-- `ceres_access`: the read, and the clock after the access. -/
def Cer.access (c : Cer) : Acc → (Bool × BitVec 8) × Cer
  | .none => ((false, 0#8), c)
  | .read .div => ((true, (c.div >>> 8).truncate 8), c)
  | .read .tima =>
    ((true, bif (1#8 ≤ c.reload) && (c.reload ≤ 4#8) then 0#8 else c.tima), c)
  | .read .tma => ((true, c.tma), c)
  | .read .tac => ((true, c.tac &&& 7#8), c)
  | .write .div _ => ((false, 0#8), c.setDiv 0#16)
  | .write .tima v =>
    ((false, 0#8), Cer.pick (5#8 ≤ c.reload) c { c with reload := 0#8, irq := 0#8, tima := v })
  | .write .tma v =>
    ((false, 0#8), { c with tma := v, tima := bif c.reload != 0#8 then v else c.tima })
  | .write .tac v =>
    let inc :=
      (c.tac &&& 4#8 != 0#8) && (c.div &&& mux c.tac != 0#16)
        && ((v &&& 4#8 == 0#8) || (c.div &&& mux v == 0#16))
    ((false, 0#8), { Cer.pick inc c.incTima c with tac := v })

/-- One T-cycle of `ceres_m_cycle`'s loop, the `t`th: the first T-cycle with
the interrupt (as a flag and an index), and whether it came twice. -/
def Cer.count (t : BitVec 8) (s : (Bool × BitVec 8) × Bool × Cer) :
    (Bool × BitVec 8) × Bool × Cer :=
  let ((first, at_), twice, c) := s
  let (fired, c) := c.tCycle
  ((first || fired, bif fired && !first then t else at_), twice || (fired && first), c)

/-- `ceres_m_cycle`: the read, the first T-cycle with the interrupt (as a
flag and an index), whether it came twice, and the clock after. -/
def Cer.mCycle (c : Cer) (acc : Acc) :
    (Bool × BitVec 8) × (Bool × BitVec 8) × Bool × Cer :=
  let (read, c) := c.access acc
  let s := Cer.count 0#8 ((false, 0#8), false, c)
  let s := Cer.count 1#8 s
  let s := Cer.count 2#8 s
  let s := Cer.count 3#8 s
  let (irq, twice, c) := s
  (read, irq, twice, c)

/-- `timing::check::related`, on the fields the models keep. -/
def related (n : Net) (c : Cer) : Bool :=
  let sameRegs :=
    (c.div == n.count * 4#16) && (c.tma == n.tma) && (c.tac &&& 7#8 == n.tac)
      && (n.timerClock == clockOf n.tac n.tac n.count)
      && !n.writeTma && !n.writeTac && !n.raceCounts
  let overflowed := n.bit7Before && (n.tima &&& 0x80#8 == 0#8)
  let timer :=
    bif n.intTimer then
      (c.reload == 5#8) && (c.irq == 0#8) && (n.tima == n.tma) && !n.bit7Before
        && (c.tima == n.tma)
    else bif overflowed then
      (c.reload == 4#8) && (c.irq == 3#8) && (n.tima == 0#8) && (c.tima == n.tma)
    else (c.reload == 0#8) && (c.irq == 0#8) && (c.tima == n.tima)
  sameRegs && timer

/-- `timing::check::known_difference`. -/
def knownDifference (n : Net) (acc : Acc) : Bool :=
  let now := clockOf n.tac n.tac n.count
  let (tac, count, glitch) : BitVec 8 × BitVec 16 × Bool :=
    match acc with
    | .write .tac v =>
      let new := v &&& 7#8
      (new, n.count, now && !(clockOf new n.tac n.count && clockOf new new n.count))
    | .write .div _ => (n.tac, 0#16, now)
    | _ => (n.tac, n.count, false)
  let tma := match acc with
    | .write .tma v => v
    | _ => n.tma
  let overflowed := !n.intTimer && n.bit7Before && (n.tima &&& 0x80#8 == 0#8)
  let fallsAtEdge := clockOf tac tac count && !clockOf tac tac (count + 1#16)
  let lostTick := (n.intTimer && glitch) || (overflowed && (glitch || fallsAtEdge))
  let unseenOverflow :=
    (!n.intTimer && !n.bit7Before && (n.tima == 0xFF#8) && glitch)
      || (n.intTimer && (tma == 0xFF#8) && fallsAtEdge)
  lostTick || unseenOverflow

/-- `timing::check::step_agrees`. -/
def stepAgrees (n : Net) (c : Cer) (acc : Acc) : Bool :=
  !related n c || knownDifference n acc ||
    let (netRead, netIrq, n) := n.mCycle acc
    let (cerRead, (cerIrq, at_), twice, c) := c.mCycle acc
    (netRead.1 == cerRead.1) && (netRead.2 == cerRead.2)
      &&(bif cerIrq then netIrq && (at_ == 2#8) else !netIrq)
      && !twice && related n c

end Pilot.Timer
