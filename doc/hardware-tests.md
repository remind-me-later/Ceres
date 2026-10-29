# Behaviour to check on hardware

What ceres does rests on three kinds of evidence, and each settles different
things:

- **The netlist** of the DMG-CPU B ([dmg-schematics], traced from die photos),
  simulated by [dmg-sim] and described by the readable model in `hdl/`. It
  settles the chip's logic: what is connected to what. It does not settle
  timing: its delays are a model fitted to measurements of one chip, so where
  the outcome of a race between two signals is a matter of nanoseconds, its
  answer is a guess. dmg-sim also starts every cell at 0, models OAM and wave
  RAM as plain SRAM, and forces the OAM data lines where two drivers meet (see
  [hdl/README.md](../hdl/README.md#limits)). There is no netlist of the CGB's
  chip.
- **Test ROMs verified on real consoles** (gambatte, gbmicrotest, Mooneye,
  Mealybug Tearoom, AGE, Blargg, SameSuite). They settle what the CPU or the
  screen sees, on the unit the test was made on, at the points the test
  happens to look. Where they leave a CGB timing undecided, ceres picks a
  value that passes them all.
- **Other emulators** (SameBoy, Gambatte). Where they only agree with each other,
  that is a convention, not evidence.

This list is where those leave something open: where the netlist and a
hardware-verified ROM disagree, where the netlist cannot decide or there is
none, or where no test looks. Each entry says what to measure, what the
evidence says, what the netlist and ceres do, the outcomes that would settle
it, and what it takes: a test ROM on a flash cartridge, a logic analyser on
the cartridge bus or the LCD connector, or a scope. Each is a fact of
[facts.toml](facts.toml), which says in a line what it rests on.

[dmg-schematics]: https://github.com/msinger/dmg-schematics
[dmg-sim]: https://github.com/msinger/dmg-sim

## Which chips the evidence comes from

The DMG had four CPU revisions: DMG-CPU, A, B and C, some of B and C as a
"blob" (the die under epoxy, no package). The [Game Boy hardware database]
shows which boards carried which: of the consoles submitted there, DMG-CPU-01
boards carry the first two, DMG-CPU-03 to -06 mostly B, and of the DMG-CPU-08
boards whose CPU is listed, 7 carry a C (blob) and 1 a B (blob).

- The netlist is a DMG-CPU B.
- gambatte's `dmg08` results and gbmicrotest were checked on a DMG-CPU-08
  board, CPU revision not recorded: probably a C.
- Mealybug Tearoom's screenshots are from a `DMG-blob`: a B or a C; two of
  its ROMs also have a screenshot from a `DMG-CPU B`.

So where the netlist and those tests disagree, a difference between the B and
the C is one explanation that has to be ruled out before calling the netlist
wrong. Each DMG entry below should be run on a B and on a C, and the revision
printed on the CPU noted.

On the CGB, gambatte's tests were made on a CGB-C (CPU CGB C), Mooneye's `-C`
and AGE's `cgbE` ROMs on a CGB-D or E, and SameBoy's default is a CGB-E. Each
set fixes its own revision only at the points it reads, so each CGB entry
should be run on a C and an E.

[Game Boy hardware database]: https://gbhwdb.gekkio.fi/consoles/dmg/

## How to write a test

Most entries need the same kind of ROM, in the style of gambatte's hwtests:

1. Turn the LCD off, set up STAT/LYC, turn it on (on a CGB, optionally in
   double speed, `KEY1` then `STOP`).
2. Wait a fixed number of cycles from a known event (the LCD turning on, or
   an interrupt), so the read lands on a chosen dot.
3. Read STAT, LY or IF (or let the interrupt fire and record when).
4. Print the value on screen or store it where a flash cart can be dumped.

Then sweep the wait one cycle at a time across the edge in question: the
cycle at which the value changes is the answer. gambatte's ROMs
(`external/test-roms/gambatte`) are good templates: each comes in `_1`/`_2`
pairs one cycle apart. The drafts in [test_roms](../test_roms/README.md)
are a start for some of the DMG's entries.

Times in the CGB's entries are in half dots (`h`) on the line clock of
`ceres-core/src/ppu/display/timing.rs`: 912 per line, LY changes at
`h = 904` for the comparator. A CPU cycle is 2 half dots at normal speed
and 1 in double speed, and an M-cycle 8 or 4.

## Open questions on both

### What the OAM scan reads while an OAM DMA is copying

Two hardware-verified ROMs seemed to want opposite things; the DMG-CPU B's
netlist explains both, and ceres does what it does on every model. Only the
CGB is left to check, since there is no netlist for it.

- **Measure:** on a CGB, start an OAM DMA at a chosen dot of the line before a
  visible line, with sprite 39 (the last one the scan reads) on that line, and
  see whether the line gets a sprite, on screen or from the length of mode 3.
  Sweep the dot where the DMA starts, the data it copies, the Y and X of
  sprite 39 before the DMA, and whether the line before drew a sprite (and
  with which tile and attributes).
- **The two ROMs:** both have the DMA running during the whole OAM scan.
  - `strikethrough.gb` starts the DMA at the end of line 67, so that it has
    written 2 bytes when line 68's scan begins. The reference screenshots
    (`strikethrough-dmg.png`, `strikethrough-cgb.png`) show a sprite on line
    68 at the Y and X of sprite 39, the last entry read on line 67 (Y `0x54`,
    X `0x4f`): every entry of line 68 looks like sprite 39.
  - `oamdma/late_sp39y_2` (`_ds_2` in double speed) has the DMA 26 bytes in
    when the scan of line 3 begins, and still running at sprite 39's read on
    line 4. It expects no sprite (`out0`, against `out3` for `_1`, where the
    DMA ends one cycle earlier), although sprite 39 is at Y `0x10`, X `0xA0`
    before the DMA, on lines 0 to 7.
- **The netlist:** while a DMA runs, the scan does not read OAM but goes on,
  and compares each entry with what OAM's two latches took last
  (`hdl/rtl/oam.vhd`, `oam_scan.vhd`). That is the Y and X of the last entry
  the scan read, unless OAM was read since: by a sprite fetch of mode 3,
  which reads the sprite's tile and attributes (at the DMA's address while
  one runs), or by the CPU (`FExx`).
  - In `strikethrough`, the latches still hold sprite 39's Y and X from the
    line before, so every entry of the next line matches.
  - In `late_sp39y_2`, the line before the DMA drew sprite 39 itself, and its
    fetch left its tile and attributes in the latches (`A0`, `A0`). A Y of
    `A0` is on no visible line, so the lines during the DMA get no sprite
    (traced in the model).

  The readable model draws `strikethrough`'s screenshot pixel for pixel, and
  passes all 16 of gambatte's DMG `late_sp*` ROMs.
- **Ceres today:** the same latches (`y_bus` and `x_bus` in
  `ceres-core/src/ppu/display/objects.rs`), on every model. It passes
  `strikethrough` on the DMG, CGB-C and CGB-E, and all of gambatte's `oamdma`
  ROMs on both models. Earlier rules each failed one of the two: reading
  `0xFF` during the DMA failed `strikethrough`, and keeping the scan's last Y
  and X but not the fetch's tile and attributes (SameBoy) failed
  `late_sp39y_2`.
- **Outcomes:** settled for the DMG. If the CGB's sweep always shows the
  object that was read last (by the scan or by the fetch), the rule holds
  there too. If not, the CGB differs, and only the DMG keeps the netlist's
  rule. SameBoy notes that pre-CGB units differ in what the scan reads when a
  DMA is halted, which this does not cover.
- **Needs:** a test ROM; a CGB-C and a CGB-E.

### STAT during an OAM DMA

- **Measure:** start an OAM DMA a few lines before a visible line, and read
  STAT from high RAM every M-cycle across that line's OAM scan (on a CGB, at
  both speeds); sweep the DMA's start so that it begins and ends at each
  point of the scan.
