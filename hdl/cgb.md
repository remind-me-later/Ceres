# The CGB model: what differs from the DMG

No netlist of the CGB's chip (the CPU CGB) is public. The model of it on
this branch (`hdl_cgb`) is the DMG-CPU B's model of branch `hdl`, which the
netlist describes, changed where the CGB is known to differ. So
`git diff hdl hdl_cgb -- hdl/rtl hdl/board` is the list of the differences,
block by block, and this file says where each one comes from and how sure
it is.

Each change carries a tag in the VHDL: `@hw die:` is still the DMG's
netlist, for what the CGB keeps; `@hw inferred:` is the CGB's, with its
source:

- **test**: a test ROM's result on a real CGB pins it (named);
- **ceres**, **SameBoy**, **Gambatte**: an emulator does it, and passes the
  tests near it, but no test pins this detail;
- **Pan Docs**: the documentation says so, with or without a test;
- **guess**: nothing says; the model keeps the DMG's way where it can
  (unknown CGB behaviour follows the DMG's).

The revision is a generic, `REVISION` (`CGB_E`, the default, or `CGB_C`):
where the two differ, the blocks say so.

## The differences

### The chip and the board

| What | DMG (netlist) | CGB (model) | Source |
| ---- | ------------- | ----------- | ------ |
| VRAM | 8 KiB, a chip on the board, on the pins MA0 to MA12, MD0 to MD7 | 16 KiB inside the chip (`video_ram.vhd`); MA13 is the bank | Pan Docs (size); guess (place: the CGB's board has no VRAM chip) |
| Work RAM | 8 KiB on the cartridge bus, selected by /CS and A14 | 32 KiB, the same, with three bank lines (`wram_bank.vhd`) | Pan Docs (size, banks); guess (the lines) |
| Boot ROM | 256 bytes at 0000 to 00FF | 2304 bytes at 0000 to 00FF and 0200 to 08FF | the boot ROM's size; SameBoy, ceres (map) |
| Crystal | 4 MiHz | 8 MiHz; halved, the 4 MiHz clock (`clocks.vhd`) | Pan Docs |

### Double speed

| What | DMG (netlist) | CGB (model) | Source |
| ---- | ------------- | ----------- | ------ |
| The CPU's clock | the 4 MiHz clock: a phase per edge | the same in normal speed; the 8 MiHz clock in double speed (`clocks.vhd`) | Pan Docs |
| What follows the CPU's clock | everything | the CPU, the divider, the timer, the joypad's debounce, the serial port, OAM DMA, the cartridge bus and PHI; the CPU's accesses to the PPU's and the APU's registers | Pan Docs (DIV, serial, OAM DMA, PHI double); guess (the rest) |
| What stays at 4 MiHz | | the PPU (`clk4` into `lcd_control`, `lcd_timing`, `oam_scan`, `oam`, `render`), the APU's dividers and channel 3's wave fetch | Pan Docs ("dots", "APU ticks" do not change) |
| The frame sequencer | DIV bit 4 (divider bit 10) | DIV bit 5 in double speed | Pan Docs |
| KEY1 (FF4D) | | bit 0 armed (R/W), bit 7 the speed; the CGB's mode only (`speed_switch.vhd`) | Pan Docs |
| The switch | | STOP with KEY1 bit 0: the DMG's STOP, which stops the oscillator and clears the divider; the speed changes while it is stopped; the switch wakes the CPU core a moment later, as a joypad line would; the CPU core then waits, as after the DMG's STOP, for the divider's bit 15: 2^15 M-cycles at the new speed (or an interrupt) | guess, which gives ceres's and SameBoy's 0x20008 T-cycles and the reset of DIV |
| The stop's length | | 16 periods of the crystal into double speed, 4 out of it | fitted to ceres, whose times pass Gambatte's `speedchange` tests: with them, `speedchange2_div_1`, `_div_2` and `speedchange2_key1` make the same accesses as on ceres through two switches |

### Registers the DMG does not have

| Register | Behaviour in the model | Source |
| -------- | ---------------------- | ------ |
| KEY0 (FF4C) | Written by the boot ROM only (locked after FF50). Bit 2 or 3 set: the DMG's mode. Does not read back. | SameBoy; Pan Docs |
| OPRI (FF6C) | Bit 0, reads FE or FF. Sets the objects' priority (1: by X) only while the boot ROM is mapped; later writes in the CGB's mode keep the bit but not the priority. | SameBoy; Pan Docs (to be verified there); ceres reads FF after the boot ROM |
| VBK (FF4F) | Bit 0, the CPU's VRAM bank (and OAM DMA's). Written in the CGB's mode; reads FE or FF on any CGB. | Pan Docs; SameBoy; ceres |
| SVBK (FF70) | Bits 0 to 2, the work RAM's bank at D000 (0 is 1). Written in the CGB's mode, or by the boot ROM; reads F8 to FF in the CGB's mode, FF in the DMG's. | Pan Docs; SameBoy |
| FF72, FF73 | Keep any byte, on any CGB. | Pan Docs ("CGB Mode only"); SameBoy (any CGB) |
| FF74 | Keeps any byte in the CGB's mode; FF in the DMG's. | Pan Docs; SameBoy |
| FF75 | Bits 4 to 6 keep a write; the others read 1. | Pan Docs; SameBoy |

The writes to these registers latch as the write strobe falls, as the DMG's
own registers do (a guess for each).

### The PPU's colours

| What | DMG (netlist) | CGB (model) | Source |
| ---- | ------------- | ----------- | ------ |
| VRAM's data bus | 8 bits | a second one shows bank 1 at the same address (`video_ram.vhd`) | SameBoy ("the CGB has a 16-bit data bus for the VRAM") |
| The tile's attributes | | latched with the tile number, in fetch count 1, from bank 1: bank, flips, palette, priority (`render.vhd`) | Pan Docs; SameBoy (same T-cycle as the tile number, on any CGB) |
| Flip in Y, bank | | the tile's row, from the bottom; the tile's bytes from the attributes' bank | Pan Docs |
| Flip in X | | the shifter loads the two bytes mirrored | Pan Docs; guess (where) |
| The background shifter | the pixels | also the tile's palette and priority, loaded with it (the shifter holds one tile) | guess |
| A sprite's attributes | bits 4 to 7 | also bits 0 to 3, the palette and bank, in the CGB's mode only | Pan Docs; ceres (the bank in the CGB's mode only) |
| Priority between sprites | the one fetched first (by X, then OAM order) | the same with OPRI 1; with OPRI 0, a pixel of a sprite earlier in OAM also replaces one of a later sprite | SameBoy (`fifo_overlay_object_row`); Pan Docs |
| LCDC bit 0 | hides the background | the DMG's mode: the same; the CGB's: turns off the priorities over sprites | Pan Docs; SameBoy |
| Priority over sprites | the sprite's bit 7 | the sprite's bit 7 or the tile's | Pan Docs |
| The pixel | BGP, OBP0, OBP1: a shade on LD0, LD1 | the CGB's mode: the palette and colour; the DMG's: the DMG palettes' shade in the background's palette 0 or the sprites' 0 or 1 | Pan Docs ("Compatibility palettes"); SameBoy |
| The palettes | | BCPS, BCPD, OCPS, OCPD, two RAMs of 64 bytes (zeros at power-on); locked in mode 3 (reads FF, writes lost, the index still moves); data registers dead in the DMG's mode after the boot ROM (`colour_palettes.vhd`) | Pan Docs; SameBoy; ceres (zeros); guess (mode 3 exactly) |
| The LCD's pins | LD0, LD1 | 15 bits of colour | Pan Docs (the CGB's LCD) |

With these, cgb-acid2 draws its picture on the model as on the real CGB but
for 158 pixels: the footer's text is one pixel to the right. Ceres draws it
exactly. The bus traces part first at the LY = LYC interrupt that wakes a
HALT, one M-cycle earlier on the model: the CGB's STAT timing, below.

## Not modelled yet

Planned, in this order (each will move up into the tables):

1. The CGB's LCD timing, which ceres fits to Gambatte's, SameSuite's and
   AGE's tests (`doc/cgb-hardware-tests.md`, entries 2 to 5): the LY = LYC
   flag a half dot after LY changes (the DMG: three halves later), a written
   LYC taking effect at the comparator's next sample (once an M-cycle),
   mode 2 half a dot earlier, mode 1 from the end of line 143, the STAT
   write's delay to the interrupt, and in double speed their own delays.
2. HDMA (FF51 to FF55): the general-purpose transfer and the HBlank one.
3. The APU: PCM12 and PCM34 (FF76, FF77), wave RAM while channel 3 plays,
   the lengths while the APU is off, the frame sequencer in double speed.
4. The serial port's fast clock (SC bit 1), the infrared port RP (FF56),
   FEA0 to FEFF, OAM DMA's sources, the OAM bug.
5. The CGB-C's differences (`doc/cgb-hardware-tests.md`, entry 6).

## Running it

`make model` builds it as on branch `hdl`. The boot ROMs are
`external/gb-bootroms/bin/cgbE.bin` (the CGB-E's) and `cgb.bin` (the
earlier revisions'), 2304 bytes each. The CGB-E's takes 3.26 million M-cycles
to reach the cartridge (about ten minutes of the model);
`build/tinyboot_cgb.bin` (`boot/tinyboot_cgb.asm`) leaves the machine as it
does in 35 thousand: for a cartridge in the CGB's mode (cgb-acid2) and one in
the DMG's (Mooneye's `boot_regs-cgb`), the model's accesses after it are the
same as ceres's after the real boot ROM, and so is the divider's phase.

`compare-ceres.sh` compares the model with ceres running a CGB-E (or a CGB-C,
`REVISION=C`), both with that boot ROM (`BOOT=real` for the real one):

```bash
CUTS=1 ./compare-ceres.sh external/test-roms/gambatte/speedchange/speedchange2_div_1_cgb04c_out00.gbc 0.15
```

Both sides number the M-cycles by time from the CPU's first access: whole
M-cycles of the normal speed, and `.5` for an M-cycle that starts in the
second half of one (double speed).

With the real CGB-E boot ROM, the model and ceres make the same accesses
over the first 2.6 million M-cycles of the boot (no HDMA yet: the boot ROM
starts its transfers later).
