# PPU FIFO rework — handoff notes (2026-08-28)

## UPDATE 2026-09-01 (session 2 final) — committed through ce9ce4fe; 141/32

Session 2 commits: `829337a5` (**scan sprites at mode-2 END instead of entry** —
+2: intr_2_0_timing, lcdon_mode_timing) and `ce9ce4fe` (instrumentation strip).
All debug traces are now removed from the tree; re-add as documented below when
debugging.

### New findings this session

1. **Sprite scan timing matters**: hardware reads OAM incrementally through the
   whole mode-2 scan (dots 4-84); the intr_2_mode0_timing_sprites ROM writes its
   sprite OAM from the mode-2 STAT ISR, so an entry-time scan misses it.
   scan_sprites now runs at mode-3 entry (`enter_mode` Drawing arm).
2. **Mealybug m3_scx_low_3_bits mechanism fully decoded** (source in
   `external/test-sources/mealybug-tearoom-tests/src/ppu/m3_scx_low_3_bits.asm`):
   the CPU sits in a 1200-NOP slide; EVERY line's STAT mode-2 IRQ re-enters at
   `jp hl` → lcdc_handler, which writes SCX=0, delays 2 nops on rows < 72
   (LY < $48), then writes SCX=2. Our run shows the writes at
   `mode=2 cyc=54 (SCX=0)` and `mode=3 cyc=374 (SCX=2, = dot 2)` for rows < 72,
   and `mode=2 cyc=2` for rows ≥ 72 — the visible content is ALREADY 95%
   correct. The residual 320 px: hardware samples SCX's low 3 bits PER FETCH at
   the B01 read (a mid-line write cascades 2 px shifts through later fetches —
   SameBoy's `line_has_fractional_scrolling` territory); we latch the fraction
   once per line at mode-3 entry.
3. **A "shifted-first-tile" SCX model was tried and reverted** (136/37, worse):
   pushing the first tile with its first k pixels discarded + an align stall
   broke scx1/2/5/6 nops that the junk-phase model passes. The empirical
   junk-phase model (`junk_at = 6 - ((k+1) & 3)`) stands until something
   beats 141.
4. **TRAP for future debugging**: `dump_frame` example runs a DIFFERENT config
   than the harness (env-dependent paths differ) — env-gated traces through
   dump_frame gave completely wrong answers (zero hits where the harness showed
   290 events). Always trace through `cargo test -- --nocapture`.

### Remaining 32 fails (grouped; 4 pre-existing at baseline)

- acid2 trio (dmg-acid2 residual 376 px: mouth rows 40-55 overlapping 8x16
  sprites swap black/gray at x=64-72; footer rows 120-127 3 px/row).
- mealybug window family ×6 (see item 2 above for scx_low_3_bits; window tests
  need the SameBoy window state machine port).
- intr_2 sprites variants ×6 + scx3/scx7 nops ×2 (per-fetch SCX low-bit
  sampling cascade, item 2).
- hblank_ly_scx ×3, lcdon_timing_gs ×2, vblank_if_timing, stat_write_if_gs,
  halt_ime0, halt_ime1_timing2_gs, di_timing_gs, intr_1_2_timing_gs.

## UPDATE 2026-09-01 — mode-3 lengths EXACT + intr_2 core family green (superseded)

Commits this session: `0ee76e39` (position model + sprite FIFO v2 + STAT knob),
`af02ae2f` (sprite FIFO pop wrap %8→%16), `fc905267` (8-dot fetch cycle + lead-in
column selection). **Suite: 139 pass / 34 fail** (HEAD at session start: 138/35).

### The three bugs behind the +8 px BG shift (fixed in fc905267 + af02ae2f)

1. **Sprite FIFO pop wrapped at %8** after the ring grew to 16 slots — pops past
   slot 7 read stale entries (broke every sprite whose pixels landed past slot 7).
2. **6-dot fetch cycle**: GetTileDataHigh pushed and jumped straight to GetTile
   when the FIFO had room, so the fetcher lapped the 1 px/dot pops and injected a
   duplicate tile row → every BG line shifted +8 px wherever the BG had content.
   High now ALWAYS hands to Push; Push pushes when bg_len <= 8. Cycle = 8 dots,
   phase-locked to the pops.
3. **Lead-in column selection** (SameBoy display.c:939-941): during the first
   half of the lead-in (raw position −16..−9) the fetched column is `SCX >> 3`,
   NOT the wrapped-position formula (which computes col 31 garbage there).
   From raw −8 on: `col = (SCX + position + 8) / 8` (the +8 = FIFO lead).

dmg-acid2 mismatched pixels: 10089 → 376 (residual: mouth-overlap 8x16 sprites
rows 40-55, and 3 px/row in the footer rows 120-127).

### Debug technique that worked (reusable)

Knob-sweeping: make the uncertain dot/duration a `const`, loop
`sed -i` + `cargo nextest run` over candidate values, count passes. Assert-failing
mooneye tests exit in ~0.2 s so each sweep step is fast. Frame-exact PNG diffs:
use the harness's own `timeout_frames` (acid2 = 480, mealybug = 500,
sprite_priority = 7160) with `dump_frame`; arbitrary counts catch transients.
`diff_png <expected> <actual>` prints per-row expected/got.