- **The evidence:** no test ROM reads STAT while a DMA runs.
- **The netlist:** STAT's mode 2 is the scan's flag gated by the DMA's (ACYL,
  from BESU and BOGE): while a DMA runs the scan goes on, but STAT shows mode
  0, as the scan hides from the OAM bus. The STAT interrupt's OAM source is
  the line clock, which the DMA does not gate. Found by the cut traces of
  `hdl/compare-ceres.sh` (`CUTS=1`) on gbmicrotest's `dma_basic`.
- **Ceres today:** follows the netlist, from the DMA's first byte to its end,
  as the object search sees the DMA (`dma_hides_object_search` in
  `ceres-core/src/ppu/display/objects.rs`; on the CGB the search runs ahead
  of the DMA's state). On the CGB too, since no test contradicts it.
- **Outcomes:** mode 0 during the scan confirms the netlist; the M-cycles
  where it begins and ends against the DMA's start, whether ceres's edges
  are right. On a CGB, mode 2 means the CGB differs, and only the DMG keeps
  the rule.
- **Needs:** a test ROM; a B, a C, a CGB-C and a CGB-E.

### An LCD interrupt requested as the CPU acknowledges another

- **Measure:** with two STAT sources or the LCD's and another interrupt
  enabled, let an interrupt be dispatched while the LCD raises its request
  in the last T-cycles of the dispatch (the line 153 LY = LYC compare, a
  mode 2 or mode 0 start), sweeping the request a cycle at a time; read IF
  in the handler.
- **The evidence:** no test that ceres runs tells the two apart: all of
  Gambatte's, Mooneye's, gbmicrotest's, AGE's and SameSuite's tests pass with
  and without Gambatte's run-ahead (below).
- **The netlist:** keeps the request: IF's flag is set after the
  acknowledge has cleared the other one (the proof in
  `ceres-core/src/interrupts/netlist_proofs.rs`, under its mapping of T-cycles
  to phases).
- **Ceres today:** keeps the request, as the netlist does, on every model
  (`ack_interrupt` in `ceres-core/src/cpu_bus/mod.rs`). Until 2026-10-26 it
  ran the LCD two cycles ahead of the acknowledge at normal speed, as
  Gambatte does, so that what the LCD requested then counted as already
  requested and the acknowledge cleared it.
- **Outcomes:** the flag set confirms the netlist; not set, the run-ahead
  comes back. On a CGB, which has no netlist, whichever it shows.
- **Needs:** a test ROM; a B, a C and a CGB.

### A CPU write to OAM just after a write to FF46

- **Measure:** write FF46, then write OAM (`FExx`) in the next M-cycle, and
  in the one after, outside modes 2 and 3; read the byte back after the
  transfer, from a source that does not hold that byte.
- **The evidence:** no test that ceres runs tells the two apart: all of
  Gambatte's, Mooneye's, gbmicrotest's, AGE's and SameSuite's tests pass
  with the write let through and with it dropped (2026-10-27).
- **The netlist:** OAM is free for the CPU until `run` rises, at the A edge
  that ends the M-cycle after the write (`hdl/rtl/oam.vhd`, `dma.vhd`): a
  write in that M-cycle lands, and the transfer then overwrites the byte.
- **Ceres today:** lets the write through, as the netlist does
  (`blocks_oam` in `ceres-core/src/memory/dma/mod.rs`, proved in
  `memory/dma/netlist_proofs.rs`). Until 2026-10-27 it dropped the CPU's
  OAM writes from the write to FF46 on, as SameBoy does.
- **Outcomes:** whether the byte written in that M-cycle shows after a
  transfer that does not reach it (a restart cut short), or in a sprite
  fetched meanwhile.
- **Needs:** a test ROM; a B, a C and a CGB.

### An OAM DMA restarted while it runs

- **Measure:** start a transfer, write FF46 again with another source while
  it copies byte c, and sweep c and the moment; dump OAM (or draw sprites
  from it) in the two M-cycles that follow, before the new transfer
  reaches bytes 0 and c. Then write FF46 twice in a row, and when the
  first byte lands.
- **The evidence:** no test that ceres runs tells the two rules apart: all
  the suites pass with either (2026-10-27).
- **The netlist:** FF46 is a latch, open while the CPU writes it, so the
  byte c that the transfer writes in that M-cycle comes from the new source;
  in the next, the restart clears the count as the write of OAM begins, so
  it writes byte 0, from the new source; then the new transfer runs from
  byte 0. A write in the M-cycle after another one finds the restart pulse
  (`start_2`) still high, which clears its request: it only changes the
  source. The replay of the model's traces (`netlist_replay`) shows the
  restart.
- **Ceres today:** follows the netlist (`Dma::write` in
  `ceres-core/src/memory/dma/mod.rs`, proved in
  `memory/dma/netlist_proofs.rs`). Until 2026-10-27 it had Gambatte's rule
  (`oamDmaStartPos_`): byte c from the old source, then byte c + 1 from the
  new one, and a second write started the transfer over.
- **Outcomes:** which bytes hold what in those two M-cycles, and whether a
  second write delays the first byte.
- **Needs:** a test ROM; a B, a C and a CGB.

## Open questions on the DMG

### A write to BGP during mode 3

- **Measure:** on the LCD connector (CP, LD0, LD1), the pixels around a write
  to BGP, sweeping the write a cycle at a time; on screen, the same at each SCX
  from 0 to 7, so that the write lands at each dot of a pixel.
- **The evidence:** the real DMG draws a black pixel at each write to BGP:
  Mealybug's `m3_bgp_change` screenshot has 15 pixels of the darkest shade in a
  line where the netlist has 12. AGE's `m3-bg-bgp` (DMG) puts the palette's
  changes where a fixed delay after the write does not explain them all.
- **The netlist:** BGP shows `FF` for 39 ns while the write's data is on its
  way (the bus is still precharged), but the LCD takes no pixel in that moment;
  `m3_bgp_change` and `m3_bgp_change_sprites` match dmg-sim to the end and not
  the screenshot (496 and 328 pixels), `m3-bg-bgp` is 40 pixels off. The end
  of a line in `m3_lcdc_obj_en_change_variant`, where BGP is written, is the
  same kind of difference (144 pixels).
- **Ceres today:** draws the pixels at the write with BGP as `FF`, and passes
  the Mealybug screenshots; `m3-bg-bgp` is 2 pixels off (ignored).
- **Outcomes:** the width of the write's `FF` moment, and where in the dot the
  LCD samples, against the write's M-cycle. It is the delay of the write path
  (how long the bus stays precharged, how fast the palette latch opens), which
  the netlist cannot give.
- **Needs:** a logic analyser on the LCD connector (the screen alone shows
  only pixels, not the dot within them).

### Where the window starts when WX is written every line

- **Measure:** on the LCD connector, the first window pixel in
  `m3_window_timing` and `m3_window_timing_wx_0`, and the same with the WX
  write moved a cycle at a time.
- **The evidence:** the screenshots of both ROMs.
- **The netlist:** starts the window a pixel later than the screenshots, with
  the stall in the same place (138 and 144 pixels): the first window pixel
  replaces the pixel after the old one. It matches dmg-sim to the end.
- **Ceres today:** matches the screenshots.
- **Outcomes:** as for the BGP write above, the delay of a register write
  against the pixel clock; likely the same cause.
- **Needs:** a logic analyser on the LCD connector.

### Enabling the timer with TAC

- **Measure:** write TAC to enable the timer, with the old and the new selected
  bit of the divider at each of 1/0, 0/1, 1/1 and 0/0, and read TIMA after.
- **The evidence:** gbmicrotest's `halt_bug` (and `004-tima_boot_phase`) on the
  DMG-CPU-08 board: enabling does not count.
- **The netlist:** counts. The enable reaches the timer's clock gate before the
  select does, so with the old selected bit at 1 and the new one at 0, the
  clock pulses for no time, and dmg-sim counts on it: TIMA is one ahead (18
  where 14 is expected).
- **Ceres today:** does not count.
- **Outcomes:** whether it counts, on a B and on a C. A zero-width pulse is the
  kind of race whose outcome depends on the chip: both answers are plausible
  for a B.
- **Needs:** a test ROM on a flash cartridge.

### A tick of the timer while TIMA reloads

- **Measure:** let TIMA overflow, and in the M-cycle of the reload that
  follows make the timer tick: by the divider's own edge, by a write to DIV,
  or by a write to TAC that drops the selected bit; then read TIMA. Repeat
  with TMA at `FF`, the tick at the end of the reload, to see whether the
  `FF` to `00` overflows again.
- **The evidence:** no test found that tells the two apart: ceres and the
  readable model pass the same timer tests.
- **The netlist:** TIMA loads TMA through the whole M-cycle after the
  overflow, so the tick is lost; and the overflow flip-flop's input stays at
  0 through the reload, so `FF` to `00` at its end overflows unseen. These
  are the two known differences of the timer's proof
  (`known_difference` in `ceres-core/src/timing/netlist_proofs.rs`).
- **Ceres today:** adds the tick to TMA, and reloads and interrupts on the
  second overflow.
- **Outcomes:** for each, whether the tick counts and whether the overflow
  interrupts.
- **Needs:** a test ROM; a B and a C.

### Writes and reads of the CPU during an OAM DMA

- **Measure:** gambatte's `oamdma_*_busypush*` and `busyread*` ROMs, and the
  cartridge bus with a logic analyser while they run.
- **The evidence:** gambatte's ROMs, verified on hardware; ceres passes them.
- **The netlist:** the model fails 17 of the 207 gambatte ROMs it ran, all of
  them these: a push while the DMA runs leaves other values (`7576AA34` where
  `4576AA34` is expected, `FE` where `22` or `AA`, `FF` where `00`), and a read
  of `FEFF` gives `FF` where the hardware gives `00`. The model matches
  dmg-sim to the end of the one that was recorded. Likely, not confirmed:
  where the CPU and the DMA drive a bus at once, the result is electrical
  (which driver is stronger, a wired AND), and the netlist, and the board
  around it, cannot resolve it.
- **Ceres today:** passes them.
- **Outcomes:** what a bus with two drivers reads, for each bus (cartridge, VRAM,
  OAM) and each pair of values, so that the rule is known beyond the values
  these ROMs use.
- **Needs:** a test ROM; a logic analyser on the cartridge bus to see who
  drives it.

### `halt_op_dupe_delay` and `stat_write_glitch_l154_d`

- **Measure:** the two gbmicrotest ROMs on a B and on a C.
- **The evidence:** they pass on the DMG-CPU-08 board they were checked on.
- **The netlist:** fails both, with the same values as ceres and SameBoy, and
  the same accesses as ceres, M-cycle by M-cycle (`hdl/compare-ceres.sh`).
- **Ceres today:** fails both (`gbmicrotest_known_failures.txt`).
- **Outcomes:** if a B passes them, the netlist has an error there, and the
  readable model shows where to look. If only a C does, the revisions differ,
  and the tests belong to the C.
- **Needs:** a test ROM; units of both revisions.

### The background's enable in `m3_lcdc_bg_en_change`

- **Measure:** the ROM on a B and on a C.
- **The evidence:** two screenshots that differ in 228 pixels: Mealybug's
  `DMG-blob` one, and its `DMG-CPU B` one, with a photo of the unit
  (`m3_lcdc_bg_en_change_dmg_b.png` in the test ROMs). The two units disagree,
  so the result depends on the revision or the unit.
- **The netlist:** fails it in the same four pixels as ceres against the
  `DMG-blob` screenshot (column 0 of lines 19 to 22, black where the
  screenshot is white), and in 232 against the `DMG-CPU B` one, although the
  netlist is a B's.
- **Ceres today:** fails it (ignored, "not investigated yet").
- **Outcomes:** as for the two gbmicrotest ROMs above: a netlist error or a
  revision difference. That a real B is further from the netlist than the
  blob points at a race whose outcome varies between units. Mealybug's
  `m3_lcdc_win_en_change_multiple_wx` also has both screenshots, 3 pixels
  apart, and the netlist draws the blob's.
- **Needs:** a test ROM; units of both revisions.

### The state that the boot ROM does not set

- **Measure:** at the cartridge's entry point, dump OAM, high RAM, work RAM and
  every I/O register the boot ROM does not write, on several units, from several
  power-ons each.
- **The evidence:** gbmicrotest's `poweron_*` ROMs: OAM is not all `FF`, and
  `FF46`, `OBP0` and `OBP1` read `FF`.
- **The netlist:** dmg-sim starts every cell at 0, which makes OAM's bytes `FF`
  and those registers `00`; the real cells settle to whatever they settle to.
  (VRAM's power-on contents do not matter: the boot ROM clears it.)
- **Ceres today:** passes those ROMs.
- **Outcomes:** which of these are the same on every power-on (and so part of
  the machine's behaviour) and which are noise (and so any value is right).
- **Needs:** a dump ROM on a flash cartridge.

### Pictures that no screenshot shows

- **Measure:** photograph, or capture on the LCD connector, Mealybug's eight DMG
  tests with no screenshot upstream: `m3_lcdc_bg_en_change2`,
  `m3_lcdc_bg_map_change2`, `m3_lcdc_tile_sel_change2`,
  `m3_lcdc_tile_sel_win_change2`, `m3_lcdc_win_map_change2`,
  `m3_scx_high_5_bits_change2`, `m3_scy_change2` and `win_without_bg`.
- **The netlist:** draws ceres's picture in all of them. In
  `m3_scx_high_5_bits_change2` it leaves a hole of four pixels where SCX
  changes (30 pixels in seven lines, matching dmg-sim to the end), which
  ceres now draws too: the map's column takes all of SCX, low bits
  included, as the write lands.
- **Ceres today:** its tests of these are ignored.
- **Outcomes:** a reference picture for each.
- **Needs:** a test ROM; a camera or a logic analyser on the LCD connector.

### The OAM corruption bug and the wave RAM's

- **Measure:** Blargg's `oam_bug` ROMs (`7-timing_effect` in particular) and
  `dmg_sound` test 10, on a B and on a C.
- **The netlist:** cannot tell. The corrupted values come from what the RAMs'
  cells do when two accesses meet, which is electrical; the netlist does not
  describe it (dmg-sim models both RAMs as plain SRAM). The cut traces of
  gbmicrotest's `oam_sprite_trashing`, which writes OAM while the PPU reads
  it, differ for that reason.
- **Ceres today:** passes `dmg_sound` test 10; fails `oam_bug`'s
  `7-timing_effect` and the combined `oam_bug` as SameBoy does (ignored).
- **Outcomes:** the corrupted values as a function of the access's timing, so
  that a rule can be fitted where SameBoy's fails.
- **Needs:** a test ROM; for the mechanism, the die photos of the two RAMs.

### The cartridge bus within an M-cycle

- **Measure:** with a logic analyser on the cartridge connector (A0 to A15, D0
  to D7, /RD, /WR, /CS, PHI), the edges of every kind of access, and whether a
  VRAM access shows its address on the pins.
- **The netlist:** gives every edge, from its fitted delays. Some outcomes
  hang on very little: a VRAM access whose address comes from HL or SP shows
  on the cartridge's address pins, for a moment, and one from BC or DE does
  not, because the address latch is open for 2.9 to 3.6 ns in the first case
  and 2.3 to 2.5 ns in the second.
- **Ceres today:** models accesses per M-cycle, not their edges; nothing in
  the CPU's view depends on them, but a cartridge's hardware (an MBC, a flash
  cartridge) sees them.
- **Outcomes:** the delays of the pins, against the netlist's; how much of
  dmg-sim's timing holds for another chip.
- **Needs:** a logic analyser, 16 or more channels at 100 to 200 MS/s.

### The sound output

- **Measure:** the left and right outputs (headphone jack and speaker), with a
  scope or a good audio interface, for square waves of each duty, volume and
  frequency, and for a DAC turning on and off.
- **The netlist:** ends at the chip's analogue outputs. The amplifier (Sharp
  IR3R40) has no public datasheet; the board's filter is in Gekkio's
  [gb-schematics].
- **Ceres today:** a high-pass filter after the mixer.
- **Outcomes:** the filter's response and the amplifier's distortion, so that
  ceres's output can be compared with a recording.
- **Needs:** a scope or an audio interface.

[gb-schematics]: https://github.com/Gekkio/gb-schematics

### The LCD turned off in VBlank, with STAT's mode 0 source on

- **Measure:** with STAT's mode 0 interrupt enabled (STAT bit 3) and the
  others off, turn the LCD off in VBlank, at several lines and dots, and
  read IF; with IE 0, so that nothing is dispatched.
- **The evidence:** no test ROM reads IF there. Mooneye's `stat_write_if-GS`
  and gambatte's `frame0/1/2_m0irq_count_scx*` turn the LCD off in VBlank
  with that source on, but do not look at the flag then.
- **The netlist:** the LCD's reset clears mode 1 (POPU) at once, while the
  pixel counter, at 167 since the last line, is reset through the line's
  start a gate later, so `mode0 = stop_mode3 and not mode1` would rise for
  that moment: a static hazard, whose width is a matter of the die's delays.
  In dmg-sim mode 1 falls 2 ns before the compare, and the gate's own delay
  swallows the pulse: mode 0 does not rise and the flag is not set. The
  readable model, where the pulse was a delta cycle wide and set the flag,
  now swallows it too (mode 0 rises 14 ns after its compare).
- **Ceres today:** turning the LCD off clears VBlank and HBlank together and
  requests nothing, as dmg-sim does.
- **Outcomes:** the flag set shows the pulse is wide enough on the chip;
  not set, that ceres is right. Run at several points, since a pulse this
  short can depend on the line and the unit.
- **Needs:** a test ROM; a B and a C.

## Open questions on the CGB

The CGB's LCD timing in ceres is fitted to the CGB's test suites. These are
the choices those tests leave open.

### LY read while LY increments

- **Measure:** read LY on every cycle across the end of a line, for an
  increment that carries through several bits (7→8, 0x0F→0x10, 0x53→0x54,
  0x8F→0x90), in normal and double speed. Repeat on a DMG if possible.
- **Why the tests don't decide:** gambatte's CGB tests expect `old & new`
  (7→8 reads 0, 0x53→0x54 reads 0x50) at one read position, which is what
  a ripple counter shows while the carry propagates; three ROMs depend on it
  (see the fact). They only sample a few increments, so they don't show
  whether every increment does it, or for how long. The DMG tests never read
  at that position.
- **Ceres today:** a CGB read that lands in the half dot where LY changes
  returns `old & new`; the DMG returns the new value.
- **Outcomes:** `old & new` for one cycle (a ripple counter); or another
  intermediate value, such as the low bits cleared before the high ones;
  or none (gambatte's value comes from something else).

### Where a read sees LY, the LY=LYC flag and the mode bits

- **Measure:** read LY and STAT on every cycle across the end of a line
  (with LYC matching the old LY) and across the end of line 143 and line
  153, at normal speed and in double speed, and after switching speed with
  the LCD on (gambatte's `lcd_offset` ROMs show how to shift the CPU's
  cycles against the LCD's).
- **Why the tests don't decide:** each value changes at a different point,
  and the tests fix those points only where they happen to read. To pass
  gambatte's 3023 CGB tests, ceres reads each signal at its own delay:

  | Signal      | DMG | CGB normal speed | CGB double speed                                                               |
  | ----------- | --- | ---------------- | ------------------------------------------------------------------------------ |
  | LY          | 4   | 4                | 2                                                                              |
  | LY=LYC flag | 3   | 1                | 2, and it holds until the next LY=LYC sample instead of dropping as LY changes |
  | Mode bits   | 4   | 4                | 3                                                                              |

  The CGB's mode 2 also runs a half dot earlier than the DMG's (from
  `h = 909` to 157 on the delayed clock, against 910 to 158), and mode 1
  shows from `h = 911` of line 143 instead of from line 144. A half dot is
  below what one normal-speed read can resolve, so these were fitted
  through the `lcd_offset` ROMs, whose CPU cycles sit a dot or two off the
  usual grid.
- **Ceres today:** the table above.
- **Outcomes:** for each signal, the first cycle that sees the new value,
  at both speeds. If LY, the flag and the mode bits all change at the
  points ceres predicts, the per-signal delays are real. If they line up
  with each other, ceres is overfitting a single read delay.

### When a STAT write reaches the interrupt

- **Measure:** at normal speed, enable (or disable) one STAT interrupt
  source with a write a few cycles before its source rises: the VBlank
  start (LY 144), the OAM interrupt of line 0 (end of line 153), the
  HBlank of a visible line. Find the last cycle at which the write still
  counts.
- **Why the tests don't decide:** gambatte's model fits these with
  per-source rules (the VBlank event sees STAT at once, the others 2
  cycles late). Ceres fits them with one delay plus an order: the written
  enables reach the line 4 half dots after the write, after that half
  dot's LY=LYC sample and together with the VBlank latch. Both models pass
  the tests; they disagree on writes at half dots the tests don't use.
- **Ceres today:** 4 half dots (normal speed), 3 (double speed). The
  HBlank source also clears a half dot later than on the DMG (`h = 907`
  instead of 906), so that an HBlank enable landing at 906 still raises
  the interrupt (gambatte `m0enable/late_enable_lcdoffset1`).
- **Outcomes:** the write-to-effect delay for each source. If they all
  match, the one-delay model holds.

### When an LYC write reaches the comparison

- **Measure:** write LYC (to the current LY, or away from it) a few cycles
  before the end of a line, sweeping the write one cycle at a time, at both
  speeds and with the CPU's cycles shifted against the LCD's
  (`lcd_offset`). Record whether the LYC interrupt fires and when the
  LY=LYC flag changes.
- **Why the tests don't decide:** on the DMG a written LYC reaches the
  comparator 3 half dots after the write and the comparison follows it at
  once. On the CGB no fixed delay passes gambatte's tests; what does is
  that the new LYC only takes effect at the next LY=LYC sample (once per
  M-cycle, at `h % 8 == 4`) at least 6 half dots after the write (4 in
  double speed). That reads like a comparator that latches LYC on its own
  clock, but the tests only probe a few write positions.
- **Ceres today:** the sample-point rule above.
- **Outcomes:** the write-to-effect delay as a function of the write's
  position in the M-cycle. A staircase (the effect jumps by an M-cycle as
  the write crosses a point) confirms the sampled LYC; a straight line
  means a fixed delay.

### A STAT write at the VBlank latch

- **Measure:** at normal speed, write STAT 4 half dots before LY becomes
  144 (and before line 153 ends), toggling the VBlank and OAM enables in
  different combinations.
- **Why the tests don't decide:** in ceres the written enables reach the
  interrupt line in the same half dot as the VBlank latch, and the line is
  evaluated once for both. That is what lets a VBlank disable at that point
  stop the VBlank interrupt, and an OAM enable at the end of line 143 not
  raise one. The DMG's model evaluates them in sequence. Only those two
  gambatte tests (`m1irq_disable`, `ly143_late_m2enable`) depend on it.
- **Ceres today:** evaluated together on the CGB.
- **Outcomes:** whether either combination raises the interrupt.

### What differs between the CGB-C and the CGB-E

- **Measure:** run the ROMs of the entries above on both revisions, and in
  particular: read LY every cycle across the start of line 153 (when does
  it read 0?), read STAT across the end of line 153 (when does mode 1 end?)
  and read the LY=LYC flag across a line end at normal speed.
- **Why the tests don't decide:** gambatte's tests were made on a CGB-C;
  Mooneye's `-C` and AGE's `cgbE` ROMs on a CGB-D/E class unit. Each set
  fixes its own revision only at the points it reads, and some
  differences may be wrong attributions (a test passing on one revision
  for an unrelated reason).
- **Ceres today:** the same structure on both, with three CGB-D/E
  differences found so far: LY becomes 0 on line 153 at `h = 6` instead
  of 4; at normal speed the LY=LYC flag holds until the next LY=LYC sample
  (as in double speed on both); mode 1 shows until `h = 910` of line 153
  (the CGB-C clears it at 908).
- **Outcomes:** the cycle of each change on each revision. A revision
  difference that shows up in only one of these reads would point at a
  different mechanism than a uniform shift.

### What the die photos show of the chip's blocks

Not a test ROM: a question for a decapped chip. The images are listed, with
their sources, in `hdl/README.md` ("Die images").

- **Measure:** decap a CGB of known revision (a CGB-E, the model's default)
  and photograph it at 20x or more, as the DMG-CPU B was: the top metal,
  each lower metal layer, then the polysilicon. Count the cells of each RAM
  and ROM array, and follow the VRAM's data lines.
- **Why the images don't decide:** the metal photos of the CGB B and C
  (1000×1070 and 2047×2160) are too coarse to follow a wire. The only sharp
  image, siliconpr0n's polysilicon scan filed as "cpu-cgb-b" (10x,
  6045×7755), has no metal left, so no wires, and is at half the DMG's
  magnification. It may not be a CGB B either: its README says "Best guess
  at chip. Was mislabeled as DMG-CPU". Its die label is CYUH54, where the
  CGB B and C dies carry the same other code (CUJ831A or close to it). It is
  also taller (width to height 0.78, against 0.93 for the CGB B), has about
  26 pads on its top edge (32 on the CGB B) and one more row of blocks on
  its left. Its plan is a CGB's, so it may be a later revision (D or E), but
  nothing says which chip it is.
- **What the polysilicon scan shows:** the cells repeat every 6.8 pixels down
  and in strips of 46 pixels across, read here as 8 columns of cells sharing
  their contacts with their neighbours (the other reading halves each count
  below).
  - Four identical arrays, each 16 strips of 8 columns by 256 rows: 16 KiB,
    the VRAM inside the chip. Each has 16 units across its middle and
    8 blocks at its foot, as for a byte.
  - No other array could hold 32 KiB: the work RAM is outside the chip.
  - An array of 64 by 16 cells, 128 bytes: the high RAM.
  - A pair of identical arrays, each 64 cells wide by 14 rows (16 if the
    edges' rows count): 112 or 128 bytes each.
  - Other arrays, not measured: in the block to the right of the high RAM,
    and in the two blocks below it.
- **The model today** (`hdl/cgb.md`, on the branch `hdl_cgb`): 16 KiB of
  VRAM inside the chip, with a second data bus that reads bank 1 at bank
  0's address (SameBoy's reading); 32 KiB of work RAM outside; 128 bytes of
  high RAM; two palette RAMs of 64 bytes; the 2304-byte boot ROM; OAM as
  the DMG's.
- **Outcomes:**
  - The VRAM: whether two arrays are read at once (bank 0 and bank 1, as the
    second bus has it) or one at a time.
  - The identical pair: palette RAMs of 64 bytes are half their size. The
    OAM's 160 bytes with the bytes the early CGBs keep behind FEA0 to FEFF
    (24 on the CGB B, as ceres has them) would fit them better. The
    palettes would then be in one of the arrays not yet measured, as would
    the boot ROM.
  - The CYUH54 die: which chip and revision it is. If it is a CGB D or E,
    its plan next to the CGB B's shows where the revisions differ.

## Settled by hardware where the netlist was not enough

- **The tile register at a window start:** the fetch counter's reset gives its
  clock a pulse of no width, which latches the precharged VRAM bus (`FF`).
  dmg-sim's gates are too slow to pass it; the real DMG's screenshots show it
  (`m3_lcdc_win_en_change_multiple_wx`, `m3_scy_change`), and the readable
  model keeps it.

- **The STAT interrupt at the end of VBlank:** with STAT's mode 1 and mode
  2 sources both on, mode 1 falls as the line clock, the mode 2 source,
  rises: a static hazard of the OR of the sources, which the readable model
  showed as a pulse setting IF. Gambatte's `m2irq_ly00_lcdstat30` reads no
  interrupt on a DMG, and the model now computes the two in one expression.

## What to get

`cargo run -p ceres-test-runner --example facts -- open` lists the open
questions with what each needs, and counts how many each piece of hardware
would settle: a flash cartridge almost all of them, a DMG-CPU B and a C
most of the DMG's, a CGB-C and a CGB-E the CGB's; a logic analyser on the
LCD connector or the cartridge bus, a scope and a camera one or two each.

A cartridge built on a microcontroller (one that serves the ROM and logs
the bus) would make most of these automatic: it can run a list of ROMs, read
their results back, and record the bus accesses in the format that
`hdl/compare-ceres.sh` compares.
