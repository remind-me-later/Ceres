# DMG behaviour to check on hardware

What ceres does on a DMG rests on three kinds of evidence, and each settles
different things:

- **The netlist** of the DMG-CPU B ([dmg-schematics], traced from die photos),
  simulated by [dmg-sim] and described by the readable model in `hdl/`. It
  settles the chip's logic: what is connected to what. It does not settle
  timing: its delays are a model fitted to measurements of one chip, so where
  the outcome of a race between two signals is a matter of nanoseconds, its
  answer is a guess. dmg-sim also starts every cell at 0, models OAM and wave
  RAM as plain SRAM, and forces the OAM data lines where two drivers meet (see
  [hdl/README.md](../hdl/README.md#limits)).
- **Test ROMs verified on real consoles** (gambatte, gbmicrotest, Mooneye,
  Mealybug Tearoom, AGE, Blargg). They settle what the CPU or the screen sees,
  on the unit the test was made on, at the points the test happens to look.
- **Other emulators** (SameBoy, Gambatte). Where they only agree with each other,
  that is a convention, not evidence.

This list is where those leave something open: where the netlist and a
hardware-verified ROM disagree, where the netlist cannot decide, or where no
test looks. Each entry says what to measure, what the evidence says, what the
netlist and ceres do, the outcomes that would settle it, and what it takes:
a test ROM on a flash cartridge, a logic analyser on the cartridge bus or the
LCD connector, or a scope. The CGB's list is
[cgb-hardware-tests.md](cgb-hardware-tests.md); its entry on the OAM scan
during an OAM DMA concerns the DMG too, and entry 1 here adds the netlist's
answer.

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
- Mealybug Tearoom's screenshots are from a `DMG-blob`: a B or a C.

So where the netlist and those tests disagree, a difference between the B and
the C is one explanation that has to be ruled out before calling the netlist
wrong. Each entry below should be run on a B and on a C, and the revision
printed on the CPU noted.

[Game Boy hardware database]: https://gbhwdb.gekkio.fi/consoles/dmg/

## Open questions

### 1. What the OAM scan reads while an OAM DMA is copying

- **Measure:** as in entry 7 of the CGB list: start an OAM DMA at a chosen dot
  of the line before a visible line, with sprite 39 on that line, and sweep the
  DMA's start, the data it copies, and sprite 39's position.
- **The evidence:** `strikethrough.gb` (a sprite drawn at sprite 39's position
  on the line after the DMA began) and gambatte's `oamdma/late_sp39y_2` (no
  sprite) are both verified on hardware, and no rule ceres has tried passes
  both.
- **The netlist:** passes both, and all 16 of gambatte's DMG `late_sp*` ROMs:
  the readable model draws `strikethrough`'s screenshot pixel for pixel. In
  the model (`hdl/rtl/oam.vhd`, `oam_scan.vhd`) the scan does not read OAM
  while a DMA runs, but it goes on, and compares each entry with what OAM's
  two latches took last. That is the Y and X of the last entry the scan
  read, unless OAM was read since: by a sprite fetch of mode 3, which reads
  the sprite's tile and attributes (at the DMA's address while one runs), or
  by the CPU (`FExx`).
  - In `strikethrough`, the latches still hold sprite 39's Y and X from the
    line before (`54`, `4F`), so every entry of the next line matches.
  - In `late_sp39y_2`, the line before the DMA drew sprite 39 itself (Y `10`),
    and its fetch left its tile and attributes in the latches (`A0`, `A0`).
    A Y of `A0` is on no visible line, so the lines during the DMA get no
    sprite (traced in the model).

  SameBoy keeps the scan's last Y and X, which is the first case only.
- **Ceres today:** keeps the two latches as the netlist does (commit
  `5e9d9783`), on every model, and passes `strikethrough` and all of
  gambatte's `oamdma` ROMs. It used to read `0xFF` during the DMA, which
  failed `strikethrough`.
- **Outcomes:** settled for the DMG. On the CGB, which has no netlist, the
  rule passes the tests; entry 7 of the CGB list says what would confirm it.

### 2. A write to BGP during mode 3

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
  the screenshot (496 and 328 pixels), `m3-bg-bgp` is 40 pixels off.
