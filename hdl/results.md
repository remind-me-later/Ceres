# Test ROM results of the oracle

Results of `run-tests.sh` (the ROMs of [tests.txt](tests.txt)) on dmg-sim
with Icarus Verilog 13, from power-on with dmg-sim's quickboot ROM. The CPU
reaches the cartridge's entry point (`0100`) at 182.06 ms; the times are
from power-on. The column "dmg-sim's table" is the table in dmg-sim's
README, for the ROMs it lists.

## gbmicrotest

A test writes its result to `FF80`, the expected value to `FF81`, and `01`
(pass) or `FF` (fail) to `FF82`, as ceres's runner reads them. The interrupt
tests compare other values themselves, so `FF80` and `FF81` can differ when
they pass. dmg-sim's README lists no gbmicrotest.

| Test                    | Result | `FF80` | `FF81` | Finished |
| ----------------------- | :----: | :----: | :----: | -------: |
| div\_inc\_timing\_a     |   ✅   |   00   |   00   | 182.1 ms |
| div\_inc\_timing\_b     |   ✅   |   01   |   01   | 182.1 ms |
| hblank\_int\_scx0       |   ✅   |   2D   |   2D   | 182.3 ms |
| hblank\_int\_scx3       |   ✅   |   00   |   01   | 182.3 ms |
| line\_153\_ly\_a        |   ✅   |   98   |   98   | 198.7 ms |
| line\_153\_ly\_b        |   ✅   |   99   |   99   | 198.7 ms |
| line\_153\_ly\_c        |   ✅   |   00   |   00   | 198.7 ms |
| line\_153\_lyc\_a       |   ✅   |   81   |   81   | 198.7 ms |
| line\_153\_lyc\_b       |   ✅   |   85   |   85   | 198.7 ms |
| line\_153\_lyc\_int\_a  |   ✅   |   99   |   01   | 198.7 ms |
| line\_153\_lyc\_int\_b  |   ✅   |   FF   |   01   | 198.7 ms |
| stat\_write\_glitch\_l0\_a |   ✅   |   E2   |   E2   | 182.1 ms |
| stat\_write\_glitch\_l0\_b |   ✅   |   E2   |   E2   | 182.2 ms |
| stat\_write\_glitch\_l0\_c |   ✅   |   E0   |   E0   | 182.2 ms |
| stat\_write\_glitch\_l1\_a |   ✅   |   E0   |   E0   | 182.3 ms |
| stat\_write\_glitch\_l1\_b |   ✅   |   E2   |   E2   | 182.3 ms |
| timer\_tima\_reload\_256k\_a |   ✅   |   FF   |   FF   | 182.1 ms |
| timer\_tima\_write\_a    |   ✅   |   7F   |   7F   | 182.1 ms |
| timer\_tma\_write\_a     |   ✅   |   71   |   71   | 182.1 ms |
| timer\_tima\_phase\_a    |   ✅   |   FE   |   FE   | 182.1 ms |
| int\_timer\_halt         |   ✅   |   0F   |   0F   | 182.1 ms |
| dma\_0x1000             |   ✅   |   99   |   99   | 182.3 ms |
| dma\_0x9000             |   ✅   |   99   |   99   | 182.3 ms |
| dma\_timing\_a          |   ✅   |   81   |   81   | 182.1 ms |
| lcdon\_to\_stat2\_a      |   ✅   |   80   |   80   | 182.2 ms |
| lcdon\_to\_ly1\_a        |   ✅   |   00   |   00   | 182.2 ms |
| oam\_write\_l1\_a        |   ✅   |   F0   |   F0   | 182.3 ms |
| poweron\_stat\_026       |   ✅   |   86   |   86   | 182.1 ms |
| halt\_bug               |   ❌   |   18   |   14   | 182.1 ms |
| poweron\_oam\_000        |   ❌   |   FF   |   FF   | 182.1 ms |
| poweron\_dma\_000        |   ❌   |   00   |   FF   | 182.1 ms |
| halt\_op\_dupe\_delay     |   ❌   |   01   |   55   | 182.4 ms |
| stat\_write\_glitch\_l154\_d |   ❌   |   E1   |   E0   | 198.9 ms |

