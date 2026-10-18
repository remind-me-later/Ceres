# The hardware oracle

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
for ceres and, later, for a readable VHDL description of the hardware
(see [Next](#next)).

## Running it

You need [Icarus Verilog](https://github.com/steveicarus/iverilog) (13 is
tested) and GNU make, and the submodules
`external/reference-implementations/dmg-sim` and `external/gb-bootroms`.

```bash
make                                 # build dmg-sim, with the probes
make run SECS=0.01                   # 10 ms after the reset, pins to trace.txt
make run SECS=1 ROM=game.gb          # with a cartridge (none, MBC1 or MBC5)
make smoke                           # check that the oracle has not changed
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

## Next

The readable description of the hardware will be hand-written VHDL, one
block at a time (timer, interrupts, OAM DMA, PPU, CPU...), each behaviour
tagged with where it comes from: the netlist, a measurement, or a guess.
Free simulators do not run VHDL and SystemVerilog together, so each block is
checked against traces of its signals that a probe records in dmg-sim.

This branch also held a VHDL translation of dmg-sim itself (until commit
`d1658a21`), which matched it pin for pin at every picosecond. It was
retired: the original, on the simulator its delays were fitted on, is the
better oracle, and the translation had to follow every change to dmg-sim.
