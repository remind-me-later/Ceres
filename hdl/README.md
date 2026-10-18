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

The CPU core, the APU and the PPU's rendering are not written yet.

The model keeps time in phases: every edge of the 4 MiHz clock starts one,
and eight of them, A to H, make an M-cycle (`clocking.vhd`). Registers
change at those edges, or at the edges of the signals that clock them in
the chip; latches are latches. Each behaviour carries a tag that says where
it comes from: `@hw die:` names the cells of the netlist that it describes.
Where the chip's behaviour depends on which of two signals arrives first
within a phase (a glitch), the model says so in a comment, with a delay
(`after`) of the size dmg-sim has.

### Checking it

The model is checked against dmg-sim phase by phase: the probe
`oracle/probe_phases.sv` records, at the end of every phase, the signals
that the model has too (under the model's names), and `check/chip_tb.vhd`
runs the model on the same inputs and compares. The checks need
[nvc](https://github.com/nickg/nvc) (1.23 is tested).

```bash
BOOT=fast PHASES=1 ./run-tests.sh timer_ dma_   # record test ROMs in dmg-sim
make check                                      # check the model on each
```

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
