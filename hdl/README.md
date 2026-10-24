# The hardware oracle and the readable model

The reference for how the Game Boy's hardware behaves is the chip itself,
the DMG-CPU B, at the level of its logic cells:

- [dmg-schematics](https://github.com/msinger/dmg-schematics) (the submodule
  `external/docs/dmg-schematics`) is its netlist, traced from the die by
  Michael Singer: which cells there are, and how the wires connect them,
  with their length on the die.
- [dmg-sim](https://github.com/msinger/dmg-sim) (the submodule
  `external/reference-implementations/dmg-sim`) is the same author's
  SystemVerilog simulation of that netlist, run with Icarus Verilog: what
  each cell does, and its delays, computed from the transistor widths and
  the wire lengths, and fitted to measurements of a real chip.

This directory runs test ROMs on dmg-sim, unchanged, with probes of ours
compiled in next to it, and records what the netlist does. It is the oracle
for ceres and for [the readable model](#the-readable-model): VHDL that
describes the same hardware block by block, written to be read.

## Running it

You need [Icarus Verilog](https://github.com/steveicarus/iverilog) (13 is
tested) and GNU make, and the submodules
`external/reference-implementations/dmg-sim` and `external/gb-bootroms`.

```bash
make                                 # build dmg-sim, with the probes
make run SECS=0.01                   # 10 ms after the reset, pins to trace.txt
make run SECS=1 ROM=game.gb          # with a cartridge (none, MBC1 or MBC5)
make smoke                           # check that the oracle has not changed
make phases SECS=0.05 ROM=game.gb    # the signals of each phase, to phases.txt
```

The chip spends its first 31.2 ms in its power-on reset, and `SECS` counts
the time after it, as dmg-sim does. Every change of the pins goes to the
trace file as `<time in ps> <pin> <value>`. A second of the Game Boy takes
about an hour to simulate, and two when six simulations run at once.

`make smoke` simulates 33 ms from power-on with dmg-sim's quickboot ROM and
checks the hash of the pin trace (sorted) against `smoke.sha256`. It fails
if a new version of dmg-sim or of Icarus changes what the chip does; the
test ROMs then have to run again.

## Probes

The probes in `oracle/` are SystemVerilog modules that reach into dmg-sim by
hierarchical names. They do not change the simulation.

- `probe_pins.sv` traces the pins (`+PINS_FILE=<path>`).
- `probe_phases.sv` samples the signals that the readable model is checked
  against, once per phase (`+PHASES_FILE=<path>`).
- `probe_results.sv` recognises how a test ROM reports its result, and ends
  the simulation as soon as it is known (`+TEST=<kind>`,
  `+RESULT_FILE=<path>`, `+SERIAL_FILE=<path>`): Mooneye's registers at
  `LD B,B`, gbmicrotest's bytes at `FF80` to `FF82` in high RAM, and
  Blargg's text on the serial port.

## Test ROMs

`run-tests.sh` runs the test ROMs of `tests.txt`, six at a time, and writes
the results to `build/results.txt`. Each test starts with the power-on
reset and dmg-sim's quickboot ROM, which skips the logo but keeps the
timings of the boot ROM (DIV, the PPU, the APU): the CPU reaches the
cartridge's entry point at 182 ms. The results are in
[results.md](results.md).

dmg-sim has its own test runner (`tests/` in its submodule), which runs each
test for a fixed time and compares the screen with a picture; ours stops at
the result, and needs neither the pictures nor ImageMagick.

## Limits

These come from dmg-sim:

- As in its default build, OAM and wave RAM use a simplified SRAM: the OAM
  corruption bug and the wave RAM corruption of Blargg's dmg_sound test 10
  are not reproduced.
- When OAM DMA and the reading of OAM drive the OAM data lines at once,
  which the gates cannot resolve, dmg-sim's testbench forces the lines to
  the values it expects.
- The delays are a model, fitted to measurements of one chip.
- Icarus 13 has no string method `tolower`, which dmg-sim uses only for its
  event files; the build comments it out in its copy of dmg-sim.

## The readable model

`rtl/` describes the chip in VHDL, one file per block, in the words of
what each block does rather than of its cells:

| File               | Block                                                    |
| ------------------ | -------------------------------------------------------- |
| `clocking.vhd`     | the phases A to H of an M-cycle                          |
| `clocks.vhd`       | the clocks and the resets                                |
| `divider.vhd`      | the divider and DIV (FF04)                               |
| `timer.vhd`        | TIMA, TMA and TAC (FF05 to FF07)                         |
| `interrupts.vhd`   | IF (FF0F)                                                |
| `serial.vhd`       | the serial port (FF01, FF02)                             |
| `joypad.vhd`       | the joypad (FF00)                                        |
| `test_mode.vhd`    | the test pins and FF60                                   |
| `boot_rom.vhd`     | the boot ROM and FF50                                    |
| `high_ram.vhd`     | high RAM                                                 |
| `external_bus.vhd` | the cartridge bus, and the precharge of the data bus     |
| `dma.vhd`          | OAM DMA (FF46)                                           |
| `lcd_control.vhd`  | LCDC (FF40) and the PPU's resets                         |
| `lcd_timing.vhd`   | lines and frames: LX, LY, LYC, STAT, the LCD's sync pins |
| `oam_scan.vhd`     | the OAM scan (mode 2)                                    |
| `sprite_store.vhd` | the ten sprites of the line, and their X matchers        |
| `oam.vhd`          | OAM, its buses and its latches                           |
| `render.vhd`       | mode 3: the fetcher, the window, the shifters, the pixel |
| `vram_bus.vhd`     | the VRAM bus                                             |
| `apu_control.vhd`  | the APU's power, clocks and frame sequencer; the mixer   |
| `apu_ch1.vhd`      | channel 1: square wave with a sweep (FF10 to FF14)       |
| `apu_ch2.vhd`      | channel 2: square wave (FF16 to FF19)                    |
| `apu_ch3.vhd`      | channel 3: wave RAM (FF1A to FF1E, FF30 to FF3F)         |
| `apu_ch4.vhd`      | channel 4: noise (FF20 to FF23)                          |
| `sm83.vhd`         | the CPU core: its blocks, its data bus and the bridge    |
| `sm83_decoder.vhd` | the first decoder: the step of the instruction           |
| `sm83_control.vhd` | the second and third decoders: the control lines         |
| `sm83_sequencer.vhd` | the opcode, the M-cycles, HALT, STOP and IME           |
| `sm83_registers.vhd` | the registers and the buses that read them             |
| `sm83_idu.vhd`     | the IDU: increments and decrements of 16 bits            |
| `sm83_alu.vhd`     | the ALU and the flags                                    |
| `sm83_interrupts.vhd` | IE, the interrupts' priority and vectors              |
| `dmg_cpu_b.vhd`    | the chip: the blocks wired together, behind its pins     |

`dmg_cpu_b.vhd` has the chip's pins, as dmg-sim's `dmg_cpu_b` has them.

The model keeps time in phases: every edge of the 4 MiHz clock starts one,
and eight of them, A to H, make an M-cycle (`clocking.vhd`). Registers
change at those edges, or at the edges of the signals that clock them in
the chip; latches are latches. Each behaviour carries a tag that says where
it comes from: `@hw die:` names the cells of the netlist that it describes.
A flip-flop's output changes 1 ns after its clock's edge (`tco`), so that
every flip-flop clocked by an edge, or by a clock derived from it, sees
the values from before the edge, as in the chip. Where the chip's
behaviour depends on which of two signals arrives first within a phase (a
glitch), the model says so in a comment, with a delay (`after`) of the
size dmg-sim has; buses that keep their value when nobody drives them get
a driver delay of 1 ns, so that the glitches of the model's zero-delay
logic do not reach them.

### Running it

`board/` is the Game Boy around the chip, as dmg-sim's board
(`dmg_cpu_b_gameboy.sv`) has it: the crystal, the reset at power-on, the
cartridge (its ROM and RAM, with no MBC, MBC1 or MBC5), the work RAM, the
VRAM chip, the buttons, and keepers on the buses (`gameboy.vhd`).
`gameboy_tb.vhd` runs a ROM on it, recognises the test ROMs' results as
the oracle's probe does, and can save the last frame on the LCD as a
picture:

```bash
make run-model SECS=0.1 ROM=game.gb SCREEN=screen.pgm
MODEL=1 ./run-tests.sh            # the test ROMs, to build/results-model.txt
```

The model runs about 25 times as fast as dmg-sim: a second of the Game
Boy takes about two and a half minutes, and the list of `tests.txt` 9
minutes, six at a time. On every test of the list, it gets the result
dmg-sim gets, with the same values, at the same time (to the 0.1 ms of
[results.md](results.md)). The board's crystal runs at dmg-sim's 119 ns per
half period, which rounds the real one's 119.2 ns, so that the times match.

### Comparing ceres with it

`compare-ceres.sh` runs a ROM on the model and on ceres, both from
power-on with the same boot ROM, and compares what their CPUs do on the
bus: one line per access, with the M-cycle, R or W, the address and the
byte. The first line that differs is where ceres and the chip part ways.

```bash
TEST=gbmicrotest ./compare-ceres.sh external/test-roms/gbmicrotest/line_153_ly_a.gb
```

The boot ROM is `build/fastboot.bin`, which turns the LCD on and jumps to
the cartridge right after the reset; `BOOT=quick` runs dmg-sim's quickboot
instead, 150 ms longer, for the ROMs that test the state the real boot ROM
leaves (`BOOT=tiny`, below, has its state in less time). `TEST` ends the run
at the test's result. A gbmicrotest takes about a second this way, and the 513
of them 10 minutes, six at a time:

```bash
ls ../external/test-roms/gbmicrotest/*.gb | sed 's|^\.\./||' |
	TEST=gbmicrotest NOFRAMES=1 xargs -n 1 -P 6 ./compare-ceres.sh
```

The traces go to `build/compare/<ROM>/`: `model.txt` from the model's
runner (`BUS_TRACE`), `ceres.txt` from ceres's example `bus_trace`
(`ceres-test-runner/examples/bus_trace.rs`), which runs the same boot ROM
(`GbBuilder::with_bootrom`).

It compares the screen too: every frame the LCD completes, as the model's
LCD pins draw it (`FRAMES`) and as ceres presents it (`bus_trace --frames`),
in `model-frames.txt` and `ceres-frames.txt` (`frame N`, then 144 lines of
160 digits, 0 white to 3 black). Neither writes the first frame after the
LCD is turned on, which the LCD does not show (its first line is not a whole
one on the pins), and `compare-frames.sh` leaves out the all-white frames
before the first picture. The report gives the first picture that differs,
how many pixels differ in it and the first of them, and how many pictures
differ, the last among them. A ROM that draws on its own needs
no `TEST`, only `SECS` long enough, and `BOOT=quick`: fastboot leaves VRAM
as the board's chip powers up, with arbitrary bytes that the ROM may show,
where quickboot zeroes it as the real boot ROM does (`NOFRAMES=1` skips the
screen; `BOOT=real` runs the real DMG boot ROM, for ROMs that need its state):

```bash
BOOT=quick ./compare-ceres.sh external/test-roms/dmg-acid2/dmg-acid2.gb 0.6
```

When the pictures differ, `PIXELS=1` also compares what the pixel pipeline
did, line by line (`compare-pixels.sh`, `model-pixels.txt` and
`ceres-pixels.txt`): when each pixel was pushed to the LCD, when each tile
fetch began, in the window or in the background, and when the ROM wrote
LCDC, SCY, SCX, BGP or WX, all in half dots from the line's first pixel. Two
timelines that agree while the pictures differ mean that the difference is in
the data (a map, a row, a palette), not in the timing; with a name
for the line, `compare-pixels.sh model-pixels.txt ceres-pixels.txt 16` shows
only that line.

```bash
BOOT=quick PIXELS=1 NOFRAMES=1 ./compare-ceres.sh \
	external/test-roms/mealybug-tearoom-tests/ppu/m3_lcdc_win_en_change_multiple_wx.gb 0.5
```

The two sides place some events by their own conventions: the model's tile
fetches begin half a dot sooner than ceres's, which the script corrects; and
a register's write lands, on each side, a fixed dot or so apart from the
other's (the same in every line), which it does not.

`CUTS=1` also compares what the chip's blocks show each other, which the
CPU sees only when it reads it: IF, TIMA, the OAM DMA's run, LY and STAT's
mode bits as a read would see them, and the divider where it does not count
on (a write to DIV, STOP), a line per change (`compare-cuts.sh`,
`model-cuts.txt` and `ceres-cuts.txt`). The model samples them when it
samples the accesses, ceres at the end of each step of its time, so a kind
of change (mode 3 starting, TIMA counting) can come an M-cycle apart on the
two sides, always the same: the script finds the usual difference of each
kind, and lists the changes only one side has and those off it. For
instance:

```sh
BOOT=tiny APU=0 CUTS=1 NOFRAMES=1 ./compare-ceres.sh \
    external/test-roms/mooneye-test-suite/acceptance/timer/tim01.gb 0.1
```

makes the same accesses on both sides, and shows ceres's TIMA counting
once more three M-cycles after a write to DIV, which the ROM overwrites
before it reads it.

Over gbmicrotest, ceres and the model make the same accesses in 491 ROMs
of 513. The others differ in three ways (see also
[results.md](results.md)):

- The read after HALT, when an interrupt is already pending (7 ROMs, a
  line each): the chip reads the byte after the HALT, ceres the HALT
  itself. Both push the HALT's address, in the same M-cycles.
- The state at power-on (15 ROMs: `poweron_*`, `000-oam_lock`, `temp`):
  RAM and the registers that no boot ROM writes start as dmg-sim starts
  them, at 0 in every cell (OAM's bytes are FF, and the board's VRAM holds
  arbitrary bytes), where ceres has zeros.
- With quickboot, the timer's enable (`halt_bug`, `004-tima_boot_phase`):
  the netlist counts on enabling the timer, ceres and the real DMG do not.

### Gambatte's ROMs

`run-gambatte.sh` runs Gambatte's DMG test ROMs (the ones whose names carry
`dmg08_out`) on the model from the short boot ROM (below), 16 frames after
the ROM starts, reads the digits in the top row of the screen with Gambatte's
glyphs, as ceres's `gambatte_dmg` does, and compares them with the digits in
the file's name. A ROM alone takes 70 s, but a 12-core laptop (8 threads on
the 4 fast cores, then the slow ones) finishes one every 20 s with 8 or more
at a time, so the 1747 take about 10 hours and `STRIDE=8` (207 ROMs, 70
minutes) runs every eighth; `JOBS` sets how many run at a time (default
six); `BOOT=quick` runs quickboot instead. The results are in
`build/gambatte/results.txt` (see [results.md](results.md)).

