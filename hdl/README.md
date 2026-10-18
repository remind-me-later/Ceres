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
`build/gen`, which is not committed. The rest is written by hand.

## Where things are

The chip is split as msinger's schematics split it: each category of cells
(the timer, the STAT register, a sound channel, the ALU...) is one VHDL
entity in its own file, whose ports are the wires it shares with the other
blocks and the pins. The cells and nets keep their names from the die
(`u_tola` is the cell TOLA, `n_tola` the net it drives), so a name in the
schematics can be found with grep.

| Path | Contents |
| --- | --- |
| `build/gen/dmg_cpu_b.vhd` | the chip: its pins, and the blocks connected by the wires they share |
| `build/gen/dmg_cpu_b/*.vhd` | its 38 blocks: `timer`, `serial`, `int`, `ppu_stat`, `apu_ch1`... |
| `build/gen/sm83.vhd` | the SM83 CPU core, a component of the chip, built the same way |
| `build/gen/sm83/*.vhd` | its 27 blocks: `reg_bc`, `alu_core`, `dec1`, `seq`, `irq`... |
| `build/gen/cells/` | the cells, one file each (`nand2`, `dffr`...), translated from dmg-sim |
| `gate/cells/` | the cells that are not gates, by hand: the memories, the boot ROM, the analog audio |
| `gate/timing.vhd` | the delay model: from the wire lengths and the transistor widths |
| `gate/keeper.vhd` | the bus keeper, which holds the value of an undriven bus |
| `support/verilog.vhd` | Verilog semantics in VHDL: delays, strengths, unknown values |
| `tb/gameboy_tb.vhd` | the Game Boy board around the chip: crystal, reset, and the parts below |
| `tb/video_ram.vhd`, `tb/work_ram.vhd` | the two RAMs on the board |
| `tb/cartridge.vhd` | a cartridge: ROM, RAM, no MBC, MBC1 or MBC5 |
| `tb/serial_receiver.vhd` | the other end of the serial link (Blargg's tests report there) |
| `tb/oam_conflict.vhd` | dmg-sim's resolution of OAM DMA conflicts, which the gates cannot model |
| `tb/mooneye_result.vhd`, `tb/gbmicrotest_result.vhd` | how these test ROMs report their result |
| `tb/pin_trace.vhd` | the trace of the pins |
| `tb/util.vhd` | helpers: files, ROM images, the result of a test |

The first line of each hand-written file gives its license. The files that
derive from dmg-schematics or dmg-sim, which are all of `gate/`, the parts
of the testbench that follow dmg-sim's, and the VHDL generated into
`build/gen`, are licensed under [CC BY-SA 4.0](gate/LICENSE), like their
sources. The generator and the other files are MIT, like the rest of
Ceres.

## Reading VHDL

VHDL describes hardware, but it reads like a program in a few kinds of
statements:

- An **entity** is the outside of a component: its `generic`s (parameters
  fixed when it is placed, like const generics in Rust) and its `port`s,
  the wires it connects to, each `in`, `out` or `inout`. An
  **architecture** is the inside.
- `u_tola : entity work.dmg_nand2 generic map (...) port map (in1 =>
  n_a, ...)` places one instance of a component and connects its ports to
  wires, like constructing a struct. `work` is the library everything is
  compiled into.
- A **signal** is a wire, such as `n_tola`. Its type `std_ulogic` is one
  bit with nine values: `'0'` and `'1'`, `'Z'` (nothing drives it), `'X'`
  (unknown, such as two drivers that disagree), `'L'` and `'H'` (weakly
  driven, by a pull-up or a keeper), and `'U'` (never set). `std_logic` is
  the same type for a wire that several outputs drive, a bus: its value is
  computed from all of them.
- A **process** is code that runs again whenever a signal in its list
  changes (`process (a, b)`), or that waits (`wait until rising_edge(clk)`,
  `wait for 2 us`). `x <= v after 3 ns` changes the signal `x` later;
  variables (`v := ...`) change at once, as in Rust.
- A **package** holds constants, types and functions, like a module.
- `<< signal ^.dut.u_cpu.n_opcode0 : std_ulogic >>` is an external name:
  the testbench reaching into the chip by the path of a net, as a debugger
  would.

The generated cells (`build/gen/cells/`) model the delays of Verilog gates
with processes, and are the hardest part to read; the blocks are only
instances and wires.

## Running it

You need [nvc](https://github.com/nickg/nvc) (or
[GHDL](https://github.com/ghdl/ghdl)), GNU make, and the submodules
`external/docs/dmg-schematics`, `external/reference-implementations/dmg-sim` and
`external/gb-bootroms`.

```bash
make                                # generate and analyse
make run TIME="40 ms"               # simulate, pins traced to trace.txt
make run TIME="2 s" ROM=game.gb     # with a cartridge (none, MBC1 or MBC5)
make SIM=ghdl run TIME="40 ms"      # with GHDL
```

The chip spends its first 31.2 ms in its power-on reset; the CPU starts after
that. Every change of the pins goes to the trace file as `<time> <pin>
<value>`. A second of the Game Boy takes about an hour to simulate, and two
when six simulations run at once.

## Comparing with dmg-sim

dmg-sim runs on Icarus Verilog, which `make ref` builds from its submodule
with a probe of the pins:

```bash
make run TIME="34 ms" TRACE=model.txt
make ref-run REF_SECS=0.002 REF_TRACE=ref.txt   # 2 ms after the reset
make diff TRACE=model.txt REF_TRACE=ref.txt
```

`make smoke`, which CI runs, simulates 33 ms with dmg-sim's quickboot ROM and
checks the hash of the pin trace against `smoke.sha256`. That trace matched
dmg-sim pin for pin; after a change that alters it, check the new one with
`make diff` before updating the hash.

## Test ROMs

`run-tests.sh` runs the test ROMs of `tests.txt` on the model, six at a time,
and writes the results to `build/results.txt`. The testbench recognises how
each kind of test reports its result and stops there: Mooneye's registers at
`LD B,B`, gbmicrotest's `FF82` in high RAM, and Blargg's text on the serial
port. Each test starts with the power-on reset and dmg-sim's quickboot ROM,
which skips the logo but keeps the timings of the boot ROM (DIV, the PPU, the
APU): the CPU reaches the cartridge's entry point at 182 ms, after about
twelve minutes of simulation (25 when six run at once). The results are in
[results-gate.md](results-gate.md).

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
- While the chip powers up, unknown values can settle differently from Icarus
  during the first 100 µs.
