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
| The PPU in a switch | stops with the oscillator | its 4 MiHz clock has a gate of its own (`clocks.vhd`, `speed_switch.vhd`): it stops for 10 periods of the crystal (5 dots) from the stop, either way, while the oscillator may already run | ceres and SameBoy (`speed_switch_freeze`: 5 T-cycles of the normal speed); with it, `lycint_ly_ds_1` makes the same accesses as on ceres |

### The CPU

| What | DMG (netlist) | CGB (model) | Source |
| ---- | ------------- | ----------- | ------ |
| Leaving HALT | the requests as their latch holds them from the M-cycle's middle (phase E) | the requests as they are at the M-cycle's start: one in an M-cycle's first half wakes the CPU an M-cycle later (`sm83_sequencer.vhd`) | ceres (SameBoy: the DMG samples 2 dots into a halted step, the CGB at its start); with it, cgb-acid2 and Gambatte's `hdma_start` make ceres's accesses |
| Waiting for HDMA | | the core's clock held in phase A for whole M-cycles (`clocks.vhd`) | guess |

### HDMA

| What | Behaviour in the model (`hdma.vhd`) | Source |
| ---- | ----------------------------------- | ------ |
| HDMA1 to HDMA4 (FF51 to FF54) | the source (its low 4 bits 0; E000 on reads as F000 on) and the destination in VRAM (low 4 bits 0, 13 bits used); they read FF | Pan Docs; ceres |
| HDMA5 (FF55) | write: the blocks of 16 bytes, minus 1, and bit 7, an HBlank transfer; bit 7 clear stops an HBlank transfer (a requested block still runs, then the rest at once); read: bit 7 high when no transfer runs, the blocks left minus 1 | Pan Docs; ceres |
| Where it runs | after an opcode fetch: the CPU core waits, its clock held for whole M-cycles while the divider, the timer and the rest count on (`clocks.vhd`); the M-cycle after the fetch then follows | ceres, SameBoy; guess (how the core is held) |
| How long | 8n + 1 M-cycles for n blocks (16n + 1 in double speed): a byte every half M-cycle (every M-cycle), half an M-cycle to begin and one to end (an M-cycle to begin) | ceres (from SameBoy) |
| The bytes | read from the cartridge's bus as OAM DMA reads it (ROM, its RAM, the work RAM in SVBK's bank); FF from 8000 to 9FFF and from E000 on; written to VRAM in VBK's bank, whatever the PPU does | ceres |
| HBlank's blocks | requested when mode 0 begins with the CPU running, when the transfer starts in mode 0, and when the LCD is turned off | ceres |

HDMA sees HBlank a dot after mode 3 ends, where ceres requests the block.

Not modelled: transfers while the CPU is halted or switches speed (and
the requests at the wake), writes to both banks while the PPU blocks
VRAM, clashes with the PPU's reads or OAM DMA, and OAM written by HDMA.

Of Gambatte's 94 HDMA ROMs that do none of these, 68 pass. Those in double
speed fail on the LCD's double-speed reads (below), not on the transfer:
its length is ceres's. At normal speed six fail: the display turned off
during a transfer (`hdma_disable_display_2`, `hdma_disabled_display_1`),
a late enable (`hdma_late_enable_1`), a late length (`hdma_late_length_1`)
and two with SCX 2 (`hdma_start_scx2_1`, `hdma_late_disable_scx2_1`).

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
| VRAM on the first line after the LCD is turned on | mode 3 takes it from the CPU | at normal speed, 5 dots after mode 3 begins (`vram_bus.vhd`) | ceres (from SameBoy); Gambatte's `enable_display/ly0_late_vramr`, `_vramw` |
| LCDC bit 1 | the sprites are neither fetched nor drawn | they are fetched (mode 3 is as long), not drawn (`cgb_cpu.vhd`, the sprite store's enable) | SameBoy; ceres |
| WX's match | needs WY's latch only: with the window off, a match still holds the fetcher's next tile back a dot | needs the window on too (`render.vhd`) | cgb-acid2's picture of a real CGB (its footer, under WX 5F with the window off); ceres |
| Priority over sprites | the sprite's bit 7 | the sprite's bit 7 or the tile's | Pan Docs |
| The pixel | BGP, OBP0, OBP1: a shade on LD0, LD1 | the CGB's mode: the palette and colour; the DMG's: the DMG palettes' shade in the background's palette 0 or the sprites' 0 or 1 | Pan Docs ("Compatibility palettes"); SameBoy |
| The palettes | | BCPS, BCPD, OCPS, OCPD, two RAMs of 64 bytes (zeros at power-on); locked while the PPU draws, from 3 dots after mode 3 begins (2 on the first line after the LCD is turned on) to 4 dots after it ends (2 in double speed): reads FF, writes lost, the index still moves; data registers dead in the DMG's mode after the boot ROM (`colour_palettes.vhd`) | Pan Docs; SameBoy; ceres (zeros, the lock's edges) |
| The LCD's pins | LD0, LD1 | 15 bits of colour | Pan Docs (the CGB's LCD) |

