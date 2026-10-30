#!/bin/sh
# Records the divider, the timer, IF, OAM DMA, the line timing and the OAM
# scan of the readable model (the DMG's), phase by phase, for ceres-core's
# replay through its transcriptions of them.
#
#   trace-blocks.sh <ROM> [SECS]
#
# runs the ROM (a path relative to the repository) for SECS seconds after
# the chip's reset (0.1 by default) on the model (board/gameboy_tb.vhd,
# BLOCKS), from build/fastboot.bin, and writes the trace to
# build/blocks/<ROM>.txt: a line at the end of every phase, from the CPU's
# first access on. TEST ends the run at a test ROM's result, as in
# compare-ceres.sh. FROM=power-on traces from the power-on instead (the
# clocks' reset and wait), into build/blocks/power-on/<ROM>.txt, which the
# replay takes for the clocks only. Then
#
#   cargo test -p ceres-netlist --release --test replay -- --ignored
#
# replays every trace in build/blocks through ceres-netlist's NetTimer,
# NetIf, NetDma, NetLcd, NetScan, NetOamLatches, NetClocks, NetRender and
# NetChip (its tests/replay.rs) and reports the phases where one parts from
# the model (REPLAY_ONLY=a,b: only the traces whose names contain a or b).

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
BOOTROM=$HDL/build/fastboot.bin

[ $# -ge 1 ] || { echo "usage: $0 <ROM> [SECS]" >&2; exit 2; }
rom=$1
secs=${2:-0.1}
from=${FROM:-access}
case $from in
access) blocks=$HDL/build/blocks; power_on=false ;;
power-on) blocks=$HDL/build/blocks/power-on; power_on=true ;;
*) echo "FROM is access or power-on" >&2; exit 2 ;;
esac
out=$blocks/$(basename "${rom%.*}").txt
dir=$blocks/run-$(basename "${rom%.*}")
mkdir -p "$dir"

make -C "$HDL" -s model build/fastboot.bin

# A run that goes wrong stops at 1 GiB (in a POSIX shell's 512-byte blocks).
ulimit -f 2097152
limit=$(awk -v s="$secs" 'BEGIN { print int(600 + 1200 * s) }')
(cd "$dir" && timeout "$limit" nvc --std=2008 -H 64m --work="${WORK:-$HDL/build/model/work}" -e --jit gameboy_tb \
	-gBOOTROM="$BOOTROM" -gROM="$ROOT/$rom" -gSECS="$secs" -gBLOCKS="$out" ${TEST:+"-gTEST=$TEST"} \
	-gBLOCKS_FROM_POWER_ON=$power_on \
	-r >model.log 2>&1)
echo "$(wc -l <"$out") phases in $out"