### Making a run shorter

The model's time goes to the simulator's events, not to the code it runs, so
what helps is fewer events and less simulated time:

- `BOOT=tiny` starts the ROM from `build/tinyboot.bin` (`boot/tinyboot.asm`,
  built with RGBDS by `make`): a 256-byte boot ROM that does not clear VRAM
  and leaves the machine as quickboot does, to the cycle, at the ROM's entry:
  the LCD on, the divider and the PPU in the same phase, the registers and
  the stack as quickboot leaves them. It reaches the ROM after 17841
  M-cycles instead of 158449, which is 0.13 s less of the Game Boy's time.
  It needs `-gVRAM_ZERO=true` (the script passes it): the board's VRAM chip
  then powers up zeroed, as quickboot's cleared VRAM. Over the 513
  gbmicrotest ROMs and the 1575 Gambatte DMG ROMs, ceres makes the same
  accesses from tinyboot as from quickboot; on the model, the 207 Gambatte
  ROMs sampled give the same results. The sound is not matched (the APU's
  sequencer and the envelopes start in another phase), so it is not for the
  sound tests.
- `APU=0` (`-gWITH_CHANNELS=false` of `dmg_cpu_b`) leaves out the four sound
  channels: their registers and the wave RAM no longer drive the data bus (a
  read gives the bus keeper's last value) and NR52's bits 0 to 3 read 0.
  Nothing else sees them, so a ROM that does not read those registers draws
  the same picture and makes the same accesses. On every eighth of
  Gambatte's ROMs (207), it changed the results of five, all in `sound/`,
  which read NR52 or the wave RAM.
- The `data` bus of each block is split into `data : in` and `data_out : out`
  (the top ties them together): nvc cannot collapse the nets of an `inout`
  port, and the 19 blocks that drive the bus made 22% of all the value
  changes, in about 20 copies of it. That always applies.

One Gambatte ROM, to 16 frames after its start, took 97 s before these
(quickboot), takes 89 s with the split bus alone, 69 s from tinyboot, and 61 s
without the channels. Optimisation levels, ahead-of-time compilation, `--jit`
and `--no-collapse` do not change it (the last makes it 60% slower).

### Checking it

The model is checked against dmg-sim phase by phase: the probe
`oracle/probe_phases.sv` records, at the end of every phase, the signals
that the model has too (under the model's names), and `check/chip_tb.vhd`
runs the model of the chip on the same inputs and compares. Only the
crystal and the pins come from dmg-sim, with what the cartridge, the work
RAM and the VRAM chip answer. The checks need
[nvc](https://github.com/nickg/nvc) (1.23 is tested).

```bash
BOOT=fast PHASES=1 TESTS=check/roms.txt ./run-tests.sh   # record, in dmg-sim
make check                                               # check the model
```

`check/roms.txt` lists the test ROMs the model is checked on. Three of them
are ours: `check/apu.asm` plays the APU's four channels through most of what
they do, in 70 ms, `check/cpu.asm` executes every opcode and takes
interrupts, and `check/sprites.asm` draws sprites whose tiles tell the two
bit planes apart. `make` builds them with [RGBDS](https://rgbds.gbdev.io) (1.0 is
tested).

`BOOT=fast` starts the test ROMs with a boot ROM that only turns the LCD on
and jumps to the cartridge (`build/fastboot.bin`): the recordings then take
minutes instead of the 182 ms of the boot ROM's timing. Their results do
not count, but what the chip does on them does.

Where two drivers on a bus disagree, dmg-sim has `x`, and the check skips
those bits (it reports them as conflicts): which one wins is not known.

This branch also held a VHDL translation of dmg-sim itself (until commit
`d1658a21`), which matched it pin for pin at every picosecond. It was
retired: the original, on the simulator its delays were fitted on, is the
better oracle, and the translation had to follow every change to dmg-sim.
