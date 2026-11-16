# ceres-netlist: the readable model's blocks in Rust

Rust transcriptions of the blocks of the readable model
([`hdl/rtl`](../hdl/rtl), described in [hdl/README.md](../hdl/README.md)),
which ceres-core's proofs against the netlist hold ceres to
([doc/verification.md](../doc/verification.md)). The VHDL files named
below are in `hdl/rtl`; the commands run from the repository's root.

## Checking them against the model

ceres-core's proofs against the netlist (`cargo kani -p ceres-core`) rest
on Rust transcriptions of some of the model's blocks, in the crate
`ceres-netlist` (which ceres-core uses under Kani, and under its feature
`verify` for the Lean proofs, `lean/`; it takes nothing from ceres):
`NetTimer` (`timer`, of `divider.vhd` and `timer.vhd`), `NetIf`
(`interrupts`, of `interrupts.vhd` and the request latch of
`sm83_interrupts.vhd`), `NetDma` (`dma`, of `dma.vhd`, clocked by
`dma_phi`, which stops while the CPU is halted), `NetLcd` (`lcd_timing`,
of `lcd_timing.vhd`'s line clock, LY, VBlank, LY = LYC, the latches of LYC
and STAT's enables, and the STAT interrupt line, edge by edge; the first
block of the PPU), `NetScan` (`oam_scan`, of `oam_scan.vhd`: the scan's
start and stop, the sprite index, the Y comparator and the save strobes,
on `NetLcd`'s clocks), `NetOamLatches` (`oam`, of `oam.vhd`'s two
stages of latches of the Y, which OAM DMA holds) and `NetClocks`
(`clocks`, of `clocks.vhd`'s DMG part: the latches that count the phases,
the clocks decoded from them, and the resets, through the power-on wait).
`hdl/trace-blocks.sh` records those
blocks at the end of every phase while the model runs a ROM (the testbench's
`BLOCKS`), and ceres-netlist's test `netlist_replay` replays every trace in
`hdl/build/blocks` through the transcriptions, phase by phase, with the model's
accesses and interrupt requests, and lists where a transcription's state
parts from the model's, and how often the traces exercised each rule:

```sh
ls external/test-roms/gbmicrotest/timer_*.gb |
	TEST=gbmicrotest xargs -P 4 -I{} hdl/trace-blocks.sh {} 0.1
cargo test -p ceres-netlist --release --test replay -- --ignored --nocapture
```

The blocks also replay wired together, as `dmg_cpu_b.vhd` wires them
(`NetChip`, ceres-netlist's `chip`): the clocks run the phases the timer,
IF, OAM DMA and the line timing step on, the divider's last bit ends the
clocks' power-on wait, the timer's interrupt, the STAT line and VBlank
rise into IF, the DMA runs on the clocks' `dma_phi`, LCDC
(`NetLcdControl`, of `lcd_control.vhd`) holds the PPU in reset and gives
the scan the sprites' height, the OAM scan runs on the line timing's
clocks and compares what OAM's latches hold, the latches follow the scan,
the DMA and render's sprite fetch, and render and the sprite store (below)
run on what the others give them and give the STAT line and STAT their
modes 0 and 3. Only the CPU's access, the other interrupt sources, the
acknowledges, OAM's RAM and odd byte and VRAM's data come from the trace,
which must have render's fields. Over those 267 traces (gbmicrotest's DMA,
timer, halt, OAM, sprite, SCX and window ROMs, Gambatte's `sprites` and
`oamdma` ROMs, 12 of Mealybug's `m3_` ROMs, and `hdl/probes/lcdc.asm`, which
writes every value to LCDC and reads it back) it finds no difference in
1.72 billion phases: 360 timer interrupts, 42473 rises of the STAT line
and 1229 of VBlank reach IF, 363 transfers start on the clocks'
`dma_phi`, LCDC turns the LCD on 1023 times and off 757 times, 816 reads
of LCDC and 452908 of STAT, LY and LYC take what the registers show, and
the scan saves 122070 sprites from what the latches hold. LCDC's latches
open after the edge that starts the write's first phase
(`lcdc-write-latch-delay`): taken at the edge, every LCD turned on starts
its clocks an edge early.

