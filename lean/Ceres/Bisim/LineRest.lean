import Ceres.Bisim.Rest
open Aeneas Aeneas.Std Result ceres_core.bisim

/-!
# The scan and the pipeline together

On the chip, render's pipeline takes the scan's outputs after each edge
(`NetChip::step_render`: `line_start`, `stop_scan`, CENO), with the clock
and the rest of its inputs. `outInputs` is that wiring; `lineScan` is the
netlist's dot in the scan with it: the scan's edge, then the pipeline's on
the scan's outputs, twice.

Kani proves the scan's flip-flops edge by edge (`netlist_scan_flags`,
`KaniScanFlags`: from `flags_at h`, the edge `h + 1` leaves them at
`flags_at (h + 1)` with the outputs `outputs_at` has, which are
`scan_inputs`'s), so from the scan's states that carry `flags_at`
(`FlagsAt`) the pipeline fed the scan's outputs is the pipeline fed
`scan_inputs` (`line_scan_dot`). After the scan, ended, the scan stays
ended through mode 3 and HBlank and gives render nothing
(`netlist_scan_ended`, `KaniScanEnded`: `ended_dot`).
-/

namespace ceres_core.rest
open ceres_core.ppu.display.background.relation.rest
open ceres_core.ppu.display.objects.relation
open ceres_netlist.render_edge ceres_netlist.oam_scan

/-- What `netlist_scan_flags` proves. -/
def KaniScanFlags : Prop := ∀ net h line y, flags_agrees net h line y = ok true

/-- What `netlist_scan_ended` proves. -/
def KaniScanEnded : Prop := ∀ scan h line y, ended_agrees scan h line y = ok true

/-- Render's inputs at the edge `h`: the scan's outputs after it, the clock,
and the rest (`Free`). -/
def outInputs (scan : NetScan) (h : I32) (f : Free) : Result EdgeInputs := do
  let ls ← NetScan.line_start scan
  let ss ← NetScan.stop_scan scan
  let i ← scan_inputs h f
  ok { i with line_start := ls, stop_scan := ss && !ls, ceno := scan.ceno }

/-- Where the scan's outputs are `outputs_at`'s, render's inputs are
`scan_inputs`'s. -/
theorem out_inputs_eq {scan : NetScan} {h : I32} (f : Free) (ho : outputs_at scan h = ok true) :
    outInputs scan h f = scan_inputs h f := by
  simp only [outputs_at, NetScan.line_start, NetScan.stop_scan, bind_ok, Result.ok.injEq,
    Bool.and_eq_true, decide_eq_true_eq] at ho
  obtain ⟨⟨h1, h2⟩, h3⟩ := ho
  unfold outInputs scan_inputs
  simp only [NetScan.line_start, NetScan.stop_scan, bind_ok, lift]
  rw [h3, h1]
  cases hs : scan.start_scan <;> cases ha : scan.anel <;> cases hb : scan.byba <;>
    cases hd : scan.doba <;> simp_all

