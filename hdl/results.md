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

- Identical in all frames (7): `m2_win_en_toggle`, `m3_scx_high_5_bits`,
  `m3_wx_4_change`, `m3_wx_4_change_sprites`, `m3_wx_5_change`,
  `m3_wx_6_change` and `win_without_bg`.
- Different in the first picture only, the same afterwards (16): 3 to 38
  pixels in 13 of them, `m3_scy_change` 139,
  `m3_lcdc_win_en_change_multiple_wx` 151 and
  `m3_lcdc_win_en_change_multiple` 7121.
- Different in the last picture too (6), in this many pixels:
  `m3_bgp_change` 496, `m3_bgp_change_sprites` 328,
  `m3_lcdc_obj_en_change_variant` 144, `m3_scx_high_5_bits_change2` 30,
  `m3_window_timing` 138 and `m3_window_timing_wx_0` 144.
- Nothing drawn after quickboot, on either side (3): `m3_lcdc_obj_en_change`,
  `m3_obp0_change` and `m3_scx_low_3_bits` need the state that the real boot
  ROM leaves. With it (`BOOT=real`, 1.5 s) the two sides make the same
  accesses through the whole boot, and the ROM's own picture is the same on
  both; only four pictures of the boot logo's scroll differ (60, 106, 106 and
  150 pixels, in the top rows, the same in the three), where the boot ROM
  writes SCY within a dot of the fetch of the first tiles.

`dmg-acid2` differs only in its first picture (62 pixels).

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

### Not looked into

- The 16 ROMs that differ in the first picture only: in every one the model's
  first picture is the same as its later ones and ceres's is not (in
  `m3_scy_change`, in its first two lines, where the pixel timelines of the
  two sides are the same; in `m3_lcdc_tile_sel_change`, the 8 pixels are a
  sprite on line 0 that ceres does not draw in that frame, whose OAM nothing
  writes after the LCD is turned on). No screenshot shows those frames.
- `m3_scx_high_5_bits_change2` (30 pixels, seven lines, a hole of four pixels
  in the model where SCX changes): the model matches dmg-sim to the end of
  the run, so it is the netlist's against ceres's; no screenshot shows it.