Render (`render.vhd`, the DMG's part) replays as `NetRender`
(ceres-netlist's `render`), from the traces that have its fields (the
trace's last 36: render's flip-flops and latches, what it takes in and
gives out, and the sprite store). Render runs on clocks it makes from the
PPU's (the dots, the pixels' clock, the fetcher's, the tile register's), and
which of them ticks at an edge depends on the order in which the simulator
settles them, so `NetRender` settles render as the simulator does: each
concurrent statement is a wire a delta cycle behind its inputs, each
process reads the current delta cycle, the flip-flops change 1 ns after
their clock's edge, the other blocks' outputs reach render at 1 ns, the
OAM scan's flip-flops clocked by `wuvu` at 2 ns, and VRAM's data at 3 ns
(the address reaches VRAM 2 ns after it changes). The sprite store
(`sprite_store.vhd`) settles with render, as its X matchers and render's
pixel counter feed each other within an edge. Over
the 267 traces with render's fields it matches the model in every
flip-flop and latch, in the sprite store's slots, fetched flags and scanned
index, and in its VRAM address, its CP, ST and LD pins and the store's
match and line:
mode 3 ends 187198 times, the window starts 10962 times, 25474 sprite
fetches start. Without the delta cycle between `myso` and the tile
register's clock, the pulse that clocks the tile register as the window
starts (`window-start-tile-register-pulse`) is lost, and
`m3_lcdc_win_en_change_multiple_wx` differs.

