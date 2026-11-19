import Ceres.Bisim.ScanDots
import Ceres.Bisim.Mode3
open Aeneas Aeneas.Std Result ceres_core.bisim

/-!
# The netlist's pipeline outside mode 3

Outside mode 3 render's pipeline does not follow ceres: it rests
(`ceres-core/src/ppu/display/background/relation/rest.rs`). Kani proves,
an edge at a time (the `netlist_rest_*` harnesses of `background/bisim.rs`):

- `KaniRestEnd`: the rise after mode 3's stop drops mode 3, into
  `hblank_rest`;
- `KaniRestHBlank`: every HBlank or VBlank edge (no line start, no stop)
  keeps `hblank_rest`;
- `KaniRestScan`: from the line's end (`h = -8`) each edge of the scan,
  with the scan's outputs as `scan_inputs` has them at that edge and
  anything else free, takes the pipeline from `pipe_at h` to
  `pipe_at (h + 1)`; at `162`, the pipeline is as `before_start` has it.

Here they are taken apart into the edges' runs (`end_edge`, `hblank_edge`,
`scan_edge`), and the scan's into a dot beside the scan's own
(`pipeScan`, as `ScanDots.netScan`: the state holds the next edge),
from the line's end to `before_start` (`pipe_scan_dot`).
-/

namespace ceres_core.rest
open ceres_core.ppu.display.background.relation.rest
open ceres_core.ppu.display.background.relation.pipeline
open ceres_netlist.render_edge

/-- What `netlist_rest_hblank` proves. -/
def KaniRestHBlank : Prop := ∀ p i, hblank_agrees p i = ok true

/-- What `netlist_rest_end` and `netlist_rest_end_window` prove. -/
def KaniRestEnd : Prop :=
  ∀ l net g mixer rising falling next, end_agrees l net g mixer rising falling next = ok true

/-- What `netlist_rest_scan` proves. -/
def KaniRestScan : Prop := ∀ p h free, scan_agrees p h free = ok true

/-- An edge of HBlank or VBlank keeps the pipeline at rest. -/
-- @assumes pipeline-rests
theorem hblank_edge (k : KaniRestHBlank) {p : NetPipeline} {i : EdgeInputs}
    (hp : hblank_rest p = ok true) (hi : idle_inputs i = ok true) :
    ∃ p', NetPipeline.edge p i = ok p' ∧ hblank_rest p' = ok true := by
  have h := k p i
  simp only [hblank_agrees, hp, hi, bind_ok, Bool.and_self, ite_true] at h
  exact bind_eq_ok h

/-- Mode 3's end: from a pair related where ceres has drawn the line, on
inputs the mode 3 dot's guard lets through, the dot's two edges and the
rise after them run, and the pipeline rests. -/
-- @assumes pipeline-rests
theorem end_edges (k : KaniRestEnd) {l net g mixer rising falling next}
    (hr : ceres_core.ppu.display.background.relation.related l net g = ok true)
    (hd : ceres_core.ppu.display.background.relation.drawn l.pace = ok true)
    (hg : ceres_core.mode3.guard l net mixer rising falling = ok true)
    (hc : next.clk = true) (hi : idle_inputs next = ok true) :
    ∃ n1 n2 n3, NetPipeline.edge net rising = ok n1 ∧ NetPipeline.edge n1 falling = ok n2 ∧
      NetPipeline.edge n2 next = ok n3 ∧ hblank_rest n3 = ok true := by
  have h := k l net g mixer rising falling next
  unfold ceres_core.mode3.guard at hg
  obtain ⟨b, e0, hg⟩ := bind_eq_ok hg
  obtain ⟨b1, e1, hg⟩ := bind_eq_ok hg
  obtain ⟨b2, e2, hg⟩ := bind_eq_ok hg
  obtain ⟨b3, e3, hg⟩ := bind_eq_ok hg
  obtain ⟨b4, e4, hg⟩ := bind_eq_ok hg
  obtain ⟨b5, e5, hg⟩ := bind_eq_ok hg
  simp only [Result.ok.injEq] at hg
  simp only [end_agrees, hr, hd, e0, e1, e2, e3, e4, e5, hi, hc, bind_ok, hg, Bool.and_true,
    Bool.true_and, ite_true] at h
  obtain ⟨n1, h1, h'⟩ := bind_eq_ok h
  obtain ⟨n2, h2, h''⟩ := bind_eq_ok h'
  obtain ⟨n3, h3, h4⟩ := bind_eq_ok h''
  exact ⟨n1, n2, n3, h1, h2, h3, h4⟩