### The LCD's timing

The counters and the interrupt's sources are the DMG's (`lcd_timing.vhd`);
what moves is what the CPU sees of them, and when its writes take effect,
by a few half dots (edges of the 4 MiHz clock). No circuit is known for
these: each is ceres's rule, fitted to Gambatte's, SameSuite's, Mooneye's
and AGE's CGB tests (`doc/cgb-hardware-tests.md`, entries 1 to 6), placed
at the half dot where ceres has it.

A CPU access here begins (its strobe, phase E) a half dot after ceres's,
measured on the PPU's clock, and a read takes the byte as phase H ends, 4
half dots on: it sees the PPU as ceres's reads do, which look 4 half dots
ahead. At normal speed every access lands on the same half dot of the
line's M-cycles, and there a rule moved by up to 3 half dots, either way,
gives the same values: the DMG and the CGB differ in what reads see only
after a speed switch has moved that grid, or for rules tied to a single
half dot.

| What | DMG (netlist) | CGB (model) |
| ---- | ------------- | ----------- |
| STAT's enables | the latches follow the write: the interrupt sees them at once, and every source for a moment (the write's glitch) | 4 half dots after ceres's write (3 after the strobe), with no glitch; at VBlank's latch, at the same moment as it |
| A written LYC | the comparison's next sample | its first sample 6 half dots or more after ceres's write |
| The HBlank source of the interrupt | rises 14 ns after mode 3 ends (after the LY = LYC flag of that edge), falls with mode 0 | rises as mode 3 ends, before anything else of that half dot; falls a half dot after mode 0 |
| The LY = LYC flag, read | as the comparison sampled it | 3 half dots later; on the CGB-C it drops as LY changes, as on the DMG; on the CGB-E it holds until the next sample |
| Mode 2, read | the scan's flag (from h = 906) | from h = 909 to 157, then mode 3 |
| Mode 1, read | VBlank's latch (from h = 908 of line 143) | from h = 911; on the CGB-E until 2 half dots after the latch's end |
| LY, read | the counter | the counter; when it changed at the last edge, old and new ANDed |
| LY on line 153 | 0 from 4 half dots into the line | the same on the CGB-C; 6 on the CGB-E |

Double speed has the same rules for now: its own are not modelled yet.

With these and the CPU's HALT (below), cgb-acid2 makes the same accesses
as on ceres and draws the same 14 pictures, which match the real CGB's
(its reference picture).

## Left out

What the model does not have, and why:

- **The LCD's double-speed rules.** In double speed the model keeps the
  normal speed's rules. Ceres's double-speed delays are fits to
  Gambatte's tests, and no single phase of the PPU against the CPU, with
  reads and writes where this CPU core puts them, gave them all: further
  fitting would add a guess per signal. Most of Gambatte's double-speed
  LCD and HDMA ROMs fail for this.
- **The last normal-speed LCD timings:** STAT's mode on the first line
  after the LCD is turned on with SCX 7
  (`enable_display/ly0_late_scx7_m3stat_scx0_2`), and an HBlank enable
  late on line 143 (`m1/ly143_late_m0enable_2`). Of the 69 LCD ROMs whose
  results differ between the DMG and the CGB, the rest pass.
- **The CGB-C's other differences** (`doc/cgb-hardware-tests.md`, entry
  6): the LCD's timing has those that ceres knows (`REVISION`); the rest
  would be more fitting.
- **The infrared port (RP, FF56), PCM12 and PCM34 (FF76, FF77)**: small
  registers that nothing else depends on.
- **The APU's other CGB differences** (wave RAM while channel 3 plays,
  the lengths while the APU is off), **the serial port's fast clock (SC bit
  1), FEA0 to FEFF, OAM DMA's sources and the OAM bug**: not done; each
  would be a row in the tables above.

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
