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

The readable model gets the same results. Where these, and the sweeps below,
part from ceres or from the hardware, the reason is a fact of
[facts.toml](../doc/facts.toml), named here by its id:

- `halt_op_dupe_delay` and `stat_write_glitch_l154_d` fail on ceres, SameBoy
  and the netlist alike, with the same accesses (`halt-op-dupe-delay`,
  `stat-write-glitch-line-154`).
- `halt_bug`: the netlist counts as the timer is enabled, the DMG does not
  (`timer-enable-race`).
- `poweron_oam_000` and `poweron_dma_000`: dmg-sim starts every cell at 0
  (`power-on-state`).

GateBoy fails the line 153 and STAT write glitch tests, which the netlist
and ceres pass.

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

`compare-ceres.sh` also compares the frames (see the
[README](README.md#comparing-ceres-with-it)). The 32 ROMs of Mealybug
Tearoom's `ppu/` directory, run for 0.5 s from quickboot, make the same
accesses on ceres and on the readable model in all of them. Their pictures:

- Identical in all frames (24), and `dmg-acid2` too.
- Different in all of their pictures (5), in this many pixels:
  `m3_bgp_change` 496, `m3_bgp_change_sprites` 328,
  `m3_lcdc_obj_en_change_variant` 144, `m3_window_timing` 138 and
  `m3_window_timing_wx_0` 144. Ceres matches the real DMG's screenshots
  there, the model matches dmg-sim (`bgp-write-in-mode-3`,
  `window-start-wx-written-every-line`).
- Nothing drawn after quickboot (3): `m3_lcdc_obj_en_change`,
  `m3_obp0_change` and `m3_scx_low_3_bits` need the state the real boot ROM
  leaves. With it (`BOOT=real`, 1.5 s) the two sides make the same accesses
  and draw the same picture; only four pictures of the boot logo's scroll
  differ, where the boot ROM writes SCY within a dot of the first tiles'
  fetch.

Where the screenshots (`*_dmg_blob.png`) can decide, both sides draw them
pixel for pixel in `m2_win_en_toggle`, `m3_scy_change`, `m3_scx_high_5_bits`,
`m3_wx_4/5/6_change`, `m3_wx_4_change_sprites`,
`m3_lcdc_obj_size_change(_scx)`, `m3_lcdc_win_en_change_multiple` and
`m3_lcdc_win_en_change_multiple_wx`. The model differs from dmg-sim on
purpose at a window start (`window-start-tile-register-pulse`).

## What the blocks show each other

`compare-ceres.sh` with `CUTS=1` (see the
[README](README.md#comparing-ceres-with-it)) over the 513 gbmicrotest ROMs,
from fastboot to their results (9 minutes, six at a time): 491 make the same
accesses; the 22 others are the read after HALT (7, since fixed in ceres)
and the power-on state (15). In 507, the IF, TIMA, DMA, LY, MODE, OAMB,
VRAMB and DIV changes are the same on both sides, but for changes an M-cycle
off their usual difference and brief ones (`sweep-cuts.sh`, 8 minutes,
eight at a time). Of the others, `500-scx-timing` and `minimal` differ in
their accesses (TIMA counts from the power-on state), and:

- `dma_basic`: STAT during an OAM DMA (`stat-during-oam-dma`).
- `oam_sprite_trashing`: writes OAM while the PPU reads it, which corrupts it
  on the real chip and in ceres, not in the netlist's plain OAM
  (`oam-corruption-bug`).
- The end of VBlank: ceres's STAT shows mode 0 for an M-cycle between mode 1
  and line 0's mode 2, where the model's samples go from 1 to 2. The reads
  agree: `check/stat_vblank_end.asm` reads STAT in every M-cycle across it,
  and both sides read the same 400 values. The model's mode 0 falls between
  its points of sampling.
- The first mode 3 after the LCD is turned on comes an M-cycle sooner on
  ceres, within the M-cycle of sampling; gbmicrotest's `lcdon_to_stat3` ROMs
  pass on both sides.

### Mooneye, Mealybug and Gambatte

The same sweep, from tinyboot, over Mooneye's 54 DMG tests (both suites,
`TEST=mooneye`, `SECS=2`), Mealybug's 32 `ppu/` ROMs (`SECS=0.4`) and every
eighth of Gambatte's DMG tests (202, `SECS=0.29`), with `APU=0`:

| Suite    | ROMs | Same accesses | Same cuts |   Time |
| -------- | ---: | ------------: | --------: | -----: |
| Mooneye  |   54 |            51 |        47 | 68 min |
| Mealybug |   32 |            32 |        32 | 13 min |
| Gambatte |  202 |           185 |       198 | 54 min |

With the fixes the sweep led to, Mooneye's cuts are the same in 50.
Gambatte's 17 different accesses are its `oamdma` ROMs that push or read
while a DMA has the bus (`bus-with-two-drivers`); Mooneye's three read what
power-on leaves (`boot_hwio-G` reads FF46; `gpu/intr_0_timing` and
`intr_1_timing` print WRAM they never wrote). The cuts led to
`stat-or-at-vblank-end`, `lcd-off-in-vblank-mode-0-source`,
`lcd-off-at-line-start` and `window-wx-166`.

Mooneye's older tests (`mooneye-test-suite-wilbertpol`) end on the opcode
ED, which locks the CPU, not on `LD B,B`: `TEST=mooneye` does not end them
and they run for `SECS`.

### All of Gambatte's DMG tests

The same sweep over all 1747 of Gambatte's DMG ROMs (tinyboot, `SECS=0.29`,
`APU=0`, eight at a time: 9.5 hours): 1573 make the same accesses, 1711 have
the same cuts. Of the others, 129 are `oamdma` ROMs that push, read, call an
interrupt or RST while a DMA has the bus (`bus-with-two-drivers`; the other
230 of them are the same on both sides). Of the 77 left, 58 are the same
after the fixes the sweep led to (the read after a HALT cut short, in ceres;
`mode-0-rises-after-lyc-flag`, in the model) and with the APU for the 29
that read NR52 or wave RAM. Of the 19 that still differ, six are the DMA's
bus in the netlist (`dma-reads-external-bus-held`, `dma-from-fe00-ff00`).

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
DMG-CPU B. The others, run on the model from the real boot ROM (6 s of
emulated time) and compared with the real DMG's screenshot where there is
one:

- Mealybug's `m3_lcdc_bg_en_change`: the model fails it in the same four
  pixels as ceres (`bg-enable-change-column-0`).
- Mealybug's eight DMG tests with no screenshot upstream: the model's picture
  is ceres's in all of them (`pictures-without-screenshots`).
- `age m3-bg-bgp` (DMG): the model is 40 pixels off the screenshot, ceres 2
  (`bgp-write-in-mode-3`).
- `little-things-gb tellinglys` (DMG): the model fails in the same 5064
  pixels as ceres.
- Blargg's `oam_bug` ROMs: not run. They take 50 to 100 s of emulated time
  and test the OAM corruption bug, which dmg-sim's default build does not
  have (`oam-corruption-bug`).
- gbmicrotest's `halt_op_dupe_delay` and `stat_write_glitch_l154_d`: as
  above.
- Gambatte's `gambatte_dmg` (ignored for its length): `hdl/run-gambatte.sh`
  runs its ROMs on the model and reads the digits as ceres does. Every eighth
  of the 1747 ROMs, 207 of them: 190 pass, 17 fail, all `oamdma`'s
  `busypush` and `busyread` (`bus-with-two-drivers`).

## Ceres's transcriptions of the timer and IF

`trace-blocks.sh` over 127 ROMs (77 of gbmicrotest's timer, DIV and
interrupt ROMs, Mooneye's 13 timer and 12 interrupt tests, and Gambatte's
20 `irq_precedence` and 5 `m1` IF-write ROMs; 5.2 GB, 26 minutes, four at a
time), replayed by `netlist_replay` (30 s): `NetTimer` and `NetIf` match
the model in all of the 223 million phases. What the traces exercised:

| Rule                                         |  Times |
| -------------------------------------------- | -----: |
| TIMA overflows                               |  17136 |
| A write of DIV, TAC, TIMA, TMA               | 65681, 166, 131, 73 |
| A write of TIMA, or TMA, as TIMA reloads     |   2, 2 |
| TIMA counts as DIV or TAC is written         |     46 |
| An IF write, read, an interrupt entry        | 118, 37, 79 |
| A source rises as it is acknowledged         |      1 |

No trace has a source rising in the phase of an IF write: the replay does
not check that rule (the proofs do, on any rise).

The first run differed in 12 phases, all in phase E of a STAT write: the
glitch's request pulse is shorter than a phase, so a trace of the values at
the phases' ends missed the rise that set IF's flag. The traces now note
every rising edge. Four faults planted in the transcriptions were each
caught (TIMA counting on the clock's rise; no overflow at the A edge; TMA
taking the bus as the write starts; a rise winning over the acknowledge,
which only Gambatte's `late_m0irq_retrigger_2` catches).