### Environment-gated traces

7 `CERES_TRACE`-gated eprintln sites remain committed (fetcher BG fetch trace +
CERES_TRACE_LY, fifo scan/fetch/PX traces, ppu LEN trace via CERES_LEN_TRACE).
They are needed for the remaining pixel work; strip in the final cleanup commit.

### Remaining 34 fails, grouped

- 4 pre-existing at baseline: cgb_acid_hell, vblank_stat_intr_c/gs,
  hblank_ly_scx_timing_variant_nops.
- acid2 trio: dmg-acid2 down to 376 px (mouth rows 40-55: overlapping 8x16
  mouth sprites swap black/gray at x=64-72 — X-priority at overlap; footer
  rows 120-127: 3 px/row). cgb-acid2 similar. Trace ly==40 fetches + PX stream
  next; suspects: overlay merge order vs SameBoy's `fifo_overlay_object_row`,
  and LCDC bit-3 (bg map) mid-frame toggle for the footer.
- mealybug window family ×6 (m2_win_en_toggle, m3_scx_low_3_bits,
  m3_wx_4_change_sprites, dmg+cgb): m3_scx_low_3_bits is CLOSE — 320 px, only
  the last ~10 columns per line differ by 2 px (mid-line SCX write vs the
  junk_at/snap phase latched at mode-3 entry from the OLD scx). The window
  tests need the SameBoy window state machine port (follow-up plan).
- intr_2_mode0_timing_sprites ×6: mode-3 extension per sprite (should be
  exactly +6 dots each); sprite-stall duration sweep was flat — the stall
  PHASE vs the fetch cycle is the suspect.
- intr_2_mode0_scx3/scx7_timing_nops, hblank_ly_scx ×3, lcdon ×3,
  vblank_if_timing, stat_write_if_gs, halt_ime0, halt_ime1_timing2_gs,
  di_timing_gs, intr_0_timing, intr_1_2_timing_gs.

### Old 2026-09-01 update (superseded details)

## UPDATE 2026-09-01 — mode-3 lengths EXACT + intr_2 core family green (UNCOMMITTED WIP)

Suite: **137 pass / 36 fail** (HEAD `66354e25` was 138/35; this session started at 132/41).
Tally history: `/tmp/suite-tally.txt`. State: all changes uncommitted on top of `66354e25`.

### What landed since the last update

1. **All mode-3 lengths are now hardware-exact** (172/172/172/176/176/176/176/180
   for `scx&7` = 0..7, DMG). The key was the junk-push *phase*:
   `junk_at = 6 - ((scx&7 + 1) & 3)` in `fifo/mod.rs::start_drawing`. The SCX
   fraction consumes k real pixels during the lead-in (pop k+1 snaps), which
   delays first-visible by k dots — hardware hides this by shifting the pipeline
   phase so first-visible is always M-cycle aligned (13/13/13/17/17/17/17/21).
   This is forced by three constraints together: pixel mapping (first visible =
   tile pixel k), the mooneye length table, and M-cycle alignment of output start.