OAM (`oam.vhd`, `NetOam` in ceres-netlist's `oam`) and VRAM's side
(`vram_bus.vhd` and the board's VRAM chip and keeper) settle with render,
in `NetRender`, as render, OAM and VRAM feed each other within an edge;
they complete the PPU, so `NetChip` takes from the trace only the CPU's
access, the other interrupt sources, the acknowledges, the data pins and
the core's `non_vram_mreq`. VRAM starts as the board fills it (its
xorshift, seed 2), OAM as FF. What reaches render, OAM and VRAM does so at
its time: the clocks decoded from the phase (`clk_t4`, `cpu_wr_sync`,
`cpu_ext_cs_en`, `dma_phi`) with the clock's edge; the flip-flops' outputs,
the line timing's and the DMA's 1 ns after it; the scan's flip-flops clocked
by `wuvu` (and with them `scanning`, from which render makes mode 2) at
2 ns, its index at 3 ns, a port behind mode 2; the data pins at 2 ns; the
CPU's address, strobes and data bus at 4 ns, after what drives them;
VRAM's data 2 ns after its address. The times decide races the traces
show: a CPU write of OAM as mode 2 starts also writes the scan's word
(`oam_write_l0_e`), OAM's data lines catch a STAT read in the moment OAM
is free between the scan and mode 3 (`oam_write_l1_a`,
`10spritesPrLine_m2stat_1`), and the first latches catch OAM DMA's first
byte while a CPU read of OAM ends (`dma_timing_a`). While nothing drives
VRAM's data pins, the board's keeper holds them against the pads' pull-ups
(the bits it holds low resolve to `W`, printed as 00), and which byte it
caught depends on the deltas through the ports; nothing reads them then,
so the replay compares VRAM's data only while driven. Over the 269 traces
the replay finds no difference in 1.73 billion phases: 335745 CPU writes
of VRAM, 363 OAM DMA transfers, 38 CPU reads of OAM and 4 of VRAM (FF
where the PPU holds them: nothing drives the precharged bus), 168 reads of
SCY, SCX, BGP, OBP0, OBP1, WY and WX, 25 of FF46.

The bus side (`external_bus.vhd`, `high_ram.vhd`, `boot_rom.vhd`,
`joypad.vhd`, `serial.vhd`) and the board's work RAM and cartridge (its
ROM, RAM and MBC) replay as `NetBus` and `NetBoard` (ceres-netlist's
`bus`), from what the CPU core gives (its address, `read`, `write`,
`mreq`, the data bus), with the cartridge the ROM that `hdl/trace-blocks.sh`
names beside each trace (`hdl/build/blocks/<ROM>.rom`). The traces' last 12
fields hold the core's request, the pins, the bus side's latches and
flip-flops, and whether the address latches were open in the phase, for
however short: as an access inside the chip starts, the core's request
pulses for a moment, and the latches can take the address and the data pins
the data bus. That is the core's timing, so the bus side's replay takes the
latches' address from the trace then (`NetChip` works it out, below). As
an access ends, the latches take the high byte as 00 (the core changes it
before its request falls); the serial
port's half clock is unknown until a write of SC; its eighth bit clears the
transfer at once. While nothing drives the data pins the board's keeper
holds them against the pull-ups, as on VRAM's, and the replay compares
them, and the data latches, only while driven. Over the 269 traces it
finds no difference: 16 million reads on the cartridge bus, 400 thousand
writes of HRAM, 189 serial transfers. Wired into `NetChip` with the rest, they
leave the chip's inputs the CPU core's (its address and what drives its
high byte, read, write, request, data bus, acknowledges; the core is in
`NetChip` too now, below), the clock pins and the world outside (the
joypad's lines, SIN, SCK): over the 269 traces the closed loop finds no
difference either. The request's pulses come from
`latch_race`, which follows the few signals of the race through the phase
in picoseconds, with the VHDL's delays: the core's request reaches the
chip 22 ns after the edge, the address's high byte at 21 ns (precharged),
22 (from W), 25 (PC, BC, DE, FF) or 26 (HL, SP), its low byte at 32; the
chip decodes VRAM in 0.3 ns and FE00 to FFFF and the boot ROM in 10, which
gate the request; the request outside VRAM rises after 2.4 ns and falls
after 1.35, and the latches' enable passes no pulse shorter than 2.7. So
an access to FE00 to FFFF opens the latches for a moment, and so does a
VRAM access through HL or SP, while one through BC, DE or PC is too short
and one through W has none. Against the openings the traces show, over
the 437 traces: 16 million as accesses end, a million at the start of
accesses to FE00 to FFFF, 64541 of VRAM accesses through HL or SP, with no
difference, and none where 10240 VRAM accesses through BC, DE or PC and
80306 through W have none either. Every read the core makes gets its
byte from `NetChip`, as the AND of what drives the bus from E to H: the
PPU, OAM DMA, the bus side, the divider and the timer (TAC's unused bits
high), IF through its read latch, IE (the core's own), the APU (below),
and the precharged FF where nothing drives. Each matches the byte the core
takes: 16 million reads on the cartridge bus, 465 thousand of STAT, LY or
LYC, 230 thousand of HRAM, 142 of the divider and the timer, 17 of IF, 8
of IE, 117 of the I/O addresses nothing decodes (most of these from
`hdl/probes/io_regs.asm`), 362 of the APU's registers, 117 of its unused
addresses and 208 of wave RAM.

The CPU core (`sm83.vhd` and its parts: the decoders, the sequencer, the
registers, the ALU, the IDU, the interrupt logic) replays as `NetCore`
(ceres-netlist's `sm83`), phase by phase: the edge that starts a phase
clocks its flip-flops (the sequencer's at the rise of `clk`, IME as
`exec_phase` falls, the opcode as the fetch ends), then its latches and
wires settle until nothing changes. The traces' last 14 fields hold the
sequencer's and the ALU's latches and flip-flops, the opcode, IR, the
M-cycle, the registers, the requests the interrupt logic latched, the
core's data bus, and the bits of the chip's data bus that were a strong 0
in the phase, for however short: the bridge between the two buses passes
only those, not the keeper's weak 0, nor an X where drivers fight (OAM's
data lines and a STAT read in `oam_write_l1_a`), which the data bus's own
field shows as 0. From the trace the core takes the clocks, the resets,
IF, those bits and the chip's decoding of its address. Over the 270
traces it matches the model in every latch and register, and in its
address, request, read, write, acknowledges and HALT's and STOP's
enables: 2.11 billion phases, 10 million opcodes fetched, 31 thousand
after the CB prefix, 30355 interrupt entries (182 of them choosing
between requests), 2913 HALTs, 6194 failed conditions.

In `NetChip` the core runs with the rest, which leaves the chip's inputs
the clock, reset and test pins and the world outside (the joypad's lines,
SIN, SCK). Each phase the core settles twice: first with the data bus and
IF as the phase finds them, which gives the blocks its address, strobes
and what it writes, then with what they drive on the data bus and IF as
they leave it. Its bridge passes the bits pulled strongly low in the
phase: a block's 0, or the core's own 0 where no block drives a 1 against
it (where STAT's mode bits rise during a read, the two fight to an X);
each drives what it did in the phase before for a while after the edge,
as the core's outputs reach the bus 39 ns after it. Those bits match the
trace's in all 2.33 billion phases, and the closed loop matches the model
in every block, the core's latches and registers and its outputs among
them. OAM's data lines follow the data bus while the PPU leaves them, so
render takes it with a read's byte on it, and with STAT's mode bits for
either level of its own mode 3, and ANDs in its own VRAM data.

`NetChip` also runs a ROM on its own from the power-on, with nothing from
a trace (ceres-netlist's `run`, and its `run` example):

```bash
cargo run -p ceres-netlist --release --example run -- mooneye hdl/build/io_regs.gb 0.4
```

`NetChip::power_on` is the VHDL's initial values, settled once with no
edge for any flip-flop (the signals start undefined, and a rise from
undefined is no edge), and the chip runs on the board's pins
(`ChipInputs::board`: the crystal, /RESET low for 64 periods, no button
pressed, nothing on the link); the run ends as the testbench ends a test,
and its time limit counts from the end of the reset, as the testbench's
does. Traces from the power-on check it (`FROM=power-on`, and `BOOTROM=`
for another boot ROM, which `hdl/trace-blocks.sh` keeps beside the trace): the
replay runs the chip from its power-on state on the board's pins against
them, and finds no difference over `hdl/probes/io_regs.asm` from the fast boot
ROM, and `poweron_stat_026` and `halt_bug` from dmg-sim's quick boot ROM.
`ceres-netlist/tests/run.rs` runs the ROMs of `hdl/tests.txt` so, with
dmg-sim's quick boot ROM as `hdl/run-tests.sh` does, and all 47 results are
dmg-sim's (42 pass), in two and a half minutes on four threads: dmg-sim
takes 13 minutes for `io_regs` alone, `NetChip` 1.5 s. Two things the
traces from the CPU's first access never needed: the divider's and the
timer's resets (/RESET and STOP's stopped oscillator clear the divider,
`reset2` TMA, TAC and TIMA), and the timer's enable race, which `NetChip`
counts as the netlist does (`timer-enable-race`: `halt_bug` then fails as
on dmg-sim, and passes as on a Game Boy without it).

The APU (`apu_control.vhd` and the four channels) replays as `NetApu`
(ceres-netlist's `apu`), delta cycle by delta cycle as render does: its
clocks are dividers of the chip's clock and counters whose bits clock the
next stage, so the order in which the simulator settles them decides what
ticks. Each flip-flop keeps its own pending change, 1 ns after its clock.
The traces' last 45 fields hold the control's flip-flops and NR50 and
NR51, and each channel's registers, counters, envelope, length, channel
1's sweep, channel 3's bit lines and wave RAM, channel 4's prescaler and
LFSR; the mixer, which the model computes in volts, is left out. Two
times decide what the traces show: a write's strobe reaches the APU with
the edge, before the CPU drives the data bus, so a write of NR12 first
opens the envelope's latch on the precharged FF and toggles the volume's
first stage; and the read strobe reaches it before the address, so a read
of wave RAM ends before the address moves and the bit lines keep the byte
read. Over the 273 traces the APU matches the model in every flip-flop,
latch and RAM cell. Almost all of it is `hdl/probes/apu.asm`'s and
`hdl/probes/sound.asm`'s, the other ROMs leave the APU powered off; the second
lets every channel's length run out, overflows channel 1's sweep and
updates it, takes envelopes to 15 and 0, plays every duty and wave level,
shifts channel 4's short LFSR and powers the APU off and on (and on its
own, 23.7 million phases, it finds a duty of channel 1 changed in the
transcription). In `NetChip` the APU runs on the clocks, the resets and
the divider's bit 10 the chip gives it, and answers the reads of FF10 to
FF3F: each register drives its bits (the unused and write-only ones stay
precharged, 1), NR52 the power and the channels' `active` ports, wave RAM
its bit lines. The closed loop matches the model there as well, in every
flip-flop and in every read.

`hdl/probes/ppu_regs.asm`, `hdl/probes/vram_oam.asm`, `hdl/probes/bus.asm`,
`hdl/probes/io_regs.asm` and `hdl/probes/sound.asm` read back the PPU's registers,
VRAM, OAM, the bus side, the rest of the I/O and the APU, and check what
they read: every register's bits
with the LCD on and off, FF46 during and after OAM DMA, every byte of VRAM
and OAM, VRAM and OAM locked out in modes 2 and 3; serial bytes sent with
the internal clock (SB, SC and IF after), P1's select bits, FF50, the
cartridge's header, every byte of HRAM and work RAM, and the echo of work
RAM; IE, IF, TMA, TIMA and TAC with their unused bits, TIMA counting, DIV
after a write, and the I/O addresses nothing decodes (FF in all bits, as
Mooneye's `unused_hwio-GS` has them); the APU's registers with their unused
and write-only bits, NR52 with the APU off and on, and NR52's channel bits
as lengths run out and the sweep overflows. They report as
`hdl/probes/report.inc` says (Mooneye's registers and `LD B,B`, "Passed" or
"Failed" on the serial port, a white or black screen), so they run on a
Game Boy as well; they pass on dmg-sim, the model and ceres
(`ceres-test-runner/tests/hdl_checks.rs`), and are in `hdl/tests.txt`.

`FROM=power-on hdl/trace-blocks.sh ROM` traces from the power-on instead,
into `hdl/build/blocks/power-on/`, for the clocks' reset and wait; the replay
takes only the clocks there, as the other blocks are held in reset.

A request that rises and falls within a phase (the STAT write's glitch)
counts as a rise in that phase: the trace notes every rising edge of the
request lines, not only their values at the phase's end. The results are in
[results.md](../hdl/results.md#ceress-transcriptions-of-the-timer-if-and-oam-dma)
and, for the line timing and the OAM scan,
[results.md](../hdl/results.md#ceress-transcription-of-the-line-timing) and
[results.md](../hdl/results.md#ceress-transcription-of-the-oam-scan).
