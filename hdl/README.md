# Gate-level VHDL model of the Game Boy

A VHDL description of the DMG-CPU B, the chip of the original Game Boy, at
the level of its logic cells. It serves as the reference for how the hardware
behaves, and as the base for finding out what we do not know about it.

The model is generated from two sources:

- the netlist of the chip traced from the die by Michael Singer
  ([dmg-schematics](https://github.com/msinger/dmg-schematics), the submodule
  `external/docs/dmg-schematics`): which cells there are, and how the wires
  connect them, with their length on the die;
- the cell models of [dmg-sim](https://github.com/msinger/dmg-sim) (the
  submodule `external/reference-implementations/dmg-sim`), the same author's
  SystemVerilog simulation of that netlist: what each cell does, and its delays,
  computed from the transistor widths and the wire lengths, and fitted to
  measurements of a real chip.

`ceres-hdlgen` (in the workspace) reads both and writes the VHDL into
`build/gen`, which is not committed:

- `cells.vhd`: the cell library, translated from dmg-sim's SystemVerilog;
- `sm83.vhd`: the SM83 CPU core, from the netlist;
- `dmg_cpu_b.vhd`: the chip, from the netlist.

The rest is written by hand:

| Path | Contents | License |
| --- | --- | --- |
| `support/verilog.vhd` | Verilog semantics in VHDL (delays, strengths, keeper) | MIT |
| `gate/timing.vhd` | the delay model of dmg-sim | CC BY-SA 4.0 |
| `gate/cells.vhd` | the memories, the boot ROM and the analog audio | CC BY-SA 4.0 |
| `tb/gameboy_tb.vhd` | a Game Boy board: crystal, reset, RAMs, cartridge | MIT |

The files in `gate/`, and the VHDL generated into `build/gen`, derive from
dmg-schematics and dmg-sim and are licensed under
[CC BY-SA 4.0](gate/LICENSE), like them. The generator and the other files are
MIT, like the rest of Ceres.

## Running it

You need [nvc](https://github.com/nickg/nvc) (or
[GHDL](https://github.com/ghdl/ghdl)), GNU make, and the submodules
`external/docs/dmg-schematics`, `external/reference-implementations/dmg-sim` and
`external/gb-bootroms`.

```bash
make                                # generate and analyse
make run TIME="40 ms"               # simulate, pins traced to trace.txt
make run TIME="2 s" ROM=game.gb     # with a cartridge (no memory bank controller)
make SIM=ghdl run TIME="40 ms"      # with GHDL
```

The chip spends its first 31.2 ms in its power-on reset; the CPU starts after
that. Every change of the pins goes to the trace file as `<time> <pin>
<value>`. A second of the Game Boy takes minutes to hours to simulate.

## How it follows dmg-sim

dmg-sim runs on Icarus Verilog, and its delays were fitted there, so the VHDL
reproduces how Icarus runs the cells (`support/verilog.vhd` and the
architectures that `ceres-hdlgen` writes):

- A module path (`specify`) delays an output from the last change of the
  input that changed most recently, and a change that arrives while another is
  pending replaces it at the pending time.
- Inside a cell, a continuous assignment reads an output after its path delay,
  and an `always` block reads it before.
- Gates and continuous assignments pick their delay by the new value and
  filter shorter pulses, as VHDL's inertial delay does.
- Delays are rounded to 1 ps, the precision of dmg-sim.
- The wire lengths are computed as nlconv computes them for dmg-sim, in single
  precision and printed with 7 significant digits.
- std_logic has two strengths, where Verilog has more: strong is `0`/`1`, pull
  and weak are `L`/`H`, and a bus keeper gives way to a pull-up.

Compared with dmg-sim on the boot ROM without a cartridge, every pin of the
chip changes at the same picosecond from 100 µs to 81 ms (the power-on reset
and the first 50 ms of the boot ROM), and every internal net of the chip and
of the CPU has the same value in the windows checked. nvc runs it about as fast
as Icarus runs dmg-sim.

## Limits

- As in dmg-sim's default build, OAM and wave RAM use a simplified SRAM: the
  OAM corruption bug and the wave RAM corruption of Blargg's dmg_sound test 10
  are not reproduced.
- dmg-sim's testbench forces the OAM data lines during a conflict between OAM
  DMA and the CPU; this is not ported yet.
- The cartridge has no memory bank controller yet (dmg-sim has MBC1 and MBC5).
- While the chip powers up, unknown values can settle differently from Icarus
  during the first 100 µs.