- **Ceres today:** draws the pixels at the write with BGP as `FF`, and passes
  the Mealybug screenshots; `m3-bg-bgp` is 2 pixels off (ignored).
- **Outcomes:** the width of the write's `FF` moment, and where in the dot the
  LCD samples, against the write's M-cycle. It is the delay of the write path
  (how long the bus stays precharged, how fast the palette latch opens), which
  the netlist cannot give.
- **Needs:** a logic analyser on the LCD connector (the screen alone shows
  only pixels, not the dot within them).

### 3. Where the window starts when WX is written every line

- **Measure:** on the LCD connector, the first window pixel in
  `m3_window_timing` and `m3_window_timing_wx_0`, and the same with the WX
  write moved a cycle at a time.
- **The evidence:** the screenshots of both ROMs.
- **The netlist:** starts the window a pixel later than the screenshots, with
  the stall in the same place (138 and 144 pixels). It matches dmg-sim to the
  end.
- **Ceres today:** matches the screenshots.
- **Outcomes:** as in entry 2, the delay of a register write against the pixel
  clock; likely the same cause, as `m3_lcdc_obj_en_change_variant` (144 pixels,
  a BGP write at the end of a line) suggests.
- **Needs:** a logic analyser on the LCD connector.

### 4. Enabling the timer with TAC

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

### 5. Writes and reads of the CPU during an OAM DMA

- **Measure:** gambatte's `oamdma_*_busypush*` and `busyread*` ROMs, and the
  cartridge bus with a logic analyser while they run.
- **The evidence:** gambatte's ROMs, verified on hardware; ceres passes them.
- **The netlist:** the model fails 17 of the 207 gambatte ROMs it ran, all of
  them these: a push while the DMA runs leaves other values (`7576AA34` where
  `4576AA34` is expected, `FE` where `22` or `AA`, `FF` where `00`), and a read
  of `FEFF` gives `FF` where the hardware gives `00`. The model matches
  dmg-sim to the end of the one that was recorded. Likely, not confirmed:
  where the CPU and the DMA
  drive a bus at once, the result is electrical (which driver is stronger, a
  wired AND), and the netlist, and the board around it, cannot resolve it.
- **Ceres today:** passes them.
- **Outcomes:** what a bus with two drivers reads, for each bus (cartridge, VRAM,
  OAM) and each pair of values, so that the rule is known beyond the values
  these ROMs use.
- **Needs:** a test ROM; a logic analyser on the cartridge bus to see who
  drives it.

### 6. `halt_op_dupe_delay` and `stat_write_glitch_l154_d`

- **Measure:** the two gbmicrotest ROMs on a B and on a C.
- **The evidence:** they pass on the DMG-CPU-08 board they were checked on.
- **The netlist:** fails both, with the same values as ceres and SameBoy, and
  the same accesses as ceres, M-cycle by M-cycle (`hdl/compare-ceres.sh`).
- **Ceres today:** fails both (`gbmicrotest_known_failures.txt`).
- **Outcomes:** if a B passes them, the netlist has an error there, and the
  readable model shows where to look. If only a C does, the revisions differ,
  and the tests belong to the C.
- **Needs:** a test ROM; units of both revisions.

### 7. The background's enable in `m3_lcdc_bg_en_change`

- **Measure:** the ROM on a B and on a C.
- **The evidence:** Mealybug's screenshot (`DMG-blob`).
- **The netlist:** fails it in the same four pixels as ceres: column 0 of lines
  19 to 22, black where the screenshot is white.
- **Ceres today:** fails it (ignored, "not investigated yet").
- **Outcomes:** as in entry 6: a netlist error or a revision difference.
- **Needs:** a test ROM; units of both revisions.

### 8. The state that the boot ROM does not set

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

### 9. Pictures that no screenshot shows

- **Measure:** photograph, or capture on the LCD connector, Mealybug's eight DMG
  tests with no screenshot upstream: `m3_lcdc_bg_en_change2`,
  `m3_lcdc_bg_map_change2`, `m3_lcdc_tile_sel_change2`,
  `m3_lcdc_tile_sel_win_change2`, `m3_lcdc_win_map_change2`,
  `m3_scx_high_5_bits_change2`, `m3_scy_change2` and `win_without_bg`.
