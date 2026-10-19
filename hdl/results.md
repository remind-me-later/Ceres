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
of them. Their pictures:

- Identical in all frames (6): `m2_win_en_toggle`, `m3_scx_high_5_bits`,
  `m3_wx_4_change`, `m3_wx_4_change_sprites`, `m3_wx_5_change` and
  `m3_wx_6_change`.
- Different in the first picture only, the same afterwards (7):
  `m3_lcdc_bg_en_change` (14 pixels), `m3_lcdc_bg_map_change`,
  `m3_lcdc_tile_sel_change`, `m3_lcdc_tile_sel_win_change` and
  `m3_lcdc_win_map_change` (8 each), `m3_scy_change` (139) and
  `m3_lcdc_win_en_change_multiple` (2471).
- Different in the last picture too (16), in this many pixels:
  `m3_bgp_change` 496, `m3_bgp_change_sprites` 120, `m3_lcdc_obj_size_change`
  629, `m3_lcdc_obj_size_change_scx` 535, `m3_lcdc_obj_en_change_variant`
  144, `m3_window_timing` 138, `m3_window_timing_wx_0` 144, `win_without_bg`
  281, `m3_scx_high_5_bits_change2` 162, `m3_lcdc_win_en_change_multiple_wx`
  18, and 140 in each of `m3_lcdc_bg_en_change2`, `m3_lcdc_bg_map_change2`,
  `m3_lcdc_tile_sel_change2`, `m3_lcdc_tile_sel_win_change2`,
  `m3_lcdc_win_map_change2` and `m3_scy_change2`.
- Nothing drawn in 0.5 s, on either side (3): `m3_lcdc_obj_en_change`,
  `m3_obp0_change` and `m3_scx_low_3_bits` (they need more `SECS`).

The 140 pixels are the same in six ROMs: in `m3_scy_change2`, one shade step
(2 on the model, 1 on ceres) in five rows of every eight, from x = 3 to 8.
`dmg-acid2` differs in about 570 pixels of its settled picture (fastboot),
around the eyes, nose and cheeks.

Which of the two is right is not settled by the comparison. Where the
screenshots of the real DMG (`*_dmg_blob.png`) have four gray levels, ceres
matches `m3_bgp_change` and `m3_lcdc_obj_size_change` pixel for pixel, as it
does the shade counts of `dmg-acid2-dmg.png`; the model's pictures have not
been compared with the hardware's.
