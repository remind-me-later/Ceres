# Draft test ROMs

Drafts of test ROMs for the failures that comparing Ceres with the readable
model of the DMG (`hdl/`) and with Mealybug Tearoom's screenshots turned up
(see [hdl/results.md](../hdl/results.md)). They are written in RGBDS (1.0 is
tested), one failure to a ROM, each small enough to read in a sitting, with
the schedule of the writes counted in M-cycles by the macros of
[common.inc](common.inc), which refuse to assemble a write that would land
late.

These are drafts: no ROM here has a screenshot of a real DMG yet. What each
one expects comes from the netlist (the model follows it, and where Mealybug's
screenshots of the real DMG can be compared, it agrees with them), or, for
`sprite_bit_planes`, from the hardware's definition of a tile. Before a ROM
becomes a test of Ceres it needs a capture from a DMG, which replaces
"expected" below.

## Building and running

```bash
make                 # all the ROMs, with rgbasm, rgblink and rgbfix
../hdl/compare-ceres.sh test_roms/win_off_during_tile_fetch.gb 0.12
```

The ROMs clear and load all the VRAM they use, so the fastboot of `hdl/` (the
default of `compare-ceres.sh`) does. The picture of each is steady after the
first frames; `compare-ceres.sh` compares the pictures of ceres and of the
model frame by frame. Mealybug's ROMs, and these, are tests of a picture, not
of a register, so nothing in them reports a result: a test in
`ceres-test-runner/tests` takes the screenshot after a few frames and
compares it (see `mealybug_tests.rs`).

## The ROMs

| ROM | Failure it catches | Today: model vs ceres | Before the fix |
| --- | --- | :---: | :---: |
| [sprite_bit_planes](sprite_bit_planes.asm) | The readable model took the two bit planes of a sprite's pixels the wrong way round (fixed in `43ae47da`) | same | model: 192 pixels |
| [win_off_during_tile_fetch](win_off_during_tile_fetch.asm) | The window turned off while the fetcher is in a tile that began in it: ceres chose the map and the row when the fetch began (fixed in `9429e718`) | 90 pixels | ceres: 900 pixels |
| [win_enabled_after_wx_match](win_enabled_after_wx_match.asm) | The window enabled a moment after WX matched: ceres, like SameBoy, stepped the LCD column back (fixed in `9429e718`) | same | ceres: 18 pixels |
| [bgp_write_across_pixels](bgp_write_across_pixels.asm) | A write to BGP in the middle of a line: the real chip leaves a black pixel at the change, ceres draws it, the model does not | 144 pixels | (not fixed) |

The numbers are pixels that differ in the last picture, of 23040, measured
with `hdl/compare-ceres.sh` and, for the "before", with ceres and the model as
they were before the commit named.

What each ROM expects is in its header: for the first three, the pixels of a
line or the columns where the window starts, by the line's number; for the
BGP ROM, the black pixel before the first dark gray in each of its eight
lines.

### What the numbers say

- `win_off_during_tile_fetch` still differs: in the line whose WX is 4 (mod
  8), ceres reads the window's row where the model, following the netlist,
  reads the background's. It looks like one dot in the order of the write and
  the data read; it is not diagnosed. The model has the support of one
  screenshot (Mealybug's `m3_lcdc_win_en_change_multiple_wx`, which it draws
  pixel for pixel), not of one of this ROM.
- `bgp_write_across_pixels` is the one where the model is the one that is
  wrong. In Mealybug's `m3_bgp_change` the screenshot has the black pixel and
  ceres matches it; the model and dmg-sim show FF on the register for 39 ns
  but take no pixel in that moment. Until the model is made to draw it, a test
  of this ROM expecting the pixel fails on the model, which is what it is for.

## Failures that no ROM here catches yet

- **The first displayed picture differs in ceres** (16 of Mealybug's PPU
  ROMs, for instance a sprite on line 0 missing in `m3_lcdc_tile_sel_change`):
  the cause is not known, and a simple ROM (a sprite on line 0 and a write to
  LCDC in every line) did not reproduce it. The ROM needs the cause first.
- **The tile register at a window start** (the model takes the precharged
  bus when the window starts with the fetch counter at 3; dmg-sim does not;
  the real DMG does): the screenshots of Mealybug's `m3_scy_change` and
  `m3_lcdc_win_en_change_multiple_wx` show it, but a ROM with a window start
  in each of the phases of the fetcher, and one with WX written in every line
  as `m3_window_timing` does, did not show it in a picture of all one colour.
- **LCDC bit 2 written at the end of a sprite fetch** (the glitch of the
  write forced a bit of the address of the VRAM bus for 1 ns in the model):
  it shows on the pins only, never in a picture, so it is covered by the
  checks against dmg-sim (`hdl/check/roms.txt`), not by a ROM that shows a
  picture.
- **A window that starts a pixel late** in `m3_window_timing` and
  `m3_window_timing_wx_0`, and the end of a line of
  `m3_lcdc_obj_en_change_variant`: Mealybug's screenshots and Ceres agree,
  the model and dmg-sim do not. A minimal ROM for them would write WX in every
  line as `m3_window_timing` does, at the dot where the model starts the
  window a pixel late; it is not drafted.