2. **The whole intr_2 core family passes**: intr_2_0_timing, intr_2_mode0_timing,
   intr_2_mode0_scx1/2/4/5/6/8_timing_nops, intr_2_timing, intr_2_mode3_timing,
   intr_2_oam_ok_timing. Fix: the DMG mode-2 (OAM) STAT IRQ fires from a
   standalone block at **HBlank `cycles == OAM_IRQ_AT = 7`** (swept; plateau
   5-7, old value 4 → 0/17). The comparator restore stays at `cycles == 4`
   (the two were split — moving them together broke the ly_lyc family).
   `update_stat_line`'s None+OamScan branch returns `false` (the entry-fire is
   superseded by the dot-7 block).
3. **Internal `mode: Mode` field** in `Ppu` (set in `enter_mode`/`write_lcdc`/
   `set_line_mode`/frame-wrap; `mode()` returns it). STAT mode bits are now pure
   readback. Infrastructure for decoupling CPU-visible mode from internal state.

### IMPORTANT NEGATIVE RESULT — do NOT retry the "line-phase +4" restructure

A SameBoy-derived restructure (OamScan 84 dots, mode-2 bits at line-dot 4, mode 3
at dot 84, HBlank −4) was tried and **collapsed the suite to 108/65**. The 20+
hardware-verified LY/LYC/ly00/ly143 tests encode OUR existing line phase (mode-2
bits at the boundary). SameBoy's internal dots (LY at "dot 3", bits at "dot 4")
do NOT translate to CPU-visible relations by naive +4 offsetting — SameBoy
compensates elsewhere. Any future phase work must start from what the
hardware-calibrated tests measure, not from SameBoy internals. (The internal
`mode` field from that experiment was kept — it is correct infrastructure.)
Also: the DMG HBlank-IRQ delay knob has no effect on hblank_ly_scx (sweep flat
1/16 across 0-3) and breaking the entry-fire kills intr_2_0 — reverted.

### Remaining 36 fails, grouped

- 4 pre-existing at baseline: cgb_acid_hell, vblank_stat_intr_c/gs,
  hblank_ly_scx_timing_variant_nops.
- **10 pixel tests (the big rock)**: acid2 trio, manual_sprite_priority ×2,
  mb_m2_win_en_toggle ×2, mb_m3_scx_low_3_bits ×2, mb_m3_wx_4_change_sprites ×2.
  Sprite/window alignment in the new FIFO: sprites fetch at the right
  position-match dots now, but overlap merges land 1-2 px off. Next step:
  tile-level truth — dump the failing ROM's tile $10 rows and compare pixel-by-
  pixel against the reference (the tests differ only in sprite overlap spans).
- intr_2_mode0_timing_sprites ×6: sprite-stall mode-3 extension (each sprite
  should add 6 dots) — check the sprite-fetch stall timing vs position match.
- intr_2_mode0_scx3/scx7_timing_nops: the k=3/7 M-cycle step cases — junk_at
  is latched at mode-3 entry from the OLD scx; these tests write SCX mid-mode-3,
  so the live snap uses the new fraction while junk_at is stale.
- lcdon ×3, hblank_ly_scx ×3, vblank_if_timing, stat_write_if_gs, halt_ime0,
  halt_ime1_timing2_gs, di_timing_gs, intr_0_timing, intr_1_2_timing_gs.
- The `OAM_IRQ_AT` / junk-phase constants are marked TEMPORARY in the source —
  fold them into a documented ladder before the final commit.

### DEBUG TECHNIQUE THAT WORKED

Knob-sweeping: make the uncertain dot a `const`, then loop
`sed -i` + `cargo nextest run` over candidate values, counting passes
(`/tmp/sweep.sh`). The tests fail fast (asserts exit early) so each sweep step
is ~0.2s. This found OAM_IRQ_AT=7 in one pass after two failed hand-derivations
from SameBoy internals. Trust the sweeps; treat SameBoy dot math as a hypothesis
generator only.

(Older 2026-08-31 update follows.)

## UPDATE 2026-08-31 — SameBoy position model landed (UNCOMMITTED WIP)

Work done in a stabilization pass (plan: `~/.claude/plans/ethereal-strolling-parrot.md`).
Suite tally history is in `/tmp/suite-tally.txt`. Per-commit test matrices and the
non-compiling-commit findings are recorded in the session transcript.