- **The netlist:** draws ceres's picture in all of them. In
  `m3_scx_high_5_bits_change2` it leaves a hole of four pixels where SCX
  changes (30 pixels in seven lines, matching dmg-sim to the end), which
  ceres now draws too: the map's column takes all of SCX, low bits
  included, as the write lands ([results](../hdl/results.md)).
- **Ceres today:** its tests of these are ignored.
- **Outcomes:** a reference picture for each.
- **Needs:** a test ROM; a camera or a logic analyser on the LCD connector.

### 10. The OAM corruption bug and the wave RAM's

- **Measure:** Blargg's `oam_bug` ROMs (`7-timing_effect` in particular) and
  `dmg_sound` test 10, on a B and on a C.
- **The netlist:** cannot tell. The corrupted values come from what the RAMs'
  cells do when two accesses meet, which is electrical; the netlist does not
  describe it (dmg-sim models both RAMs as plain SRAM).
- **Ceres today:** passes `dmg_sound` test 10; fails `oam_bug`'s
  `7-timing_effect` and the combined `oam_bug` as SameBoy does (ignored).
- **Outcomes:** the corrupted values as a function of the access's timing, so
  that a rule can be fitted where SameBoy's fails.
- **Needs:** a test ROM; for the mechanism, the die photos of the two RAMs.

### 11. The cartridge bus within an M-cycle

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

### 12. The sound output

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

### 13. STAT during an OAM DMA

- **Measure:** start an OAM DMA a few lines before a visible line, and read
  STAT from high RAM every M-cycle across that line's OAM scan; sweep the
  DMA's start so that it begins and ends at each point of the scan.
- **The evidence:** no test ROM reads STAT while a DMA runs.
- **The netlist:** STAT's mode 2 is the scan's flag gated by the DMA's (ACYL,
  from BESU and BOGE): while a DMA runs the scan goes on, but STAT shows mode
  0, as the scan hides from the OAM bus. The STAT interrupt's OAM source is
  the line clock, which the DMA does not gate. Found by the cut traces of
  `hdl/compare-ceres.sh` (`CUTS=1`) on gbmicrotest's `dma_basic`.
- **Ceres today:** follows the netlist, from the DMA's first byte to its end,
  as the scan stops reading OAM; on the CGB too, which has no netlist, since
  no test contradicts it (entry 8 of the CGB list).
- **Outcomes:** mode 0 during the scan confirms the netlist; the M-cycles
  where it begins and ends against the DMA's start, whether ceres's edges
  are right. On a CGB, which has no netlist, whether it does the same.
- **Needs:** a test ROM; a DMG and a CGB.

### 14. The LCD turned off in VBlank, with STAT's mode 0 source on

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

| Entry                      | Flash cartridge | Logic analyser | Scope | Units          |
| -------------------------- | :-------------: | :------------: | :---: | -------------- |
| 1. OAM scan during DMA     |  only to check  |                |       | B, C           |
| 2. BGP write               |       ✅        |   LCD          |       | B, C           |
| 3. Window start            |       ✅        |   LCD          |       | B, C           |
| 4. Timer enable            |       ✅        |                |       | B, C           |
| 5. Accesses during DMA     |       ✅        |   cartridge    |       | B, C           |
| 6. Two gbmicrotest ROMs    |       ✅        |                |       | B, C           |
| 7. Background enable       |       ✅        |                |       | B, C           |
| 8. Power-on state          |       ✅        |                |       | several of each |
| 9. Pictures with no screenshot |   ✅        |   LCD or camera |      | B or C         |
| 10. OAM and wave RAM bugs  |       ✅        |                |       | B, C           |
| 11. Cartridge bus edges    |                 |   cartridge    |   ✅  | B              |
| 12. Sound output           |                 |                |   ✅  | any            |
| 13. STAT during DMA        |       ✅        |                |       | B, C, a CGB    |
| 14. LCD off in VBlank      |       ✅        |                |       | B, C           |

A cartridge built on a microcontroller (one that serves the ROM and logs
the bus) would make most of these automatic: it can run a list of ROMs, read
their results back, and record the bus accesses in the format that
`hdl/compare-ceres.sh` compares.