/-- An edge of the scan, `h + 1`, from the pipeline at `h`. -/
-- @assumes pipeline-rests
theorem scan_edge (k : KaniRestScan) {p : NetPipeline} {h : I32} (free : Free)
    (hlo : -8 ≤ h.val) (hhi : h.val ≤ 161) (hp : pipe_at p h = ok true) :
    ∃ h' e p', h + 1#i32 = ok h' ∧ scan_inputs h' free = ok e ∧
      NetPipeline.edge p e = ok p' ∧ pipe_at p' h' = ok true ∧
      (h'.val = 162 → ∃ q, before_start p' = ok q ∧
        NetPipeline.Insts.CoreCmpPartialEqNetPipeline.eq p' q = ok true) := by
  have hk := k p h free
  have c1 : (h >= (-8)#i32) = true := by simp; scalar_tac
  have c2 : (h <= 161#i32) = true := by simp; scalar_tac
  simp only [scan_agrees, hp, c1, c2, bind_ok, Bool.and_self] at hk
  obtain ⟨h', hh, k1⟩ := bind_eq_ok hk
  obtain ⟨e, he, k2⟩ := bind_eq_ok k1
  obtain ⟨p', hp', k3⟩ := bind_eq_ok k2
  obtain ⟨b1, hb1, k4⟩ := bind_eq_ok k3
  obtain ⟨q, hq, k5⟩ := bind_eq_ok k4
  obtain ⟨b2, hb2, k6⟩ := bind_eq_ok k5
  simp only [Result.ok.injEq, Bool.and_eq_true, Bool.or_eq_true, bne_iff_ne, ne_eq] at k6
  obtain ⟨hb1t, k7⟩ := k6
  subst hb1t
  refine ⟨h', e, p', hh, he, hp', hb1, fun h162 => ?_⟩
  refine ⟨q, hq, ?_⟩
  cases k7 with
  | inl k8 => exact absurd (by scalar_tac : h' = 162#i32) k8
  | inr k8 => exact k8 ▸ hb2

/-! ## The pipeline's dot in the scan

As the scan's (`ScanDots.netScan`), the state holds the next edge; the
inputs are what the scan's outputs leave free at each of the two edges
(`Free`: LCDC, LY, hclk, VBlank, the CPU's bus). -/

open ceres_core.scan in
/-- A sum's value, the sum known. -/
theorem add_val {x y z : I32} (h : x + y = ok z) (h1 : IScalar.min .I32 ≤ x.val + y.val)
    (h2 : x.val + y.val ≤ IScalar.max .I32) : z.val = x.val + y.val := by
  obtain ⟨w, hw, hwv⟩ := i32_add h1 h2
  rw [h] at hw
  simp only [Result.ok.injEq] at hw
  subst hw
  exact hwv

open ceres_core.scan in
/-- The pipeline's dot in the scan: its two edges, with the scan's outputs
at each. -/
def pipeScan (s : NetPipeline × I32) (f : Free × Free) :
    Result (Unit × (NetPipeline × I32)) := do
  let e1 ← scan_inputs s.2 f.1
  let p1 ← NetPipeline.edge s.1 e1
  let h1 ← s.2 + 1#i32
  let e2 ← scan_inputs h1 f.2
  let p2 ← NetPipeline.edge p1 e2
  let h2 ← s.2 + 2#i32
  ok ((), (p2, h2))

/-- The pipeline as `pipe_at` has it after the edge before the next. -/
def PipeAt (s : NetPipeline × I32) : Prop :=
  ∃ hp : I32, hp.val = s.2.val - 1 ∧ pipe_at s.1 hp = ok true

open ceres_core.scan in
/-- A dot of the scan, from the line's end (the next edge `-7`) to the
scan's last (`161` and `162`): the pipeline at the next dot; after `162`,
as `before_start` has it. -/
-- @assumes pipeline-rests
theorem pipe_scan_dot (k : KaniRestScan) {s : NetPipeline × I32} (hs : PipeAt s)
    (hlo : -7 ≤ s.2.val) (hhi : s.2.val ≤ 161) (f : Free × Free) :
    ∃ s', pipeScan s f = ok ((), s') ∧ s'.2.val = s.2.val + 2 ∧ PipeAt s' ∧
      (s'.2.val = 163 → ∃ q, before_start s'.1 = ok q ∧
        NetPipeline.Insts.CoreCmpPartialEqNetPipeline.eq s'.1 q = ok true) := by
  obtain ⟨hp, hv, hp0⟩ := hs
  obtain ⟨h1, e1, p1, hh1, he1, hp1, hpa1, -⟩ := scan_edge k f.1 (by omega) (by omega) hp0
  have hv1 : h1.val = s.2.val := by
    have := add_val hh1 (by scalar_tac) (by scalar_tac); simp at this; omega
  have hs1 : h1 = s.2 := IScalar.eq_of_val_eq hv1
  subst hs1
  obtain ⟨h2, e2, p2, hh2, he2, hp2, hpa2, h162⟩ := scan_edge k f.2 (by omega) (by omega) hpa1
  have hv2 : h2.val = s.2.val + 1 := by
    have := add_val hh2 (by scalar_tac) (by scalar_tac); simp at this; omega
  obtain ⟨b, hb, hbv⟩ := i32_add (x := s.2) (y := 2#i32) (by scalar_tac) (by scalar_tac)
  refine ⟨(p2, b), ?_, by simp [hbv], ⟨h2, by simp [hbv, hv2]; omega, hpa2⟩, fun h163 => ?_⟩
  · obtain ⟨p, h'⟩ := s
    simp only [pipeScan, he1, hp1, hh2, he2, hp2, hb, bind_ok]
  · exact h162 (by simp [hbv] at h163; omega)

open ceres_core.scan in
/-- A scan state's next edge is `-7` to `161`. -/
theorem scan_rel_range {line s t} (h : ScanRel line s t) : -7 ≤ s.2.val ∧ s.2.val ≤ 161 := by
  obtain ⟨-, ⟨k, ys, hk, hh, -⟩ | ⟨j, y, hh, n0, h0, ha, -⟩ | ⟨j, y, hh, n0, h0, h1, ha, -⟩⟩ := h
  · omega
  · have := anchor_range ha; omega
  · have := anchor_range ha; omega

open ceres_core.scan in
/-- The scan's dot moves its next edge on by two. -/
theorem netScan_next {line s i s'} (h : netScan line s i = ok ((), s'))
    (hr : -1000 ≤ s.2.val ∧ s.2.val ≤ 1000) : s'.2.val = s.2.val + 2 := by
  unfold netScan at h
  obtain ⟨n, -, h⟩ := bind_eq_ok h
  obtain ⟨b, hb, h⟩ := bind_eq_ok h
  simp only [Result.ok.injEq, Prod.mk.injEq, true_and] at h
  subst h
  have := add_val hb (by scalar_tac) (by scalar_tac)
  simpa using this

/-- The pipeline's dot in HBlank or VBlank: its two edges. -/
def pipeIdle (p : NetPipeline) (i : EdgeInputs × EdgeInputs) : Result (Unit × NetPipeline) := do
  let p1 ← NetPipeline.edge p i.1
  let p2 ← NetPipeline.edge p1 i.2
  ok ((), p2)

/-- A dot of HBlank or VBlank keeps the pipeline at rest. -/
-- @assumes pipeline-rests
theorem pipe_idle_dot (k : KaniRestHBlank) {p : NetPipeline} (hp : hblank_rest p = ok true)
    {i : EdgeInputs × EdgeInputs} (h1 : idle_inputs i.1 = ok true)
    (h2 : idle_inputs i.2 = ok true) :
    ∃ p', pipeIdle p i = ok ((), p') ∧ hblank_rest p' = ok true := by
  obtain ⟨p1, e1, r1⟩ := hblank_edge k hp h1
  obtain ⟨p2, e2, r2⟩ := hblank_edge k r1 h2
  exact ⟨p2, by simp [pipeIdle, e1, e2], r2⟩

/-- HBlank's first dot, from mode 3's stop: the pipeline rests. -/
-- @assumes pipeline-rests
theorem pipe_end_dot (kEnd : KaniRestEnd) (kH : KaniRestHBlank) {p : NetPipeline}
    (hs : ceres_core.mode3.Stops p) {i : EdgeInputs × EdgeInputs} (hc : i.1.clk = true)
    (h1 : idle_inputs i.1 = ok true) (h2 : idle_inputs i.2 = ok true) :
    ∃ p', pipeIdle p i = ok ((), p') ∧ hblank_rest p' = ok true := by
  obtain ⟨l, net, g, mixer, rising, falling, n1, hr, hd, hg, e1, e2⟩ := hs
  obtain ⟨m1, m2, m3, f1, f2, f3, hm3⟩ := end_edges kEnd hr hd hg hc h1
  rw [e1] at f1
  simp only [Result.ok.injEq] at f1
  subst f1
  rw [e2] at f2
  simp only [Result.ok.injEq] at f2
  subst f2
  obtain ⟨p2, e3, r2⟩ := hblank_edge kH hm3 h2
  exact ⟨p2, by simp [pipeIdle, f3, e3], r2⟩

/-- The pipeline in HBlank or VBlank: at rest, or just stopped. -/
def HRest (p : NetPipeline) : Prop := hblank_rest p = ok true ∨ ceres_core.mode3.Stops p

/-- A dot of HBlank or VBlank (its first edge a rise, neither a line
start nor a stop): the pipeline rests. -/
-- @assumes pipeline-rests
theorem pipe_hblank_dot (kEnd : KaniRestEnd) (kH : KaniRestHBlank) {p : NetPipeline}
    (hp : HRest p) {i : EdgeInputs × EdgeInputs} (hc : i.1.clk = true)
    (h1 : idle_inputs i.1 = ok true) (h2 : idle_inputs i.2 = ok true) :
    ∃ p', pipeIdle p i = ok ((), p') ∧ hblank_rest p' = ok true := by
  rcases hp with hp | hp
  · exact pipe_idle_dot kH hp h1 h2
  · exact pipe_end_dot kEnd kH hp hc h1 h2

/-- At the line's end the pipeline at rest is where the scan's takes it
from (`pipe_at` at `-8`; the scan's state holds the next edge, `-7`). -/
theorem rest_pipe_at {p : NetPipeline} (hp : hblank_rest p = ok true) {e : I32}
    (he : e.val = -7) : PipeAt (p, e) := by
  refine ⟨(-8)#i32, by simp [he], ?_⟩
  unfold pipe_at
  simp [hp]

end ceres_core.rest