**State: working tree has the changes below; branch HEAD is still `66354e25` (132/173 pass).
Working tree: 132/173 (same count, different composition: gained
`intr_2_mode0_scx1/scx5_timing_nops`, lost 8 pixel tests pending the alignment work below).**

What changed (all in `ceres-core/src/ppu/`):

1. **`fifo/mod.rs` — SameBoy `position_in_line` port.** `lx` is no longer the FIFO
   clock; a new `position: i16` field (−16..=160) counts every FIFO pop.
   - `step_dot` pops whenever `bg_len > 0` (the old `bg_len > 8` gate is GONE — it was
     the cause of the linear `171 + scx&7` mode-3 lengths).
   - 8 junk pixels are pushed at `line_dots == 5` (SameBoy display.c:1851); they are
     consumed by the lead-in drops. This dot is the calibration constant that makes
     DMG scx=0 mode-3 length exactly 172.
   - SCX discard is modeled by SameBoy's lead-in snap (display.c:686-704):
     `(position as u8 & 7) == (scx & 7)` snaps position to −8. The old
     `scx_discard` counter and an intermediate `fetch_stall` experiment were removed.
   - Measured lengths now: scx&7 0→172 ✓, 4→176 ✓, 8→172 ✓, but 1→173 ✗(172),
     5→177 ✗(176) — k≥1 is one pop late per fraction unit. SameBoy must compensate
     somewhere we haven't found yet (suspects: `line_has_fractional_scrolling`
     display.c:702, or the snap firing one pop earlier than we model). FIXING THIS
     IS THE FIRST TASK — intr_2_mode0_scx1 passes *only because* its +1 error
     cancels a ladder error; k=0/4/8 lengths are exact but their tests still fail
     on the ladder (next item).
2. **`fifo/mod.rs` — sprite FIFO v2.** Ring is 16 entries; fetches match on
   `position + 8 == sprite.x` (i16, `x_for_object_match`), not the old `lx + 8`
   (which froze at 0 during the lead-in and fetched edge sprites ~12 dots early).
   `overlay_sprite_pixels` writes position-aligned slots (`sprite_x - 8 + j -
   position`), pads in-between with empties, merges overlaps (DMG first-opaque-wins,
   CGB lower `sprite_priority`). `sprite.rs` lost `effective_x` (raw X compared in
   i16 position space); the X<8 "cut" is now automatic (off-screen pixels consumed
   by lead-in drops). 6-dot stall and the row-boundary wait are unchanged.
3. **`fetcher.rs`** — BG map column = `(SCX + position as u8 + 8) / 8 & 0x1F`
   (display.c:943), replacing the `lx + bg_len + scx` formula; `lx` param renamed
   `position`. `mod.rs` uses `fifo.line_done()` instead of `fifo.lx() >= 160`.
4. **`ppu/mod.rs`** — all session instrumentation removed; no functional change.

**Known-broken (8 pixel tests): `dmg_acid2_dmg/cgb`, `cgb_acid2`, `manual_sprite_priority_dmg/cgb`,
`mb_m2_win_en_toggle_*`, `mb_m3_scx_low_3_bits_*`, `mb_m3_wx_4_change_sprites_*`.**
Diagnosis so far (from line-48 traces of sprite_priority): sprites DO fetch and
render, but the overlap merge lands 1-2 px off (x=8 transparent where the reference
has gray; x=11 opaque where it should end). Suspects, in order:
- the k≥1 snap being one pop late (same root as the mode-3 length anomaly — fix
  that first, it may fix all of these),
- the window-glitch predicate still present in `step_dot` (d5129cad's fitted
  conditions — Phase 2 removes them; they reference `is_get_tile()` whose meaning
  was also bent),
- `is_ready_for_sprite_fetch()` waiting for a row boundary while `position` keeps
  ticking (SameBoy freezes rendering during the whole object fetch, we only pause
  output — check whether position should freeze too).

Debug tooling: `diff_png` example was restored to a real pixel diff
(`diff_png <expected> <actual>`, prints per-row expected/got). WARNING: dump_frame
comparisons are frame-phase-sensitive — a 60-frame dump mismatches a settled
reference spuriously; the test harness itself is the only reliable oracle.

