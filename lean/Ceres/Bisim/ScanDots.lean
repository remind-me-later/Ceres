import Ceres.Bisim.Scan
import Ceres.Bisim.Feedback
open Aeneas Aeneas.Std Result ceres_core.bisim

/-!
# The OAM scan, a dot at a time

Kani proves the scan in pieces of a sprite's window, two dots
(`Scan.lean`: the lead-in, a window, the tail), and the outline's line
steps a dot at a time (`Sketch/Dmg.lean`). Here the scan is a block whose
step is a dot (`netScan`, `cerScan`): the netlist's two edges, from the
half dot its state holds, OAM giving a Y at each (`ScanIn`); ceres's read
of a sprite, at the dots that have one (`reads`: from `-3` to `153`, every
other dot).

A dot ends inside a window, where Kani relates nothing, so the pairs
between dots are those reached from a pair Kani does relate: a prefix of
the lead-in from the line's end (`LeadW`), or one or three of a window's
edges from a pair related at its boundary (`Mid1`, `Mid3`, on `Anchor`;
windows 39 and 40 are the tail's, from a pair related at 39). Kani's
claims give that these runs go through (`lead_ok`, `window_ok`, `tail_ok`)
and where they lead; `scan_dot_steps` is the dot as a block, from the line's
end (`scan_starts`) to the scan's last edge, `162` (`ScanEnd`), where the
netlist's slots hold ceres's first ten sprites (`scan_end_saves`).

The Ys a dot is given are its input, and OAM gives the same one over a
window (the CPU does not reach OAM in the scan): a dot given another is
the known difference (`ScanDotK`), as Kani's pieces take one Y a window.
The tail (`tail`) runs to edge `162` so that the scan's last dot is
whole; mode 3's first is `163` and `164`.
-/

namespace ceres_core.scan
open ceres_core.ppu.display.objects.relation

/-- An I32 sum in range, as a value. -/
theorem i32_add {x y : I32} (h1 : IScalar.min .I32 ≤ x.val + y.val)
    (h2 : x.val + y.val ≤ IScalar.max .I32) : ∃ z : I32, x + y = ok z ∧ z.val = x.val + y.val :=
  spec_ok_exists (IScalar.add_spec h1 h2)

/-- The netlist's two edges of a dot, from `h`, OAM giving `y1` then `y2`. -/
def edge2 (line : Line) (n : NetSide) (h : I32) (y1 y2 : U8) : Result NetSide := do
  let n1 ← net_edge n h line y1
  let h1 ← h + 1#i32
  net_edge n1 h1 line y2

/-- A window as its first edge, a dot's two and its last. -/
theorem net_window_split (n : NetSide) (line : Line) (j : U8) (y : U8) (hj : j.val ≤ 100)
    (h h1 h3 : I32) (hh : h.val = 4 * j.val - 2) (hh1 : h1.val = h.val + 1)
    (hh3 : h3.val = h.val + 3) :
    net_window n line j y = (do
      let n1 ← net_edge n h line y
      let n2 ← edge2 line n1 h1 y y
      net_edge n2 h3 line y) := by
  unfold net_window edge2
  obtain ⟨i, hi, hiv⟩ := spec_ok_exists
    (UScalar.hcast_inBounds_spec (tgt_ty := .I32) j (by scalar_tac))
  obtain ⟨i1, hi1, hi1v⟩ := spec_ok_exists
    (IScalar.mul_spec (x := 4#i32) (y := i) (by scalar_tac) (by scalar_tac))
  obtain ⟨h', hh', hh'v⟩ := spec_ok_exists
    (IScalar.sub_spec (x := i1) (y := 2#i32) (by scalar_tac) (by scalar_tac))
  have e0 : h' = h := IScalar.eq_of_val_eq (by scalar_tac)
  subst e0
  obtain ⟨a, ha, hav⟩ := i32_add (x := h') (y := 1#i32) (by scalar_tac) (by scalar_tac)
  obtain ⟨b, hb, hbv⟩ := i32_add (x := h') (y := 2#i32) (by scalar_tac) (by scalar_tac)
  obtain ⟨c, hc, hcv⟩ := i32_add (x := h') (y := 3#i32) (by scalar_tac) (by scalar_tac)
  obtain ⟨d, hd, hdv⟩ := i32_add (x := h1) (y := 1#i32) (by scalar_tac) (by scalar_tac)
  have e1 : a = h1 := IScalar.eq_of_val_eq (by scalar_tac)
  have e2 : d = b := IScalar.eq_of_val_eq (by scalar_tac)
  have e3 : c = h3 := IScalar.eq_of_val_eq (by scalar_tac)
  subst e1 e2 e3
  simp only [hi, hi1, hh', ha, hb, hc, hd, bind_ok]
  simp

/-- An I32 sum, given its value. -/
theorem i32_add_eq (x y z : I32) (hz : z.val = x.val + y.val)
    (h1 : IScalar.min .I32 ≤ x.val + y.val) (h2 : x.val + y.val ≤ IScalar.max .I32) :
    x + y = ok z := by
  obtain ⟨w, hw, hwv⟩ := i32_add h1 h2
  rw [hw, IScalar.eq_of_val_eq (hwv.trans hz.symm)]

/-- A dot's two edges, the second's half dot given. -/
theorem edge2_eq (line : Line) (n : NetSide) (h a : I32) (y1 y2 : U8)
    (ha : a.val = h.val + 1) (_hb : -1000 ≤ h.val ∧ h.val ≤ 1000) :
    edge2 line n h y1 y2 = (do let n1 ← net_edge n h line y1; net_edge n1 a line y2) := by
  unfold edge2
  rw [i32_add_eq h 1#i32 a (by scalar_tac) (by scalar_tac) (by scalar_tac)]
  simp

/-! ### The lead-in, a dot at a time -/

/-- The lead-in's dots, the half dot each starts at (`-7` to `3`). -/
def leadH : Nat → I32
  | 0 => (-7)#i32
  | 1 => (-5)#i32
  | 2 => (-3)#i32
  | 3 => (-1)#i32
  | 4 => 1#i32
  | _ => 3#i32

/-- The Ys OAM gives at a lead-in dot's two edges. -/
def leadYs (ys : LeadYs) : Nat → U8 × U8
  | 0 => (ys.y39, ys.y0)
  | 1 => (ys.y0, ys.y0)
  | 2 => (ys.y0, ys.y1)
  | 3 => (ys.y1, ys.y1)
  | 4 => (ys.y1, ys.y2)
  | _ => (ys.y2, ys.y2)

/-- The netlist over the lead-in's first `k` dots (up to 6). -/
def leadNet (line : Line) (n0 : NetSide) (ys : LeadYs) : Nat → Result NetSide
  | 0 => ok n0
  | k + 1 => do
    let n ← leadNet line n0 ys k
    edge2 line n (leadH k) (leadYs ys k).1 (leadYs ys k).2

/-- Whether ceres reads a sprite in the dot from half dot `h`: at `4k - 3`,
sprite `k`'s, from `-3` to `153` (`mapping-scan-reads`). -/
def reads (h : I32) : Bool := decide (h.val % 4 = 1 ∧ -3 ≤ h.val ∧ h.val ≤ 153)

/-- Ceres over the lead-in's first `k` dots: its reads of sprites 0 and 1,
in dots 2 and 4. -/
def leadCer (line : Line) (c0 : CeresSide) (ys : LeadYs) : Nat → Result CeresSide
  | 0 | 1 | 2 => ok c0
  | 3 | 4 => ceres_read c0 line ys.y0
  | _ => do let c ← ceres_read c0 line ys.y0; ceres_read c line ys.y1

/-- The lead-in, as its six dots, the edge `5` and ceres's three reads. -/
theorem lead_in_dots (n0 : NetSide) (c0 : CeresSide) (line : Line) (ys : LeadYs) :
    lead_in n0 c0 line ys = (do
      let n ← leadNet line n0 ys 6
      let n' ← net_edge n 5#i32 line ys.y2
      let c ← leadCer line c0 ys 6
      let c' ← ceres_read c line ys.y2
      ok (n', c')) := by
  have a0 := i32_add_eq (-7)#i32 1#i32 (-6)#i32 (by simp) (by scalar_tac) (by scalar_tac)
  have a1 := i32_add_eq (-5)#i32 1#i32 (-4)#i32 (by simp) (by scalar_tac) (by scalar_tac)
  have a2 := i32_add_eq (-3)#i32 1#i32 (-2)#i32 (by simp) (by scalar_tac) (by scalar_tac)
  have a3 := i32_add_eq (-1)#i32 1#i32 0#i32 (by simp) (by scalar_tac) (by scalar_tac)
  have a4 := i32_add_eq 1#i32 1#i32 2#i32 (by simp) (by scalar_tac) (by scalar_tac)
  have a5 := i32_add_eq 3#i32 1#i32 4#i32 (by simp) (by scalar_tac) (by scalar_tac)
  simp [lead_in, leadNet, leadCer, leadH, leadYs, edge2, a0, a1, a2, a3, a4, a5]

/-! ### What runs, from Kani's claims -/

/-- From the line's end, the lead-in runs, and leaves the pair related at
window 2 (`netlist_scan_lead_in`). -/
-- @assumes scan-lead-in-agrees
theorem lead_ok (kLead : KaniScanLeadIn) {count ns held line n0 c0} (ys : LeadYs)
    (hc : count ≤ ceres_netlist.oam_scan.SLOTS) (hl : line.ly < 144#u8)
    (hn : net_at_line_end count ns held = ok n0) (hce : ceres_at_line_end held = ok c0) :
    ∃ N N' C C', leadNet line n0 ys 6 = ok N ∧ net_edge N 5#i32 line ys.y2 = ok N' ∧
      leadCer line c0 ys 6 = ok C ∧ ceres_read C line ys.y2 = ok C' ∧
      related N' C' line 2#u8 = ok true := by
  obtain ⟨⟨N', C'⟩, hp, hr⟩ := spec_ok_exists (lead_in_related kLead (ys := ys) hc hl hn hce)
  rw [lead_in_dots] at hp
  obtain ⟨N, hN, hp⟩ := bind_eq_ok hp
  obtain ⟨N1, hN1, hp⟩ := bind_eq_ok hp
  obtain ⟨C, hC, hp⟩ := bind_eq_ok hp
  obtain ⟨C1, hC1, hp⟩ := bind_eq_ok hp
  simp only [Result.ok.injEq, Prod.mk.injEq] at hp
  obtain ⟨rfl, rfl⟩ := hp
  exact ⟨N, N1, C, C1, hN, hN1, hC, hC1, hr⟩

/-- A prefix of a run that ends runs. -/
theorem leadNet_prefix {line n0 ys v} : ∀ {m k : Nat}, leadNet line n0 ys m = ok v → k ≤ m →
    ∃ u, leadNet line n0 ys k = ok u := by
  intro m
  induction m generalizing v with
  | zero => intro k h hk; exact ⟨v, by rw [Nat.le_zero.1 hk]; exact h⟩
  | succ m ih =>
    intro k h hk
    rcases Nat.lt_or_eq_of_le hk with hk | rfl
    · simp only [leadNet] at h
      obtain ⟨u, hu, -⟩ := bind_eq_ok h
      exact ih hu (by omega)
    · exact ⟨v, h⟩

/-- Ceres over a lead-in dot: its read, in dots 2 and 4. -/
theorem leadCer_succ (line : Line) (c0 : CeresSide) (ys : LeadYs) (k : Nat) (hk : k < 6) :
    leadCer line c0 ys (k + 1) = (do
      let c ← leadCer line c0 ys k
      if reads (leadH k) then ceres_read c line (leadYs ys k).1 else ok c) := by
  match k, hk with
  | 0, _ | 1, _ | 2, _ | 3, _ | 4, _ | 5, _ => simp [leadCer, reads, leadH, leadYs]

/-- A window `j` from 2 to 38, from a related pair: it runs, and ceres's
read with it, into the pair related at `j + 1` (`netlist_scan_step`). -/
-- @assumes scan-step-agrees
theorem window_ok (kStep : KaniScanStep) {line j n0 c} (y : U8)
    (hr : related n0 c line j = ok true) (h2 : 2 ≤ j.val) (h38 : j.val ≤ 38) :
    ∃ n4 c', net_window n0 line j y = ok n4 ∧ ceres_read c line y = ok c' ∧
      ∀ j' : U8, j'.val = j.val + 1 → related n4 c' line j' = ok true := by
  obtain ⟨⟨n4, c'⟩, hs, hpost⟩ := spec_ok_exists (window kStep (y := y) hr h2 h38)
  unfold step at hs
  obtain ⟨n1, hn1, hs⟩ := bind_eq_ok hs
  obtain ⟨c1, hc1, hs⟩ := bind_eq_ok hs
  simp only [Result.ok.injEq, Prod.mk.injEq] at hs
  obtain ⟨rfl, rfl⟩ := hs
  exact ⟨n1, c1, hn1, hc1, hpost⟩

/-- From a pair related at window 39, the tail runs (`netlist_scan_tail`):
windows 39 and 40, then the edge `162`. -/
-- @assumes scan-tail-agrees
theorem tail_ok (kTail : KaniScanTail) {line m c} (hr : related m c line 39#u8 = ok true) :
    ∃ w39 w40 n, net_window m line 39#u8 c.latches.y = ok w39 ∧
      net_window w39 line 40#u8 c.latches.y = ok w40 ∧
      net_edge w40 162#i32 line c.latches.y = ok n := by
  obtain ⟨n, hn, -⟩ := spec_ok_exists (tail_saves kTail hr)
  unfold tail at hn
  obtain ⟨w39, h39, hn⟩ := bind_eq_ok hn
  obtain ⟨w40, h40, hn⟩ := bind_eq_ok hn
  exact ⟨w39, w40, n, h39, h40, hn⟩

/-- Ceres's latches hold the Y it read. -/
theorem read_latches {c line y c'} (h : ceres_read c line y = ok c') : c'.latches.y = y := by
  simp [ceres_read, ppu.display.objects.OamLatches.latch] at h
  obtain ⟨b, -, h⟩ := bind_eq_ok h
  obtain ⟨p, -, h⟩ := bind_eq_ok h
  obtain ⟨i, -, h⟩ := bind_eq_ok h
  simp only [Result.ok.injEq] at h
  rw [← h]

/-! ### The scan's dot as a block -/

/-- A dot's input: the Ys OAM gives at its two edges. -/
abbrev ScanIn := U8 × U8

/-- The netlist's dot: its two edges, from the half dot its state holds. -/
def netScan (line : Line) (s : NetSide × I32) (i : ScanIn) :
    Result (Unit × (NetSide × I32)) := do
  let n ← edge2 line s.1 s.2 i.1 i.2
  let h ← s.2 + 2#i32
  ok ((), (n, h))

/-- Ceres's dot: its read, at the dots that have one (`reads`). -/
def cerScan (line : Line) (s : CeresSide × I32) (i : ScanIn) :
    Result (Unit × (CeresSide × I32)) := do
  let c ← if reads s.2 then ceres_read s.1 line i.1 else ok s.1
  let h ← s.2 + 2#i32
  ok ((), (c, h))

/-- The netlist's dot, the half dots given. -/
theorem netScan_eq (line : Line) (n : NetSide) (h a b : I32) (i : ScanIn)
    (ha : a.val = h.val + 1) (hb : b.val = h.val + 2) (hr : -1000 ≤ h.val ∧ h.val ≤ 1000) :
    netScan line (n, h) i =
      (do let n1 ← net_edge n h line i.1; let n2 ← net_edge n1 a line i.2; ok ((), (n2, b))) := by
  unfold netScan
  rw [edge2_eq line n h a i.1 i.2 ha hr,
    i32_add_eq h 2#i32 b (by scalar_tac) (by scalar_tac) (by scalar_tac)]
  simp

/-- Ceres's dot, the half dot given. -/
theorem cerScan_eq (line : Line) (c : CeresSide) (h b : I32) (i : ScanIn)
    (hb : b.val = h.val + 2) (_hr : -1000 ≤ h.val ∧ h.val ≤ 1000) :
    cerScan line (c, h) i =
      (do let c' ← (if reads h then ceres_read c line i.1 else ok c); ok ((), (c', b))) := by
  unfold cerScan
  rw [i32_add_eq h 2#i32 b (by scalar_tac) (by scalar_tac) (by scalar_tac)]
  simp

/-- A half dot ahead, as a value. -/
theorem i32_next (h : I32) (k : Int) (hr : -1000 ≤ h.val ∧ h.val ≤ 1000) (hk : 0 ≤ k ∧ k ≤ 2) :
    ∃ a : I32, a.val = h.val + k := by
  obtain ⟨a, -, ha⟩ := i32_add (x := h) (y := IScalar.ofInt k (by scalar_tac))
    (by simp [IScalar.ofInt_val_eq]; scalar_tac) (by simp [IScalar.ofInt_val_eq]; scalar_tac)
  exact ⟨a, by simp [ha, IScalar.ofInt_val_eq]⟩

/-- Where window `j`'s pair comes from: Kani's relation at `j` (2 to 38);
at 39 too, with OAM giving sprite 39's Y again, as ceres latched it; at
40, window 39 run from there. -/
def Anchor (line : Line) (j : U8) (n0 : NetSide) (c : CeresSide) (y : U8) : Prop :=
  (2 ≤ j.val ∧ j.val ≤ 38 ∧ related n0 c line j = ok true) ∨
  (j = 39#u8 ∧ related n0 c line j = ok true ∧ y = c.latches.y) ∨
  (j = 40#u8 ∧ y = c.latches.y ∧
    ∃ m, related m c line 39#u8 = ok true ∧ net_window m line 39#u8 y = ok n0)

/-- From its anchor, window `j` runs. -/
theorem anchor_window (kStep : KaniScanStep) (kTail : KaniScanTail) {line j n0 c y}
    (ha : Anchor line j n0 c y) : ∃ n4, net_window n0 line j y = ok n4 := by
  rcases ha with ⟨h2, h38, hr⟩ | ⟨rfl, hr, rfl⟩ | ⟨rfl, rfl, m, hr, hw⟩
  · obtain ⟨n4, -, hn, -⟩ := window_ok kStep y hr h2 h38
    exact ⟨n4, hn⟩
  · obtain ⟨w39, -, -, h39, -⟩ := tail_ok kTail hr
    exact ⟨w39, h39⟩
  · obtain ⟨w39, w40, -, h39, h40, -⟩ := tail_ok kTail hr
    rw [hw, Result.ok.injEq] at h39
    subst h39
    exact ⟨w40, h40⟩

/-- Window `j`'s first edge run (at `4j - 2`), OAM giving `y`. -/
def Mid1 (line : Line) (j : U8) (n : NetSide) (c : CeresSide) (y : U8) : Prop :=
  ∃ n0 h0, Anchor line j n0 c y ∧ h0.val = 4 * (j.val : Int) - 2 ∧ net_edge n0 h0 line y = ok n

/-- Window `j`'s first three edges run (to `4j`), OAM giving `y`. -/
def Mid3 (line : Line) (j : U8) (n : NetSide) (c : CeresSide) (y : U8) : Prop :=
  ∃ n0 h0 h1, Anchor line j n0 c y ∧ h0.val = 4 * (j.val : Int) - 2 ∧
    h1.val = 4 * (j.val : Int) - 1 ∧
    (do let n1 ← net_edge n0 h0 line y; edge2 line n1 h1 y y) = ok n

/-- An anchor's window is 2 to 40. -/
theorem anchor_range {line j n0 c y} (ha : Anchor line j n0 c y) :
    2 ≤ j.val ∧ j.val ≤ 40 := by
  rcases ha with ⟨h2, h38, -⟩ | ⟨rfl, -⟩ | ⟨rfl, -⟩
  · omega
  · simp
  · simp

/-- From a window's first edge, a dot with the window's Y: its next two
edges, ceres reading nothing. -/
theorem mid1_dot (kStep : KaniScanStep) (kTail : KaniScanTail) {line j n c y} {h : I32}
    (hm : Mid1 line j n c y) (hh : h.val = 4 * (j.val : Int) - 1) :
    ∃ n' c' b, netScan line (n, h) (y, y) = ok ((), (n', b)) ∧
      cerScan line (c, h) (y, y) = ok ((), (c', b)) ∧ c' = c ∧
      b.val = 4 * (j.val : Int) + 1 ∧ Mid3 line j n' c' y := by
  obtain ⟨n0, h0, ha, hh0, hn⟩ := hm
  have hj := anchor_range ha
  obtain ⟨n4, hw⟩ := anchor_window kStep kTail ha
  obtain ⟨a, hav⟩ := i32_next h 1 (by constructor <;> omega) (by omega)
  obtain ⟨b, hbv⟩ := i32_next h 2 (by constructor <;> omega) (by omega)
  rw [net_window_split n0 line j y (by omega) h0 h b hh0 (by scalar_tac) (by scalar_tac)] at hw
  rw [hn, bind_ok] at hw
  obtain ⟨n', hn', -⟩ := bind_eq_ok hw
  have hr : reads h = false := by simp [reads, hh]
  refine ⟨n', c, b, ?_, ?_, rfl, by scalar_tac, n0, h0, h, ha, hh0, hh, by simp [hn, hn']⟩
  · rw [netScan_eq line n h a b (y, y) hav hbv (by scalar_tac)]
    rw [edge2_eq line n h a y y hav (by scalar_tac)] at hn'
    simp only at hn' ⊢
    obtain ⟨n1, hn1, hn'⟩ := bind_eq_ok hn'
    simp [hn1, hn']
  · rw [cerScan_eq line c h b (y, y) hbv (by scalar_tac), hr]
    simp

/-- A window that runs, its first edge does. -/
theorem window_first {line n0 j y n4} {h0 : I32} (hw : net_window n0 line j y = ok n4)
    (hj : j.val ≤ 100) (hh0 : h0.val = 4 * (j.val : Int) - 2) :
    ∃ n1, net_edge n0 h0 line y = ok n1 := by
  obtain ⟨h1, hh1⟩ := i32_next h0 1 (by constructor <;> omega) (by omega)
  obtain ⟨h3, hh3⟩ := i32_next h0 2 (by constructor <;> omega) (by omega)
  obtain ⟨h3', hh3'⟩ := i32_next h3 1 (by constructor <;> omega) (by omega)
  rw [net_window_split n0 line j y hj h0 h1 h3' hh0 hh1 (by omega)] at hw
  obtain ⟨n1, hn1, -⟩ := bind_eq_ok hw
  exact ⟨n1, hn1⟩

/-- A window that runs, from its first three edges: its last. -/
theorem window_last {line n0 j y n4 n} {h0 h1 h : I32} (hw : net_window n0 line j y = ok n4)
    (hj : j.val ≤ 100) (hh0 : h0.val = 4 * (j.val : Int) - 2) (hh1 : h1.val = 4 * (j.val : Int) - 1)
    (hh : h.val = 4 * (j.val : Int) + 1)
    (hn : (do let n1 ← net_edge n0 h0 line y; edge2 line n1 h1 y y) = ok n) :
    net_edge n h line y = ok n4 := by
  rw [net_window_split n0 line j y hj h0 h1 h (by omega) (by omega) (by omega)] at hw
  obtain ⟨n1, hn1, hn⟩ := bind_eq_ok hn
  simp only [hn1, bind_ok, hn] at hw
  exact hw

/-- The scan's end: windows 39 and 40 and the edge `162` run, from a pair
related at 39. -/
def Ended (line : Line) (n : NetSide) (c : CeresSide) : Prop :=
  ∃ m, related m c line 39#u8 = ok true ∧ tail m line c.latches.y = ok n

/-- From a window's third edge, a dot with the window's Y, and the next
window's (the same past 38): the window's last edge, ceres's read of its
sprite (up to 38), and the next window's first edge; or, after window 40,
the scan's last edge. -/
theorem mid3_dot (kStep : KaniScanStep) (kTail : KaniScanTail) {line j n c y} {h : I32}
    (hm : Mid3 line j n c y) (hh : h.val = 4 * (j.val : Int) + 1) (y2 : U8)
    (hf : 38 ≤ j.val → y2 = y) :
    ∃ n' c' b, netScan line (n, h) (y, y2) = ok ((), (n', b)) ∧
      cerScan line (c, h) (y, y2) = ok ((), (c', b)) ∧ b.val = 4 * (j.val : Int) + 3 ∧
      ((∃ j' : U8, j'.val = j.val + 1 ∧ Mid1 line j' n' c' y2) ∨
        (j.val = 40 ∧ Ended line n' c')) := by
  obtain ⟨n0, h0, h1, ha, hh0, hh1, hn⟩ := hm
  have hj := anchor_range ha
  obtain ⟨a, hav⟩ := i32_next h 1 (by constructor <;> omega) (by omega)
  obtain ⟨b, hbv⟩ := i32_next h 2 (by constructor <;> omega) (by omega)
  have hnet : ∀ n4 n5, net_edge n h line y = ok n4 → net_edge n4 a line y2 = ok n5 →
      netScan line (n, h) (y, y2) = ok ((), (n5, b)) := by
    intro n4 n5 h4 h5
    rw [netScan_eq line n h a b (y, y2) hav hbv (by constructor <;> omega)]
    simp [h4, h5]
  have hab : a.val = 4 * ((j.val + 1 : Nat) : Int) - 2 := by push_cast; omega
  rcases ha with ⟨h2, h38, hr⟩ | ⟨rfl, hr, hy⟩ | ⟨rfl, hy, m, hr, hw0⟩
  · -- Kani's window: ceres reads its sprite, the pair is related at `j + 1`.
    obtain ⟨n4, c', hw, hc, hr'⟩ := window_ok kStep y hr h2 h38
    have h4 := window_last hw (by omega) hh0 hh1 hh hn
    have hrd : reads h = true := by simp [reads, hh]; omega
    have hcer : cerScan line (c, h) (y, y2) = ok ((), (c', b)) := by
      rw [cerScan_eq line c h b (y, y2) hbv (by constructor <;> omega), hrd]
      simp [hc]
    obtain ⟨j', hj', hj'v⟩ := spec_ok_exists (U8.add_spec (x := j) (y := 1#u8) (by scalar_tac))
    have hj'1 : j'.val = j.val + 1 := by simpa using hj'v
    have hr1 := hr' j' hj'1
    by_cases h37 : j.val ≤ 37
    · obtain ⟨n7, -, h7, -⟩ := window_ok kStep y2 hr1 (by omega) (by omega)
      obtain ⟨n6, h6⟩ := window_first (h0 := a) h7 (by omega) (by rw [hj'1]; exact hab)
      refine ⟨n6, c', b, hnet n4 n6 h4 h6, hcer, by omega, Or.inl ⟨j', hj'1, n4, a, ?_, ?_, h6⟩⟩
      · exact Or.inl ⟨by omega, by omega, hr1⟩
      · rw [hj'1]; exact hab
    · have h39 : j' = 39#u8 := UScalar.eq_of_val_eq (by simp; omega)
      subst h39
      have hy2 := hf (by omega)
      subst hy2
      have hlat := read_latches hc
      obtain ⟨w39, w40, nf, hw39, -, -⟩ := tail_ok kTail hr1
      rw [hlat] at hw39
      obtain ⟨n6, h6⟩ := window_first (h0 := a) hw39 (by simp) (by simp; omega)
      refine ⟨n6, c', b, hnet n4 n6 h4 h6, hcer, by omega,
        Or.inl ⟨39#u8, hj'1, n4, a, Or.inr (Or.inl ⟨rfl, hr1, hlat.symm⟩), by simp; omega, h6⟩⟩
  · -- Window 39, the tail's: no read; window 40 starts.
    subst hy
    have hy2 := hf (by simp)
    subst hy2
    obtain ⟨w39, w40, nf, hw39, hw40, -⟩ := tail_ok kTail hr
    have h4 := window_last hw39 (by simp) hh0 hh1 hh hn
    obtain ⟨n6, h6⟩ := window_first (h0 := a) hw40 (by simp) (by simp at hh ⊢; omega)
    have hrd : reads h = false := by simp [reads, hh]
    refine ⟨n6, c, b, hnet w39 n6 h4 h6, ?_, by omega,
      Or.inl ⟨40#u8, by simp, w39, a, Or.inr (Or.inr ⟨rfl, rfl, n0, hr, hw39⟩),
        by simp at hh ⊢; omega,
        h6⟩⟩
    rw [cerScan_eq line c h b _ hbv (by constructor <;> omega), hrd]
    simp
  · -- Window 40: no read; the scan's last edge, `162`.
    subst hy
    have hy2 := hf (by simp)
    subst hy2
    obtain ⟨w39, w40, nf, hw39, hw40, hf162⟩ := tail_ok kTail hr
    rw [hw0, Result.ok.injEq] at hw39
    subst hw39
    have h4 := window_last hw40 (by simp) hh0 hh1 hh hn
    have ha162 : a = 162#i32 := IScalar.eq_of_val_eq (by simp at hh ⊢; omega)
    subst ha162
    have hrd : reads h = false := by simp [reads, hh]
    refine ⟨nf, c, b, hnet w40 nf h4 hf162, ?_, by omega, Or.inr ⟨by simp, m, hr, ?_⟩⟩
    · rw [cerScan_eq line c h b _ hbv (by constructor <;> omega), hrd]
      simp
    · simp [tail, hw0, hw40, hf162]

/-- The lead-in after `k` dots, from the line's end (the line before's
scan ended with any count), OAM giving `ys`. -/
def LeadW (line : Line) (k : Nat) (n : NetSide) (c : CeresSide) (ys : LeadYs) : Prop :=
  ∃ count ns held n0 c0, count ≤ ceres_netlist.oam_scan.SLOTS ∧ line.ly < 144#u8 ∧
    net_at_line_end count ns held = ok n0 ∧ ceres_at_line_end held = ok c0 ∧
    leadNet line n0 ys k = ok n ∧ leadCer line c0 ys k = ok c

/-- The Ys the lead-in's dot `k` must be given: those of the sprites whose
window it is in (a new window's are any). -/
def leadFits (k : Nat) (ys : LeadYs) (i : ScanIn) : Prop :=
  match k with
  | 0 => True
  | 2 => i.1 = ys.y0
  | 4 => i.1 = ys.y1
  | 6 => i.1 = ys.y2
  | _ => i = leadYs ys k

/-- The lead-in's Ys with dot `k`'s new ones. -/
def ysNext (k : Nat) (ys : LeadYs) (i : ScanIn) : LeadYs :=
  match k with
  | 0 => { ys with y39 := i.1, y0 := i.2 }
  | 2 => { ys with y1 := i.2 }
  | 4 => { ys with y2 := i.2 }
  | _ => ys

theorem ysNext_fits {k ys i} (hk : k < 6) (hf : leadFits k ys i) :
    leadYs (ysNext k ys i) k = i := by
  obtain ⟨i1, i2⟩ := i
  match k, hk with
  | 0, _ | 1, _ | 2, _ | 3, _ | 4, _ | 5, _ => simp_all [leadFits, ysNext, leadYs]

theorem ysNext_same {line k n c ys} (i : ScanIn) (hk : k < 6) (hw : LeadW line k n c ys) :
    LeadW line k n c (ysNext k ys i) := by
  obtain ⟨count, ns, held, n0, c0, hc, hl, hn, hce, hN, hC⟩ := hw
  match k, hk with
  | 0, _ | 1, _ | 2, _ | 3, _ | 4, _ | 5, _ =>
    exact ⟨count, ns, held, n0, c0, hc, hl, hn, hce,
      by simpa [ysNext, leadNet, leadYs] using hN, by simpa [ysNext, leadCer] using hC⟩

theorem leadCer_prefix {line c0 ys C} (h6 : leadCer line c0 ys 6 = ok C) (k : Nat) (hk : k ≤ 6) :
    ∃ c, leadCer line c0 ys k = ok c := by
  simp only [leadCer] at h6
  obtain ⟨c1, hc1, -⟩ := bind_eq_ok h6
  match k, hk with
  | 0, _ | 1, _ | 2, _ => exact ⟨c0, rfl⟩
  | 3, _ | 4, _ => exact ⟨c1, hc1⟩
  | 5, _ | 6, _ => exact ⟨C, by simp only [leadCer]; exact h6⟩

theorem leadH_val (k : Nat) (hk : k < 6) : (leadH k).val = 2 * (k : Int) - 7 := by
  match k, hk with
  | 0, _ | 1, _ | 2, _ | 3, _ | 4, _ | 5, _ => simp [leadH]

/-- A lead-in dot but the last, with Ys that fit: both sides step, into
the lead-in a dot on. -/
-- @assumes scan-lead-in-agrees
theorem lead_dot (kLead : KaniScanLeadIn) {line k n c ys} {h : I32} (i : ScanIn) (hk : k < 6)
    (hw : LeadW line k n c ys) (hh : h.val = 2 * (k : Int) - 7) (hf : leadFits k ys i) :
    ∃ n' c' b ys', netScan line (n, h) i = ok ((), (n', b)) ∧
      cerScan line (c, h) i = ok ((), (c', b)) ∧ b.val = 2 * ((k + 1 : Nat) : Int) - 7 ∧
      LeadW line (k + 1) n' c' ys' := by
  have hw' := ysNext_same i hk hw
  have hi := ysNext_fits hk hf
  generalize ysNext k ys i = ys' at hw' hi
  obtain ⟨count, ns, held, n0, c0, hc, hl, hn, hce, hN, hC⟩ := hw'
  obtain ⟨N, -, C, -, hN6, -, hC6, -, -⟩ := lead_ok kLead ys' hc hl hn hce
  obtain ⟨n', hn'⟩ := leadNet_prefix hN6 (k := k + 1) (by omega)
  obtain ⟨c', hc'⟩ := leadCer_prefix hC6 (k + 1) (by omega)
  have hH : h = leadH k := IScalar.eq_of_val_eq (by rw [leadH_val k hk]; exact hh)
  subst hH
  obtain ⟨b, hbv⟩ := i32_next (leadH k) 2 (by rw [leadH_val k hk]; constructor <;> omega) (by omega)
  rw [leadNet, hN, bind_ok, hi] at hn'
  rw [leadCer_succ line c0 ys' k hk, hC, bind_ok, hi] at hc'
  refine ⟨n', c', b, ys', ?_, ?_, by rw [hbv, leadH_val k hk]; push_cast; omega,
    count, ns, held, n0, c0, hc, hl, hn, hce, ?_, ?_⟩
  · unfold netScan
    rw [hn', bind_ok, i32_add_eq _ 2#i32 b hbv (by scalar_tac) (by scalar_tac)]
    simp
  · unfold cerScan
    rw [hc', bind_ok, i32_add_eq _ 2#i32 b hbv (by scalar_tac) (by scalar_tac)]
    simp
  · rw [leadNet, hN, bind_ok, hi]; exact hn'
  · rw [leadCer_succ line c0 ys' k hk, hC, bind_ok, hi]; exact hc'

/-- The lead-in's last dot, with sprite 2's Y: its edge `5` and ceres's
read of sprite 2 end the lead-in, related at window 2; then window 2's
first edge. -/
-- @assumes scan-lead-in-agrees
-- @assumes scan-step-agrees
theorem lead_last (kLead : KaniScanLeadIn) (kStep : KaniScanStep) {line n c ys} {h : I32}
    (i : ScanIn) (hw : LeadW line 6 n c ys) (hh : h.val = 5) (hf : i.1 = ys.y2) :
    ∃ n' c' b, netScan line (n, h) i = ok ((), (n', b)) ∧
      cerScan line (c, h) i = ok ((), (c', b)) ∧ b.val = 7 ∧ Mid1 line 2#u8 n' c' i.2 := by
  obtain ⟨count, ns, held, n0, c0, hc, hl, hn, hce, hN, hC⟩ := hw
  obtain ⟨N, N', C, C', hN6, hN', hC6, hC', hr⟩ := lead_ok kLead ys hc hl hn hce
  rw [hN, Result.ok.injEq] at hN6
  rw [hC, Result.ok.injEq] at hC6
  subst hN6 hC6
  obtain ⟨n7, -, h7, -⟩ := window_ok kStep i.2 hr (by simp) (by simp)
  obtain ⟨a, hav⟩ := i32_next h 1 (by constructor <;> omega) (by omega)
  obtain ⟨b, hbv⟩ := i32_next h 2 (by constructor <;> omega) (by omega)
  obtain ⟨n6, h6⟩ := window_first (h0 := a) h7 (by simp) (by simp; omega)
  have h5 : h = 5#i32 := IScalar.eq_of_val_eq (by simp [hh])
  subst h5
  have hrd : reads 5#i32 = true := by simp [reads]
  refine ⟨n6, C', b, ?_, ?_, by omega, N', a, Or.inl ⟨by simp, by simp, hr⟩, by simp; omega, h6⟩
  · rw [netScan_eq line n 5#i32 a b i hav hbv (by simp)]
    simp [hf, hN', h6]
  · rw [cerScan_eq line c 5#i32 b i hbv (by simp), hrd]
    simp [hf, hC']

/-! ### The scan, a dot at a time -/

/-- The scan's pairs between its dots: at the same half dot, the next
edge's, in the lead-in (a prefix of it from the line's end) or in a
window (one or three of its edges run from its anchor). -/
def ScanRel (line : Line) (s : NetSide × I32) (t : CeresSide × I32) : Prop :=
  s.2 = t.2 ∧
    ((∃ (k : Nat) (ys : LeadYs), k ≤ 6 ∧ s.2.val = 2 * (k : Int) - 7 ∧ LeadW line k s.1 t.1 ys) ∨
      (∃ j y, s.2.val = 4 * (j.val : Int) - 1 ∧ Mid1 line j s.1 t.1 y) ∨
      (∃ j y, s.2.val = 4 * (j.val : Int) + 1 ∧ Mid3 line j s.1 t.1 y))

/-- The scan ended: its last edge, `162`, run, the next is mode 3's. -/
def ScanEnd (line : Line) (s : NetSide × I32) (t : CeresSide × I32) : Prop :=
  s.2 = t.2 ∧ s.2.val = 163 ∧ Ended line s.1 t.1

/-- The scan's known differences: OAM gives a dot a Y that is not the one
of the window it is in (some ceres state with the netlist's: the Ys are
the pair's, not the netlist's alone). -/
def ScanDotK (line : Line) (s : NetSide × I32) (i : ScanIn) : Prop :=
  ∃ c, (∃ (k : Nat) (ys : LeadYs), k ≤ 6 ∧ s.2.val = 2 * (k : Int) - 7 ∧
      LeadW line k s.1 c ys ∧ ¬ leadFits k ys i) ∨
    (∃ j y, s.2.val = 4 * (j.val : Int) - 1 ∧ Mid1 line j s.1 c y ∧ i ≠ (y, y)) ∨
    (∃ j y, s.2.val = 4 * (j.val : Int) + 1 ∧ Mid3 line j s.1 c y ∧
      ¬ (i.1 = y ∧ (38 ≤ j.val → i.2 = y)))

/-- The scan's dot as a block: from the pairs `ScanRel` relates, a dot
leaves them related, or, at the scan's end, ended. -/
-- @assumes scan-lead-in-agrees
-- @assumes scan-step-agrees
-- @assumes scan-tail-agrees
theorem scan_dot_steps (kStep : KaniScanStep) (kLead : KaniScanLeadIn) (kTail : KaniScanTail)
    (line : Line) :
    StepsInto (netScan line) (cerScan line) (fun i j => i = j) (fun _ _ => True) (ScanDotK line)
      (ScanRel line) (fun s t => ScanRel line s t ∨ ScanEnd line s t) := by
  rintro ⟨n, h⟩ ⟨c, h'⟩ ⟨hh', hr⟩ i _ rfl
  simp only at hh' hr
  subst hh'
  rcases hr with ⟨k, ys, hk, hh, hw⟩ | ⟨j, y, hh, hm⟩ | ⟨j, y, hh, hm⟩
  · by_cases hf : leadFits k ys i
    · right
      rcases Nat.lt_or_eq_of_le hk with hk6 | rfl
      · obtain ⟨n', c', b, ys', hn, hc, hb, hw'⟩ := lead_dot kLead i hk6 hw hh hf
        exact ⟨(), (n', b), (), (c', b), hn, hc, trivial,
          Or.inl ⟨rfl, Or.inl ⟨k + 1, ys', hk6, hb, hw'⟩⟩⟩
      · obtain ⟨n', c', b, hn, hc, hb, hm'⟩ := lead_last kLead kStep i hw (by simpa using hh) hf
        exact ⟨(), (n', b), (), (c', b), hn, hc, trivial,
          Or.inl ⟨rfl, Or.inr (Or.inl ⟨2#u8, i.2, by simp [hb], hm'⟩)⟩⟩
    · exact Or.inl ⟨c, Or.inl ⟨k, ys, hk, hh, hw, hf⟩⟩
  · by_cases hf : i = (y, y)
    · subst hf
      obtain ⟨n', c', b, hn, hc, rfl, hb, hm'⟩ := mid1_dot kStep kTail hm hh
      exact Or.inr ⟨(), (n', b), (), (c', b), hn, hc, trivial,
        Or.inl ⟨rfl, Or.inr (Or.inr ⟨j, y, hb, hm'⟩)⟩⟩
    · exact Or.inl ⟨c, Or.inr (Or.inl ⟨j, y, hh, hm, hf⟩)⟩
  · by_cases hf : i.1 = y ∧ (38 ≤ j.val → i.2 = y)
    · obtain ⟨i1, i2⟩ := i
      obtain ⟨rfl, hf2⟩ := hf
      obtain ⟨n', c', b, hn, hc, hb, hnext⟩ := mid3_dot kStep kTail hm hh i2 hf2
      refine Or.inr ⟨(), (n', b), (), (c', b), hn, hc, trivial, ?_⟩
      rcases hnext with ⟨j', hj', hm'⟩ | ⟨h40, he⟩
      · exact Or.inl ⟨rfl, Or.inr (Or.inl ⟨j', i2, by rw [hb, hj']; push_cast; ring, hm'⟩)⟩
      · exact Or.inr ⟨rfl, by rw [hb, h40]; rfl, he⟩
    · exact Or.inl ⟨c, Or.inr (Or.inr ⟨j, y, hh, hm, hf⟩)⟩

/-- The scan's start: from the line's end, at half dot `-7`. -/
theorem scan_starts {count ns held line n0 c0} {h : I32} (ys : LeadYs)
    (hc : count ≤ ceres_netlist.oam_scan.SLOTS) (hl : line.ly < 144#u8)
    (hn : net_at_line_end count ns held = ok n0) (hce : ceres_at_line_end held = ok c0)
    (hh : h.val = -7) : ScanRel line (n0, h) (c0, h) :=
  ⟨rfl, Or.inl ⟨0, ys, by omega, by simp [hh], count, ns, held, n0, c0, hc, hl, hn, hce, rfl, rfl⟩⟩

/-- The scan's end: the netlist's slots hold ceres's first ten sprites on
the line, and its scan has stopped (`netlist_scan_tail`). -/
-- @assumes scan-tail-agrees
theorem scan_end_saves (kTail : KaniScanTail) {line s t} (he : ScanEnd line s t) :
    slots_are s.1 t.1 t.1.found = ok true ∧ ¬ s.1.scan.scanning := by
  obtain ⟨-, -, m, hr, ht⟩ := he
  have hs := tail_saves kTail hr
  rw [ht, WP.spec_ok] at hs
  exact hs

end ceres_core.scan