/-- An edge of the scan from its flip-flops at `h`: they are at `h + 1`, with
the outputs `outputs_at` has. -/
-- @assumes scan-outputs
theorem flags_edge (k : KaniScanFlags) {net : NetSide} {h : I32} {line : Line} (y : U8)
    (hlo : -8 ≤ h.val) (hhi : h.val ≤ 161) (hly : line.ly < 144#u8)
    (hf : flags_at net.scan h = ok true) :
    ∃ h' n', h + 1#i32 = ok h' ∧ net_edge net h' line y = ok n' ∧
      outputs_at n'.scan h' = ok true ∧ flags_at n'.scan h' = ok true := by
  have hk := k net h line y
  have c1 : (h >= (-8)#i32) = true := by simp; scalar_tac
  have c2 : (h <= 161#i32) = true := by simp; scalar_tac
  have c3 : (line.ly < 144#u8) = true := by simpa using hly
  simp only [flags_agrees, hf, c1, c2, c3, bind_ok, Bool.and_self, ite_true] at hk
  obtain ⟨h', hh, k1⟩ := bind_eq_ok hk
  obtain ⟨n', hn, k2⟩ := bind_eq_ok k1
  obtain ⟨b1, hb1, k3⟩ := bind_eq_ok k2
  obtain ⟨b2, hb2, k4⟩ := bind_eq_ok k3
  simp only [Result.ok.injEq, Bool.and_eq_true] at k4
  obtain ⟨rfl, rfl⟩ := k4
  exact ⟨h', n', hh, hn, hb1, hb2⟩

/-- The scan's flip-flops as `flags_at` has them after the edge before the
next. -/
def FlagsAt (s : NetSide × I32) : Prop :=
  ∃ hp : I32, hp.val = s.2.val - 1 ∧ flags_at s.1.scan hp = ok true

/-- The netlist's dot in the scan: the scan's edge, then the pipeline's on
the scan's outputs (`outInputs`), twice. -/
def lineScan (line : Line) (s : (NetSide × I32) × NetPipeline)
    (i : ceres_core.scan.ScanIn × (Free × Free)) :
    Result (Unit × ((NetSide × I32) × NetPipeline)) := do
  let n1 ← net_edge s.1.1 s.1.2 line i.1.1
  let e1 ← outInputs n1.scan s.1.2 i.2.1
  let p1 ← NetPipeline.edge s.2 e1
  let a ← s.1.2 + 1#i32
  let n2 ← net_edge n1 a line i.1.2
  let e2 ← outInputs n2.scan a i.2.2
  let p2 ← NetPipeline.edge p1 e2
  let b ← s.1.2 + 2#i32
  ok ((), ((n2, b), p2))

open ceres_core.scan in
/-- From the scan's states that carry `flags_at`, the netlist's dot is the
scan's and the pipeline's fed `scan_inputs` (`netScan`, `pipeScan`), and
the flip-flops are carried on. -/
-- @assumes scan-outputs
theorem line_scan_dot (k : KaniScanFlags) {line : Line} {s s' : NetSide × I32} {p : NetPipeline}
    {i : ScanIn} {f : Free × Free} {q : NetPipeline × I32}
    (hfl : FlagsAt s) (hly : line.ly < 144#u8) (hlo : -7 ≤ s.2.val) (hhi : s.2.val ≤ 161)
    (hn : netScan line s i = ok ((), s')) (hq : pipeScan (p, s.2) f = ok ((), q)) :
    lineScan line (s, p) (i, f) = ok ((), (s', q.1)) ∧ FlagsAt s' := by
  obtain ⟨n, h0⟩ := s
  obtain ⟨hp, hpv, hpf⟩ := hfl
  simp only at hpv hpf hlo hhi hn hq ⊢
  obtain ⟨h1, n1, hh1, hn1, ho1, hf1⟩ := flags_edge k i.1 (by omega) (by omega) hly hpf
  have hv1 : h1.val = h0.val := by
    have := add_val hh1 (by scalar_tac) (by scalar_tac); simp at this; omega
  have hs1 : h1 = h0 := IScalar.eq_of_val_eq hv1
  rw [hs1] at hn1 ho1 hf1
  obtain ⟨a, n2, ha, hn2, ho2, hf2⟩ := flags_edge k i.2 (by omega) (by omega) hly hf1
  have hav : a.val = h0.val + 1 := by
    have := add_val ha (by scalar_tac) (by scalar_tac); simpa using this
  obtain ⟨b, hb, hbv⟩ := ceres_core.scan.i32_add (x := h0) (y := 2#i32) (by scalar_tac) (by scalar_tac)
  rw [netScan_eq line n h0 a b i hav (by simpa using hbv) (by omega), hn1] at hn
  simp only [bind_ok] at hn
  rw [hn2] at hn
  simp only [bind_ok, Result.ok.injEq, Prod.mk.injEq, true_and] at hn
  subst hn
  unfold pipeScan at hq
  simp only at hq
  obtain ⟨e1, he1, hq⟩ := bind_eq_ok hq
  obtain ⟨p1, hp1, hq⟩ := bind_eq_ok hq
  obtain ⟨a', ha', hq⟩ := bind_eq_ok hq
  rw [ha] at ha'
  simp only [Result.ok.injEq] at ha'
  subst ha'
  obtain ⟨e2, he2, hq⟩ := bind_eq_ok hq
  obtain ⟨p2, hp2, hq⟩ := bind_eq_ok hq
  obtain ⟨b', hb', hq⟩ := bind_eq_ok hq
  rw [hb] at hb'
  simp only [Result.ok.injEq] at hb'
  subst hb'
  simp only [Result.ok.injEq, Prod.mk.injEq, true_and] at hq
  subst hq
  refine ⟨?_, ⟨a, by simp [hbv, hav]; omega, hf2⟩⟩
  simp only [lineScan, hn1, out_inputs_eq f.1 ho1, he1, hp1, ha, hn2, out_inputs_eq f.2 ho2,
    he2, hp2, hb, bind_ok]

/-- An edge of mode 3 or HBlank from the scan ended: it stays ended, and
render's inputs are idle, the clock high at an odd edge. -/
-- @assumes scan-outputs
theorem ended_edge (k : KaniScanEnded) {scan : NetScan} {h : I32} {line : Line} (y : U8)
    (he : scan_ended scan = ok true) (hlo : 163 ≤ h.val) (hhi : h.val ≤ 904) (f : Free) :
    ∃ s1 e, scan_edge_at scan h line y = ok s1 ∧ scan_ended s1 = ok true ∧
      outInputs s1 h f = ok e ∧ idle_inputs e = ok true ∧ (h.val % 2 = 1 → e.clk = true) := by
  have hk := k scan h line y
  have c1 : (h >= 163#i32) = true := by simp; scalar_tac
  have c2 : (h <= 904#i32) = true := by simp; scalar_tac
  simp only [ended_agrees, he, c1, c2, bind_ok, Bool.and_self, ite_true] at hk
  obtain ⟨s1, hs1, k1⟩ := bind_eq_ok hk
  obtain ⟨b1, hb1, k2⟩ := bind_eq_ok k1
  obtain ⟨b2, hb2, k3⟩ := bind_eq_ok k2
  obtain ⟨b3, hb3, k4⟩ := bind_eq_ok k3
  simp only [Result.ok.injEq, Bool.and_eq_true, decide_eq_true_eq, Bool.not_eq_true] at k4
  obtain ⟨⟨⟨rfl, rfl⟩, rfl⟩, hc⟩ := k4
  obtain ⟨r, hr, hrv⟩ := ceres_core.scan.spec_ok_exists (IScalar.rem_spec h (y := 2#i32) (by simp))
  refine ⟨s1, ⟨r != 0#i32, false, false, f.lcdc, f.ly, f.hclk, f.mode1, false, false, false,
    f.cpu_wr_sync, f.addr, f.data⟩, hs1, hb1, ?_, ?_, ?_⟩
  · simp [outInputs, hb2, hb3, scan_inputs, hr, hc]
  · simp [idle_inputs]
  · intro hodd
    have : r.val = 1 := by
      have h2 : ((2#i32 : I32) : Int) = 2 := by simp
      rw [hrv, h2, Int.tmod_eq_emod_of_nonneg (by omega)]
      omega
    simp only [bne_iff_ne, ne_eq]
    intro h0
    rw [h0] at this
    simp at this

/-- The scan's dot in mode 3 and HBlank: its two edges alone (OAM's latches
are the sprite fetch's), OAM giving `y`. -/
def scanIdle (line : Line) (s : NetScan × I32) (y : U8 × U8) : Result (NetScan × I32) := do
  let s1 ← scan_edge_at s.1 s.2 line y.1
  let a ← s.2 + 1#i32
  let s2 ← scan_edge_at s1 a line y.2
  let b ← s.2 + 2#i32
  ok (s2, b)

/-- A dot of mode 3 or HBlank keeps the scan ended. -/
-- @assumes scan-outputs
theorem scan_idle_dot (k : KaniScanEnded) {line : Line} {s : NetScan × I32} (y : U8 × U8)
    (he : scan_ended s.1 = ok true) (hlo : 163 ≤ s.2.val) (hhi : s.2.val ≤ 903) :
    ∃ s', scanIdle line s y = ok s' ∧ s'.2.val = s.2.val + 2 ∧ scan_ended s'.1 = ok true := by
  obtain ⟨n, h⟩ := s
  simp only at he hlo hhi ⊢
  obtain ⟨s1, -, h1, he1, -⟩ := ended_edge (line := line) k y.1 he hlo (by omega) ⟨0#u8, 0#u8, false, false, false, 0#u16, 0#u8⟩
  obtain ⟨a, ha, hav⟩ := ceres_core.scan.i32_add (x := h) (y := 1#i32) (by scalar_tac) (by scalar_tac)
  obtain ⟨s2, -, h2, he2, -⟩ := ended_edge (line := line) (h := a) k y.2 he1 (by simp at hav; omega) (by simp at hav; omega)
    ⟨0#u8, 0#u8, false, false, false, 0#u16, 0#u8⟩
  obtain ⟨b, hb, hbv⟩ := ceres_core.scan.i32_add (x := h) (y := 2#i32) (by scalar_tac) (by scalar_tac)
  exact ⟨(s2, b), by simp [scanIdle, h1, ha, h2, hb], by simpa using hbv, he2⟩

/-- The netlist's dot in HBlank: the scan's edge, then the pipeline's on
the scan's outputs, twice. -/
def lineIdle (line : Line) (s : (NetScan × I32) × NetPipeline) (i : (U8 × U8) × (Free × Free)) :
    Result (Unit × ((NetScan × I32) × NetPipeline)) := do
  let n1 ← scan_edge_at s.1.1 s.1.2 line i.1.1
  let e1 ← outInputs n1 s.1.2 i.2.1
  let p1 ← NetPipeline.edge s.2 e1
  let a ← s.1.2 + 1#i32
  let n2 ← scan_edge_at n1 a line i.1.2
  let e2 ← outInputs n2 a i.2.2
  let p2 ← NetPipeline.edge p1 e2
  let b ← s.1.2 + 2#i32
  ok ((), ((n2, b), p2))

/-- A dot of HBlank, at an odd edge, from the scan ended and the pipeline at
rest or just stopped: the scan stays ended, the pipeline rests. -/
-- @assumes scan-outputs
-- @assumes pipeline-rests
theorem line_idle_dot (k : KaniScanEnded) (kEnd : KaniRestEnd) (kH : KaniRestHBlank)
    {line : Line} {s : (NetScan × I32) × NetPipeline} (i : (U8 × U8) × (Free × Free))
    (he : scan_ended s.1.1 = ok true) (hodd : s.1.2.val % 2 = 1) (hlo : 163 ≤ s.1.2.val)
    (hhi : s.1.2.val ≤ 903) (hp : HRest s.2) :
    ∃ r, lineIdle line s i = ok ((), r) ∧ r.1.2.val = s.1.2.val + 2 ∧
      scan_ended r.1.1 = ok true ∧ hblank_rest r.2 = ok true := by
  obtain ⟨⟨n, h⟩, p⟩ := s
  simp only at he hodd hlo hhi hp ⊢
  obtain ⟨s1, e1, h1, he1, ho1, hi1, hc1⟩ := ended_edge (line := line) k i.1.1 he hlo (by omega) i.2.1
  obtain ⟨a, ha, hav⟩ := ceres_core.scan.i32_add (x := h) (y := 1#i32) (by scalar_tac) (by scalar_tac)
  simp at hav
  obtain ⟨s2, e2, h2, he2, ho2, hi2, -⟩ := ended_edge (line := line) (h := a) k i.1.2 he1 (by omega) (by omega) i.2.2
  obtain ⟨b, hb, hbv⟩ := ceres_core.scan.i32_add (x := h) (y := 2#i32) (by scalar_tac) (by scalar_tac)
  obtain ⟨p', hp', hr'⟩ := pipe_hblank_dot (i := (e1, e2)) kEnd kH hp (hc1 hodd) hi1 hi2
  simp only [pipeIdle] at hp'
  obtain ⟨p1, hp1, k1⟩ := bind_eq_ok hp'
  obtain ⟨p2, hp2, k2⟩ := bind_eq_ok k1
  simp only [Result.ok.injEq, Prod.mk.injEq, true_and] at k2
  subst k2
  exact ⟨((s2, b), p2), by simp [lineIdle, h1, ho1, hp1, ha, h2, ho2, hp2, hb],
    by simpa using hbv, he2, hr'⟩

/-- From `-8` on and from `158`, `flags_at` is the scan ended. -/
theorem flags_ended {scan : NetScan} {h : I32} (hf : flags_at scan h = ok true)
    (h158 : 158 ≤ h.val) : scan_ended scan = ok true := by
  have c : ((h <= (-7)#i32) || (h >= 158#i32)) = true := by simp; right; scalar_tac
  simp only [flags_at, c, ite_true] at hf
  exact hf

/-- The scan's block in mode 3 and HBlank, with the line it is on. -/
def scanBlock (s : Line × (NetScan × I32)) (y : U8 × U8) : Result (Line × (NetScan × I32)) := do
  let r ← scanIdle s.1 s.2 y
  ok (s.1, r)

/-- The scan in mode 3 and HBlank: ended, at an odd edge from `163` to `903`. -/
def ScanIdle (s : Line × (NetScan × I32)) : Prop :=
  scan_ended s.2.1 = ok true ∧ s.2.2.val % 2 = 1 ∧ 163 ≤ s.2.2.val ∧ s.2.2.val ≤ 903

/-- Short of the line's last dot, the scan's block keeps it idle. -/
-- @assumes scan-outputs
theorem scan_block_dot (k : KaniScanEnded) {s : Line × (NetScan × I32)} (hs : ScanIdle s)
    (hcut : s.2.2.val ≤ 901) (y : U8 × U8) :
    ∃ s', scanBlock s y = ok s' ∧ ScanIdle s' := by
  obtain ⟨he, hodd, hlo, hhi⟩ := hs
  obtain ⟨r, hr, hrv, hre⟩ := scan_idle_dot (line := s.1) k y he hlo hhi
  refine ⟨(s.1, r), by simp [scanBlock, hr], hre, ?_, ?_, ?_⟩ <;> simp only <;> omega

end ceres_core.rest