Branch `pixel-fifo`, HEAD was `4b292a8f`. This file documents an in-progress
architectural fix to the PPU. **The work is ~90% done but the final full test
run after the last one-line fix has not been executed yet — that is the first
thing to do.**

## Background: why this rework

Baseline `8320f2fd` passed 164/168 tests (4 pre-existing failures:
`cgb_acid_hell`, `vblank_stat_intr_gs`, `vblank_stat_intr_c`,
`gpu_hblank_ly_scx_timing_variant_nops`). The 12 commits after it (mealybug +
FIFO work) broke 5 previously-passing tests (all 4 acid2 + both
`sprite_priority`) while gaining 1 mealybug pass.

Root causes identified (verified by per-commit bisect + pixel diffs):

1. **Dropped sprites**: `20a90fa7` gated the sprite fetch on a *coincidence*
   (`is_ready_for_sprite_fetch()` must hold at exactly the dot when
   `lx == sprite.x - 8`). When the fetcher was mid-tile-row at that dot the
   sprite was silently dropped forever. This caused both `sprite_priority`
   failures (missing gray X=12 sliver at x=7..10) and part of the acid2 wedge
   (missing OBJ-priority "eye" objects).
2. **Dual clock**: Mode 3 length was a formula (`m_cycles(scx)` + the ~275-line
   `sprite_penalty_m_cycles` heuristic) and the FIFO was force-drained
   (`while lx < 160`) at formula expiry. Two clocks fighting; every fix
   shuffled symptoms.
3. **PPU dot leak**: `advance_dots` advanced the PPU only whole M-cycles and
   dropped `cpu_t_cycles % 4` dots. The sub-M-cycle conflict dispatch
   (`write_cpu`) flushes 3/5/6-dot values, so every conflict-mapped register
   write desynced CPU/PPU.

Reference model: SameBoy `external/reference-implementations/SameBoy/Core/display.c`
(user explicitly pointed at it). Key facts decoded from it:

- Sprite match: `x_for_object_match() == position_in_line + 8`; sprites with
  smaller x are *dropped* (display.c:1946-1949); ALL sprites matching the same
  x are fetched (while loop, display.c:1952-2026), each costing 6 dots.
- On a match the fetcher first *finishes the current BG tile row*
  (`while fetcher_state < GET_TILE_DATA_HIGH_T2 || fifo empty`), pausing pixel
  output — it never drops the sprite.
- Mode 3 length is an *output* of the simulation (`cycles_for_line`), never an
  input formula.

## What was changed (all uncommitted, working tree)

### `ceres-core/src/ppu/fifo/mod.rs` — `step_dot` rewrite
- Sprite trigger now: `match_x = lx + 8`; `sprites.discard_behind(match_x)`;
  if `next_x() == Some(match_x)` and obj-enabled (`lcdc & 0x02 != 0 || is_cgb`):
  - if fetcher at a row boundary (`is_ready_for_sprite_fetch()` = Push|GetTile
    with cycle 0) → pop + `fetch_sprite_data` (starts the existing 6-dot
    stall). Sprites sharing the same x are fetched on successive dots.
  - else → `output_paused = true` for this dot (fetcher steps, finishes the
    row; pixel output waits so the overlay stays aligned).
- `scan_sprites(&oam, ly, sprite_height)` — dropped `is_cgb`/`opri` params.
- `step_dot` no longer takes `scx` (fetcher tracks its own tile column).

### `ceres-core/src/ppu/fifo/sprite.rs`
- `take_sprite_at` replaced by `discard_behind(match_x)` / `next_x()` /
  `pop_next()`; `effective_x(x) = max(x, 8)` mapping (sprites at/left of the
  screen edge all match once output begins).
- `scan_line` now **always** sorts ascending by x (stable ⇒ ties keep OAM
  order). Fetch order is x-ascending on both DMG and CGB; pixel priority is
  resolved in the overlay (first-opaque-wins = DMG x-priority; CGB compares
  `oam_index`) so always-sorting is safe. (Verified against Pan Docs:
  DMG = x-priority, CGB = OAM-index priority, OPRI bit flips CGB to DMG style.)