GateBoy fails the line 153 and STAT write glitch tests and ceres passes them.
The netlist behaves as ceres and the ROMs expect.

ceres and SameBoy fail `halt_op_dupe_delay` and `stat_write_glitch_l154_d`
(ceres's `gbmicrotest_known_failures.txt`). The netlist fails them too, with
the same values: the CPU's accesses on ceres and on the readable model are
the same, M-cycle by M-cycle (`compare-ceres.sh`). The ROMs were checked on
a real DMG: either the chip they ran on behaves otherwise than the
netlist's DMG-CPU B here, or the netlist has an error.

ceres passes the other three that fail here, and the netlist cannot tell
how a real chip behaves in them:

- `halt_bug` reads TIMA right after it enables the timer. The netlist's
  enable reaches the timer's clock before its select, and enabling it with
  the old selected bit of the divider at 1 and the new one at 0 gives a
  pulse of no width, on which dmg-sim counts: TIMA is one ahead from there.
  The DMG that the ROM was checked on does not count.
- `poweron_oam_000` reads OAM, and `poweron_dma_000` reads `FF46`, which no
  boot ROM writes. dmg-sim starts every flip-flop and RAM cell at 0, which
  makes OAM's bytes FF and the DMA register 00. A real chip starts with
  whatever its cells settle to: OAM was not all FF and `FF46` was FF on the
  DMG the ROMs were checked on (`OBP0` and `OBP1` too, in
  `poweron_obp0_000` and `poweron_obp1_000`).

The readable model gets the same results. It failed `oam_write_l1_a` and
`poweron_stat_026` at first, with STAT's mode 0 where dmg-sim has 2: a read
at the end of the OAM scan saw the scan's flag clear before the flag of
mode 3 was set. In dmg-sim, OAM sees that moment (a write of the CPU
lands then: `oam_write_l1_c`) and STAT does not; the model now holds mode 2
in STAT until mode 3 has begun. Two ROMs that the
comparison ran showed one more difference, on the cartridge's address pins:
a VRAM access shows there for a moment when its address comes from HL or
SP, which reach the chip 4 ns after the request, but not when it comes
from W (`LD (nn),A`), which arrives with it, nor from BC or DE, a
nanosecond sooner than HL, too soon for the pins' latches to take it. The
model's core now drives the address with the delay of its source, as
measured in dmg-sim.

## Mooneye

A test ends with `LD B,B` and passes with the Fibonacci numbers 3, 5, 8, 13,
21 and 34 in B, C, D, E, H and L.

| Test                       | Result | dmg-sim's table | Finished  |
| -------------------------- | :----: | :-------------: | --------: |
| acceptance/di\_timing-GS   |   ✅   |       ✅        |  436.6 ms |
| acceptance/div\_timing     |   ✅   |       ✅        |  356.4 ms |
| acceptance/ei\_sequence    |   ✅   |       ✅        |  356.3 ms |
| acceptance/intr\_timing    |   ✅   |       ✅        |  356.3 ms |
| acceptance/oam\_dma\_timing |   ✅   |       ✅        |  406.4 ms |
| acceptance/timer/div\_write |   ✅   |       ✅        | 1038.2 ms |
| acceptance/timer/rapid\_toggle |   ✅   |       ✅        |  356.3 ms |
| acceptance/timer/tim01\_div\_trigger |   ✅   |       ✅        |  356.3 ms |
| acceptance/timer/tima\_reload |   ✅   |       ✅        |  356.6 ms |

The whole list took 2 hours 40 minutes with six tests at once.

The readable model, on its own (`MODEL=1 ./run-tests.sh`, see the
[README](README.md#running-it)), gets every result of these tables, with
the same values, finished at the same times. It took 9 minutes.

## Pictures

`compare-ceres.sh` also compares the frames (see the [README](README.md#comparing-ceres-with-it)).
The 32 ROMs of Mealybug Tearoom's `ppu/` directory, run for 0.5 s from
quickboot, make the same accesses on ceres and on the readable model in all
of them. Their pictures, as `compare-frames.sh` finds them:

- Identical in all frames (24).
- Different in the last picture (5), in all of their pictures, in this many
  pixels:
  `m3_bgp_change` 496, `m3_bgp_change_sprites` 328,
  `m3_lcdc_obj_en_change_variant` 144, `m3_window_timing` 138 and
  `m3_window_timing_wx_0` 144.
- Nothing drawn after quickboot, on either side (3): `m3_lcdc_obj_en_change`,
  `m3_obp0_change` and `m3_scx_low_3_bits` need the state that the real boot
  ROM leaves. With it (`BOOT=real`, 1.5 s) the two sides make the same
  accesses through the whole boot, and the ROM's own picture is the same on
  both; only four pictures of the boot logo's scroll differ (60, 106, 106 and
  150 pixels, in the top rows, the same in the three), where the boot ROM
  writes SCY within a dot of the fetch of the first tiles.

`dmg-acid2` is identical in all frames.

16 of the 23 differed in the first picture only, and `dmg-acid2` too, until
both sides' frame writers were mended: the first picture was not the same
frame on the two sides. After the LCD is turned on, S does not pulse until
the second frame; the model's writer, which skipped a frame after the LCD
was turned on and counted the lines from S, skipped the second frame as
well when the LCD had been on before (at power-on its count of lines starts
at 0, without S). Ceres's took a frame when LY reached 144, which is a few
dots before ceres presents it: when a step ended between, it wrote the frame
before again. A frame is now taken from S on the model, and from STAT's mode
1 on ceres.

Where the real DMG's screenshots (`*_dmg_blob.png`) can decide, the
comparison with them says who is right: the settled pictures of ceres and of
the model are the screenshots, pixel for pixel, in `m2_win_en_toggle`,
`m3_scy_change`, `m3_scx_high_5_bits`, `m3_wx_4/5/6_change`,
`m3_wx_4_change_sprites`, `m3_lcdc_obj_size_change(_scx)`,
`m3_lcdc_win_en_change_multiple` and `m3_lcdc_win_en_change_multiple_wx`;
ceres's alone, in `m3_bgp_change`, `m3_bgp_change_sprites`,
`m3_window_timing`, `m3_window_timing_wx_0` and
`m3_lcdc_obj_en_change_variant`.

### What the differences were

The comparison, and the work that followed it, found these, in the order
they were found:

- **Sprite planes, in the model.** 140 pixels in six ROMs and 570 in
  `dmg-acid2` were sprite pixels with one of their two bits where the other
  belongs (a tile with only its low plane set came out as colour 2). The
  model's mix of a sprite's pixel had its A and B planes the wrong way
  round: in the netlist, the palette cells of OBP0 and OBP1 pick `2*A + B`,
  A being the plane latched from the tile's second byte. dmg-sim agrees with
  ceres on a ROM that tells the planes apart (`check/sprites.asm`, now one of
  the checked ROMs: the check fails on the old mix with `ld is 10, dmg-sim
  01` and passes on the new one). It also showed that the first frame after
  the LCD is turned on is displaced by a line on the pins, which is why
  neither side writes it.
- **The window's enable bit, in ceres.** `m3_lcdc_win_en_change_multiple_wx`
  (43 pixels off the screenshot; its DMG test was ignored) had three causes,
  all found by comparing the pixel timelines with the model's: the DMG's
  "in window" falls with LCDC bit 5 at once, so a tile that began in the
  window reads its number from the background's map, and its data with the
  background's row, if bit 5 falls before the read (ceres decided both when
  the fetch began); and a window that starts a pixel after its WX matched
  did not overwrite the pixel before it (ceres stepped the LCD column back,
  SameBoy's "horizontal desync"; no test of ceres's suite reaches that, and
  the screenshot contradicts it). The ceres test passes now.
- **The sprite address, in the model.** At a write to LCDC the register
  shows FF for a moment, until the bus has the data. When that moment began
  a nanosecond before the address of a sprite fetch was released, bit 2
  (8×16 sprites) forced a bit of the address and the bus kept the glitch:
  `ma` differed from dmg-sim's at a phase end in
  `m3_lcdc_obj_en_change_variant`.
  The drivers of the PPU's addresses now filter pulses shorter than 2 ns,
  not 1; all the recordings still match, and this one now matches dmg-sim to
  the end.

### Where the model differs from dmg-sim, on purpose

At a window start the fetch counter is reset, and when it was at 3 its bit 1
falls a moment before MYSO does, so the clock of the tile register pulses
for no time and the register takes the VRAM bus, which is still precharged
(FF). dmg-sim, whose gates are slower than that pulse is wide, does not
show it: the check on `m3_window_timing` and on
`m3_lcdc_win_en_change_multiple_wx` differs there (`bg_tile` FF, `ld` 11
where dmg-sim has 01). The real DMG does: with the pulse, the model's picture
of `m3_lcdc_win_en_change_multiple_wx` is the screenshot's, pixel for pixel,
and so is `m3_scy_change`'s; with a 1 ns delay on the clock (which makes the
check pass) it is not (2109 pixels off in the first, 354 in the second).
The model keeps the pulse: it is the die's, and the hardware shows it;
`render.vhd` says so. Those two recordings are not among the checked ROMs.

### Where the model and dmg-sim agree and the hardware does not

In `m3_bgp_change`, `m3_bgp_change_sprites`, `m3_window_timing_wx_0` and
`m3_lcdc_obj_en_change_variant` the model matches dmg-sim to the end of the
run and ceres's test of the screenshot passes. What the netlist and its
fitted delays give differs from the chip in how a register write lines up with
the pixel clock:

- Each write to BGP leaves a black pixel at the change of palette on the
  real DMG: the screenshot of `m3_bgp_change` has 15 pixels of the darkest
  shade in a line where the model has 12, and ceres, which draws them,
  matches it exactly. In the model and in dmg-sim the register does show FF
  for 39 ns while the write's data is on its way (`ld` follows), but the LCD
  takes no pixel in that moment in these ROMs. The real chip's glitch is
  wider, or the pixel is sampled elsewhere, or both; sampling `ld` at the
  other edge of CP gives the same pictures. The end of a line in
  `m3_lcdc_obj_en_change_variant`, where BGP is written, is the same kind of
  difference: ceres draws black from a pixel earlier than the model does.
- The window of `m3_window_timing` and `m3_window_timing_wx_0`, whose WX the
  ROM writes in every line, starts a pixel later in the model than in the
  screenshot, with the stall in the same place: the first window pixel
  replaces the old pixel there, not the one after.

These are where the netlist is not enough: the real delays of the write path
(how long the bus is precharged, how long the latch takes) are what they
turn on. They are open.

### SCX's low bits and the tile's column, in ceres

`m3_scx_high_5_bits_change2` differed in 30 pixels, a hole of four pixels on
seven lines in the model where the ROM writes SCX in mode 3. A waveform of
`render.vhd` shows why: the column of the map read is the sum of SCX and
the pixel counter (`(px + SCX) >> 3`, SCX's low bits carrying), worked out
by gates from the latch of SCX until the tile register takes the map's byte
at the end of the fetcher's count 1. The write lands 157 ns after the map
read begins and 200 ns before that, so the read takes all of the new value:
on line 48, px 19 plus SCX 3F gives column 11 (tile 4C, whose row is half
black, half white), where SCX 38 would give 10.

Ceres's DMG wrote SCX's high bits a dot before its low bits, for the pixel
mixer, whose fine scroll takes the low bits a dot later (Gambatte's
`scx_during_m3/scx_m3_extend_1` and
`enable_display/ly0_late_scx7_m3stat_scx3_2` fail without that, and pass on
the model); the tile fetcher saw the late low bits too, and read column 10.
The fetcher now sees all of SCX as the write lands, and only the mixer (the
fine scroll, and the window at WX 0, which waits for it) keeps the old low
bits for that dot (`cpu_bus/write.rs`, `mixer_scx`). The picture is then the
model's in all frames, and the whole suite passes, AGE's `m3-bg-scx` and
the two Gambatte ROMs among them. (Reading the map at the fetcher's second
dot instead, with SCX as it is then, was tried first: it draws this ROM as
the model does on the line where the write lands early in the fetch, but
not where it lands a dot later, and AGE's `m3-bg-scx` and `m3-bg-lcdc`
fail with it. The model draws both of those as the DMG does.)

## What the blocks show each other

`compare-ceres.sh` with `CUTS=1` (see the
[README](README.md#comparing-ceres-with-it)) over the 513 gbmicrotest ROMs,
from fastboot to their results (9 minutes, six at a time). 491 make the same
accesses; the 22 others are the read after HALT (7, since fixed in ceres: see
the full Gambatte sweep below) and the power-on state (15), as above. In 507,
the IF, TIMA, DMA, LY, MODE, OAMB, VRAMB and DIV changes are the same on both
sides, but for changes an M-cycle off their
usual difference (a change between the two sides' points of sampling) and
brief ones (`sweep-cuts.sh`, 8 minutes, eight at a time). Of the others,
two differ in their accesses (`500-scx-timing` and `minimal`: TIMA counts
from the power-on state); what the others show, and what the sweeps showed
before ceres or the model was changed:

- **STAT during an OAM DMA** (`dma_basic`): the DMA runs through line 1's
  OAM scan, and STAT shows mode 0 there on the model, mode 2 on ceres. In the
  netlist, STAT's mode 2 is the scan's flag gated by the DMA's (ACYL, from
  BESU and BOGE): while a DMA runs, the scan goes on, but STAT, like the OAM
  bus, does not see it. No test reads STAT then; a CPU running from high RAM
  can. Ceres now does the same, on the CGB too (entry 13 of
  [the DMG's list](../doc/dmg-hardware-tests.md) is what would confirm it).
- **The length of mode 3 in `oam_sprite_trashing`**: on 48 lines, mode 3
  ends 4 M-cycles apart, both ways. The ROM writes OAM while the PPU reads it,
  which corrupts OAM on the real chip and in ceres, and not in the netlist's
  plain OAM: the lines have other sprites on the two sides (and OAMB and
  VRAMB, which follow mode 3, differ with it).
- **The end of VBlank**: ceres's STAT shows mode 0 for an M-cycle between
  mode 1 and line 0's mode 2, in every frame, where the model's samples go
  from 1 to 2 (in the `stat_write_glitch_l154` ROMs, two M-cycles). The
  reads agree: `check/stat_vblank_end.asm` reads STAT in every M-cycle across
  the end of line 153 (twice, each pass from a HALT that ends at line 152's
  LY=LYC interrupt, the second an M-cycle sooner), and both sides read the
  same 400 values: mode 1, then mode 0 for one M-cycle, then line 0's mode 2
  (20 M-cycles), 3 (43) and 0 (51). The model's mode 0 there falls between
  its points of sampling.
- **The first mode 3 after the LCD is turned on** comes an M-cycle sooner on
  ceres in every ROM that turns it on (one change, within the M-cycle off
  above), and gbmicrotest's `lcdon_to_stat3` ROMs pass on both sides: a
  change between the points of sampling, it seems.

### Mooneye, Mealybug and Gambatte

The same sweep, from tinyboot, over Mooneye's 54 DMG tests (both suites,
`TEST=mooneye`, `SECS=2`), Mealybug's 32 `ppu/` ROMs (`SECS=0.4`) and every
eighth of Gambatte's DMG tests (202, `SECS=0.29`), with `APU=0`:

| Suite    | ROMs | Same accesses | Same cuts |   Time |
| -------- | ---: | ------------: | --------: | -----: |
| Mooneye  |   54 |            51 |        47 | 68 min |
| Mealybug |   32 |            32 |        32 | 13 min |
| Gambatte |  202 |           185 |       198 | 54 min |

With the fix of ceres below (the LCD turned off as a line begins), Mooneye's
cuts are the same in 50.

- **Accesses.** Gambatte's 17 are its `oamdma` ROMs that push or read while
  a DMA has the bus, which the model fails (the busy bus is not modelled).
  Mooneye's three read what power-on leaves: `boot_hwio-G` reads FF46,
  `FF` on ceres and on the DMG (Pan Docs), 00 on the model (an initial
  value of the VHDL); `gpu/intr_0_timing` and `gpu/intr_1_timing` read
  WRAM they never wrote, and print it, so their timing differs after it.
- **The STAT interrupt at the end of VBlank, in the model.** With STAT's
  mode 1 and mode 2 sources both on, mode 1 falls as vclk rises, and the
  OR of the sources dropped for a delta cycle between them and rose again,
  which set IF (Gambatte's `m2irq_ly00_lcdstat30` read E2 where the DMG
  reads E0; Mooneye's `stat_irq_blocking` differed in IF). On the die this
  is a static hazard of the OR; the hardware test says it does not set
  the flag, and the model now computes the two sources in one expression
  (`lcd_timing.vhd`). Both now agree with ceres.
- **The LCD turned off in VBlank, with STAT's mode 0 source on** (Mooneye's
  `stat_write_if-GS` and Gambatte's `frame0/1/2_m0irq_count_scx*`, IF
  only): the model set the STAT flag, ceres did not. The LCD's reset
  clears mode 1 (POPU) a delta cycle before the pixel counter leaves 167,
  so `mode0 <= stop_mode3 and not mode1` (`render.vhd`) pulsed: the same
  kind of hazard. In dmg-sim mode 1 falls 2 ns before the compare, and the
  gate swallows the pulse; the model now does too, since the fix of mode
  0's rise below, and agrees with ceres. No hardware test reads IF there
  (entry 14 of [the DMG's list](../doc/dmg-hardware-tests.md)).
- **The LCD turned off in the M-cycle a line begins, in ceres** (Mooneye's
  `intr_2_mode0_timing_sprites*`, IF only): the model requests line 0x45's
  mode 2 interrupt and then turns the LCD off; ceres turned it off first.
  `check/lcd_off_line_start.asm` turns the LCD off at 16 M-cycles in a row
  around line 69's start, with STAT's mode 2 source on, and reads IF after
  each: in one of them the model reads E2, ceres read E0. There the model's
  line-end pulse (vclk, from which LY counts and the mode 2 source rises)
  comes 156 ns before the LCD's reset, more than a phase, so the netlist
  settles it; ceres's write landed 2 half dots before its pulse. Ceres's
  interrupt sources now see the LCD off 3 half dots after the write: the
  probe makes the same accesses on both sides, and so do the three ROMs'
  cuts.
- **The window at WX 166, in ceres** (Gambatte's `enable_wxA6_2x_spxA7_*`
  and `m2int_wxA6_spxA7_*`, ten ROMs, MODE, OAMB and VRAMB; the 34 other
  WX=A5 and A6 ROMs were the same): with WY 0, WX A6 and a sprite at X A7,
  on the first line 0 after the window is turned on, mode 3 lasted three
  M-cycles longer on the model. `check/window_line0_sprite.asm` reads STAT
  every M-cycle across that line's end (ceres read mode 0 an M-cycle where
  the model read mode 3: the CPU can see it; no hardware test reads there),
  and its colours (background 1, window 3, sprite 2) showed a second
  difference. A waveform of `render.vhd` gives both, as flip-flops half a
  dot or more apart, not races:
  - WX matches the pixel counter at its last dot (px A6): PYCO takes the
    match at the dot's rise, and at its end the window latch sets, as px
    becomes A7 and the sprite matches. The window's start resets the
    fetcher, so the sprite's fetch waits for the window's first tile and
    the next (11 dots), and only then does mode 3 end. Without a sprite
    there, mode 3 ends at A7 and the start shows nothing. The pixel at
    column 159 stays the background's.
  - PYCO is reset only by the LCD's reset, and samples the match (WY's
    latch and WX, not LCDC bit 5) at every dot: at the start of the next
    line it still holds the match of the line before, and the window starts
    as the line does, if bit 5 is on then. In the probe's second pass the
    window was turned off in line 1 of the frame before, after WY had
    matched, and on again in VBlank: the model drew line 0 as window, ceres
    as background (ceres carried the window into the next line only if bit
    5 was on at the end of the line before).

  Ceres's DMG now does both (`mode3.rs`, `window.rs`): an object fetched
  after a window start at WX 166 waits 11 dots more, and the next line
  starts in the window if WY had matched and WX was 166 at the end of the
  line before, and the window is on when the line's mode 3 starts. The
  probe makes the same accesses on both sides, with the same cuts and the
  same pictures, and all 44 of Gambatte's WX=A5 and A6 DMG ROMs have the
  same cuts.

Mooneye's older tests (`mooneye-test-suite-wilbertpol`) end on the opcode
ED, which locks the CPU, not on `LD B,B`: `TEST=mooneye` does not end them
and they run for `SECS`.

### All of Gambatte's DMG tests

The same sweep over all 1747 of Gambatte's DMG ROMs (tinyboot, `SECS=0.29`,
`APU=0`, eight at a time: 9.5 hours): 1573 make the same accesses, 1711 have
the same cuts. Of the others, 129 are `oamdma` ROMs that push, read, call an
interrupt or RST while a DMA has the bus (the busy bus, as above; the other
230 of them are the same on both sides). The 77 left ran again after the
fixes below, the sound ones with the APU: 58 are then the same in their
accesses and their cuts.

- **Without the APU** (29: 26 of `sound/`, and `m1`'s three
  `lyc14*_ly148_push_m1irq_late_enable`, which push onto the stack in wave
  RAM): they read NR52 or wave RAM, which `APU=0` leaves out. With the APU
  all 29 are the same.
- **The read after a HALT cut short, in ceres** (eight of `halt/`, EI then
  HALT with the interrupt pending): the chip's dispatch reads the address
  after the HALT, ceres read the HALT's own (it took PC back in the HALT).
  Ceres now takes PC back in the dispatch, after its read (`sm83/mod.rs`);
  the eight make the same accesses, and the seven gbmicrotest ROMs above
  with them.
- **A write to LYC that ends LY = LYC as mode 3 ends, in the model**
  (`m0enable/lycdisable_ff45_3`, 2 on the DMG, 0 on the model): with STAT's
  LYC and mode 0 sources on, dmg-sim's STAT line drops for 13 ns, as LY =
  LYC's flag (ROPO, at hclk + 1 ns) falls before mode 0 rises (at hclk + 14
  ns, from the pixel counter, whose clock comes 8 ns after hclk), and its
  rise sets IF. The model's counter clocked first and mode 0 rose with hclk.
  Mode 0 now rises 14 ns after its compare (`render.vhd`); the recording is
  in `check/roms.txt`, and all 147 `m0enable` and `lycEnable` ROMs pass on
  the model. The same delay swallows mode 0's pulse at the LCD's turning
  off (above), and the 12 `frame*_m0irq_count` ROMs and four IF-only others
  (`lcdirq_precedence`, `m1`) are now the same too.

The 19 that still differ:

- **The DMA's bus, in the netlist** (6): `oamdma`'s three `srambankchange_1`
  (cartridge RAM read while the DMA has the external bus: the model reads 00,
  ceres the DMA's byte, 4, as the DMG) and `oamdma_srcFE00_readFE00`,
  `oamdma_srcFF00_readFE00` and `oamdma_srcFF00_readFE45` (a DMA from `FE00`
  or `FF00`: the model copies 83 or 64 where the DMG and ceres give `FF` and
  00). Recorded in dmg-sim from fastboot (`srambankchange_1` from `0000`,
  `srcFE00_readFE00`), the model's internal signals are dmg-sim's to well
  past the read, so dmg-sim reads the same: the netlist's, as the busy bus.
  (The cartridge's address pins differ in both recordings where the CPU
  runs from `7FFF` into `8000`: the model's latch keeps the low byte `FF`,
  dmg-sim's takes the precharged 00. Only the cartridge sees it; open.)

Not yet looked into:

- In their accesses (1): `oam_access/postread_scx3_2` (OAM read as mode 3
  ends: 00 on ceres, `FF` on the model; with `xout1` in its name, Gambatte's
  own runner, which looks for `dmg08_out`, does not run it on the DMG).
- In MODE, OAMB and VRAMB (2): `oamdma/late_sp00x_2` and `late_sp39x_2`.
- In MODE only (4): `oamdma_late_halt_stat_1`,
  `oamdmasrc80_halt_lycirq_read8000`, `speedchange/key1_set` and
  `lyc153int_m2irq_ifw_2`.
- In IF only (6): `irq_precedence`'s `late_if_via_sp_if_1` and
  `late_m0irq_vs_tima_scx3_2` (and its `_halt_`), `m1`'s
  `lycint143_m1irq_late_retrigger_2`, `m2m1irq_ifw_2` and `m2m1irq_ifw_3`: a
  flag that rises in the M-cycle the CPU writes IF, which the two sides
  sample in another order.

## The tests that ceres ignores

Of the tests ceres ignores, the ones for CGB and MGB models do not apply to a
DMG-CPU B. For the others (all run from the real boot ROM, 6 s of emulated
time, and compared with the real DMG's screenshot where there is one):

- Mealybug's `m3_lcdc_bg_en_change` ("fails, not investigated yet"): the model
  fails it in the same four pixels as ceres (column 0 of lines 19 to 22, black
  where the screenshot is white).
- Mealybug's eight DMG tests with no screenshot upstream: the model's picture is
  ceres's in all of them (in `m3_scx_high_5_bits_change2` since ceres's fix of
  SCX's low bits, see above).
- `strikethrough` (the OAM DMA quirk, "not emulated"): **the model draws the
  screenshot pixel for pixel**; ceres is 7 pixels off. The netlist has the
  quirk.
- `age m3-bg-bgp` (DMG): the model is 40 pixels off the screenshot, ceres 2; the
  model's are the black pixels at the writes to BGP that it lacks (see above).
- `little-things-gb tellinglys` (DMG): the model fails in the same 5064 pixels as
  ceres.
- Blargg's `oam_bug` ROMs: not run. They take 50 to 100 s of emulated time and
  test the OAM corruption bug, which dmg-sim's default build does not have.
- gbmicrotest's `halt_op_dupe_delay` and `stat_write_glitch_l154_d`: the model
  fails them with the values ceres does (above).
- Gambatte's `gambatte_dmg` (ignored for its length): `hdl/run-gambatte.sh` runs
  its ROMs on the model (from quickboot or the short boot ROM, with the same
  results) and reads the digits as ceres does. Every eighth of the 1747 ROMs,
  207 of them: **190 pass, 17 fail**, all in `oamdma`:
  the `busypush` and `busyread` ROMs, where the CPU pushes to, or reads, the
  area at `FE00`/`FEA0` while an OAM DMA runs (for instance a read of `FEFF` gives
  `FF` on the model, `00` on the hardware). The model matches dmg-sim to the
  end of the one of them that was recorded, so these are the netlist's and the
  board's, not the model's.

Running them found two mistakes in the model's own test bench, now fixed. Its
MBC5 started with ROM bank 0 at `4000` to `7FFF` (dmg-sim's board does; a real
chip starts with bank 1), which broke every Gambatte ROM that calls into the
second bank. And the write strobe of LCDC opened as soon as the CPU's write
did, a few nanoseconds before dmg-sim's, which made the model see a change of
the sprites' size during the OAM scan that dmg-sim does not
(`sprites/late_sizechange2_*`): the strobe opens 3 ns later now, and all the
recordings, the Mealybug pictures and these ROMs agree.