### `ceres-core/src/ppu/fifo/fetcher.rs`
- `step_t_cycle` signature: removed unused `_scx`, `_lx` params.
- GetTile bg branch now uses the fetcher's own `bg_tile_x` counter (reset from
  SCX at line start, incremented every push). The old lx-based recomputation
  could fetch the same tile twice whenever output stalled behind the fetcher.

### `ceres-core/src/ppu/mod.rs` — FIFO owns Mode 3; dot-granular PPU
- `tick_m_cycle` → **`tick_t_cycle`** (1 dot per call). `self.cycles` is now a
  DOT countdown; every previously M-cycle-calibrated constant is
  `* DOTS_PER_M` (=4): OamScan latch `19*4`, HBlank LY events `2*4`/`1*4`,
  VBlank line-153 phases `114/113/112 * 4`, `base_cycles - 1` → `-4`, the DMG
  early-hblank STAT IRQ `cycles == 4`, `read_ly`/`write_lyc` special cases,
  `write_lcdc` (`20*4`, `HBlank.m_cycles*4`), `set_line_mode` caller in lib.rs
  (`65*4`), `update_stat_line`'s `cgb_hblank_delayed`.
- **Mode 3 = watchdog only**: `enter_mode(Drawing)` sets
  `cycles = MODE3_MAX_DOTS` (= 456 - 80 = 376). In the Drawing arm, one
  `step_fifo_dot(cgb_mode)` per dot; when `fifo.lx() >= 160` the measured
  length becomes the penalty:
  `sprite_penalty = mode3_dots - Drawing.m_cycles(scx)*4`, then `cycles = 1`
  so the mode-expiry block enters HBlank with
  `cycles = HBlank.m_cycles*4 - sprite_penalty` (line still totals 456 dots).
  A safety drain remains in the expiry block (watchdog path).
- `sprite_penalty_m_cycles` (the ~275-line heuristic) **deleted**.
- `write_scx` no longer re-times mode 3 (SCX changes flow through the FIFO).
- New helper `step_fifo_dot` (steps FIFO + resolves + writes framebuffer px).

### `ceres-core/src/timing/mod.rs` — per-dot PPU ticking with credit
`advance_dots` now banks T-cycles in `Gb::ppu_t_credit` and ticks
`ppu.tick_t_cycle` once per **1** CPU T-cycle (2 in double speed). No dots are
ever dropped, so the sub-M-cycle conflict offsets in `write_cpu` land at exact
dots. (Replaced the old `ppu_dskip` parity hack; field renamed
`ppu_t_credit` in `lib.rs`.)

## Where it stands RIGHT NOW

Verified mid-rework (before the final fix):
- **`test_dmg_acid2_dmg` PASSES pixel-perfect (0 mismatches)**
- **`test_manual_sprite_priority_dmg` and `_cgb` PASS** (dropped-sprite fix works)
- `test_mb_m2_win_en_toggle_dmg_blob` PASSES
- CGB-mode tests (`cgb_acid2`, `dmg_acid2_cgb`, `m2_win_en_toggle_cgb_c`)
  were failing with a wild symptom: CGB OAM all `0xFF`, CPU stuck in the **CGB
  boot ROM's VBlank wait** (`cgb.bin` 0x0212: `push af; ld hl,$FF0F; res 0,(hl);
  bit 0,(hl); jr z,-4`).

**Root cause found**: the credit loop charged **4 credits per PPU tick**
(leftover from M-cycle semantics) while `tick_t_cycle` advances only 1 dot ⇒
**the PPU ran at exactly 1/4 speed**. Event-driven ROMs self-synchronize (DMG
acid2 still rendered), but fixed-frame-count dumps / boot-Rom VBlank waits
broke. Verified empirically: 10 frames = 702240 `dots_ran` but only ~175k PPU
ticks; PPU mode machine advanced ~4 lines.

**The fix (last edit, NOT YET TESTED)**: `t_cycles_per_dot` changed
`{2}else{4}` → `{2}else{1}` in `timing/mod.rs::advance_dots`.

## NEXT STEPS (in order)

1. **Run the full suite**: `cargo nextest run -p ceres-test-runner --no-fail-fast`.
   Expect: the 5 regressions fixed, 4 pre-existing failures unchanged. If CGB
   acid2 still differs, pixel-diff (see tooling below) — the fetch machinery is
   now sound, so remaining diffs should be small calibration issues (e.g. the
   `bg_len > 8` output gate, or fetch timing offsets).
2. **Watch out for `MODE3_MAX_DOTS` watchdog**: if any test shows lines that
   render only partially and then the rest of the line is "teleported", the
   FIFO stalled and the watchdog fired. Add a temporary eprintln in the
   watchdog branch (expiry `Mode::Drawing` arm) to detect it.
3. **Then try enabling mealybug m3 tests** in
   `ceres-test-runner/tests/mealybug_tests.rs` (all `#[ignore]`d): start with
   `m3_bgp_change` (BGP is latched at pixel pop → only needs correct write-dot
   timing, which the conflict dispatch + per-dot PPU now provide), then
   `m3_scx_*`, `m3_lcdc_*`, window tests, sprite tests last. Enable one family
   at a time via the `ignore` flag removal; note some DMG references are
   missing upstream (comments already in the file).
4. **Known deferred items** (documented in code comments):
   - `fifo/mod.rs` window-glitch condition contains `wx == ly` (WX register
     compared to LY) — almost certainly wrong/typo, near-dead code, but
     `m2_win_en_toggle` passes with it; revisit when doing mealybug window
     tests.
   - Mid-line SCX writes: the fetcher latches `bg_tile_x` only during the
     first 6 dots (`set_scx`). SameBoy instead computes each GetTile column
     from live `SCX + position_in_line` — needed for `m3_scx_*` tests.
   - `sprite_priority` in overlay uses `oam_index` on CGB; DMG uses fetch
     order (first opaque wins). Both verified vs Pan Docs.
   - The `bg_len > 8` pop gate and the 6-dot sprite stall are SameBoy-shaped
     but not yet validated against the m3 sprite tests.

## Debug tooling added (untracked, in `ceres-test-runner/examples/`)

- `dump_frame.rs` — run a ROM N frames, write framebuffer PNG:
  `cargo run -p ceres-test-runner --example dump_frame -- <rom-rel-path> dmg|cgb <frames> <out.png>`
- `diff_png.rs` — pixel diff, prints mismatched coords per row.
  **WARNING: its `expected=`/`got=` labels are SWAPPED** (args are
  `<reference> <actual>` but it prints them backwards). Trust the test
  harness's own mismatch output over this tool's labels.
- `crop_png.rs` — crop+magnify a region for visual inspection.

Useful technique used extensively: temporary `std::env::var_os("CERES_TRACE")`-gated
`eprintln!` instrumentation in `tick_t_cycle` / `step_dot` / `write_lcdc` /
`run_cpu` + `run_frame`-count sampling of PC (add a `pub fn debug_pc` to `Gb`
reading `self.cpu.pc()`). All such instrumentation has been removed; the tree
is clean of debug code.

## Gotchas learned the hard way

- `run_frame` = run CPU until 70224 T-cycles consumed (`dots_ran`); it is
  wall-clock frame count, NOT PPU frame count. If PPU rate ≠ CPU rate, dumps at
  fixed frame counts show unfinished screens. Several false leads (sprites
  "missing", "OAM never written") were actually the 1/4-speed PPU.
- DMG acid2 draws "Hello World" only after ~150 frames; dumps at 60 frames
  look broken even when everything is correct.
- The dev profile is `opt-level = 1, debug-assertions = true`; `std::env` is
  available in ceres-core despite `extern crate alloc`.
- `external/test-roms` is gitignored (auto-downloaded) and `external/*`
  submodules are not initialized in fresh worktrees — symlink the whole
  `external/` dir when testing a worktree.
- SameBoy `position_in_line` semantics (in case more m3 work is needed): starts
  at -16 at mode-3 start, advances only on FIFO pops; render output starts
  around pos 0..12; sprite match at pos+8. Ceres' `lx` is the same counter
  restricted to 0..160 for rendered pixels; the `+8` in `match_x` encodes the
  FIFO lead.
