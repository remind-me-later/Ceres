#!/bin/sh
# Lists the pulses of the readable model too short for the die to decide
# by anything but its physics, at the inputs that control storage.
#
#   find-runts.sh <ROM> [SECS]
#
# runs the ROM (a path relative to the repository) for SECS seconds after
# the chip's reset (0.1 by default) on the model, from build/fastboot.bin,
# with the VHPI plugin oracle/runts.c, which notes every pulse of a one-bit
# signal, or of a bit of a vector, shorter than RUNT_WIDTH ns (60, half a
# phase), including those of no width (a delta cycle). It writes them to
# build/runts/reports/<ROM>.txt (the ROM's path under external/test-roms,
# without its extension; suites reuse names) and prints the ones at a
# signal that clocks a flip-flop ("edge") or that a latch, an asynchronous
# set or reset, or a process with no clock tests ("level"), as
# oracle/controls.awk finds them in rtl/: the kind, the number of pulses,
# the shortest in ns, the entity and signal, and the first pulse's time in
# ms. TEST ends the run at a test ROM's result, as in compare-ceres.sh;
# BOOT=quick runs dmg-sim's quickboot, BOOT=tiny build/tinyboot.bin with the
# VRAM zeroed (as run-gambatte.sh).
#
# A flip-flop that a pulse clocks, or a latch it opens, takes it or not by
# the widths of the paths that make it: there the model's outcome rests on
# its delays (the `after` clauses of rtl/), not only on their order.
#
#   find-runts.sh -s
#
# prints the pulses at controls of every report in build/runts/reports
# together, for a sweep (xargs -P, as with compare-ceres.sh). The knowledge
# database (knowledge.sh) holds them too, with the verdicts of races.toml.

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
OUT=$HDL/build/runts

# The instances and the controls of rtl/ (written whole, for the runs of a
# sweep that read them meanwhile).
tables() {
	(cd "$HDL" && awk -v TABLE=instance -f knowledge/rtl.awk rtl/*.vhd board/*.vhd) >"$OUT/instances.tsv.$$"
	(cd "$HDL" && awk -f oracle/controls.awk rtl/*.vhd) | sort -u >"$OUT/controls.txt.$$"
	mv "$OUT/instances.tsv.$$" "$OUT/instances.tsv"
	mv "$OUT/controls.txt.$$" "$OUT/controls.txt"
}

at_controls() {
	awk -f "$HDL/oracle/runts.awk" "$OUT/instances.tsv" "$@" |
		awk -f "$HDL/oracle/runts-at-controls.awk" "$OUT/controls.txt" - |
		sort -k4,4 -k1,1
}

mkdir -p "$OUT/reports"
if [ "${1:-}" = -s ]; then
	tables
	find "$OUT/reports" -name '*.txt' -exec awk -f "$HDL/oracle/runts.awk" "$OUT/instances.tsv" {} + |
		awk -f "$HDL/oracle/runts-at-controls.awk" "$OUT/controls.txt" - |
		sort -k4,4 -k1,1
	exit
fi

[ $# -ge 1 ] || { echo "usage: $0 <ROM> [SECS] | -s" >&2; exit 2; }
rom=$1
secs=${2:-0.1}
name=${rom#external/test-roms/}
name=${name%.*}
dir=$OUT/runs/$name
mkdir -p "$dir" "$(dirname "$OUT/reports/$name")"
vram_zero=false
case ${BOOT:-fast} in
fast) bootrom=$HDL/build/fastboot.bin ;;
quick) bootrom=$ROOT/external/reference-implementations/dmg-sim/boot/quickboot.bin ;;
tiny) bootrom=$HDL/build/tinyboot.bin vram_zero=true ;;
*) echo "BOOT is fast, quick or tiny" >&2; exit 2 ;;
esac

make -C "$HDL" -s model build/fastboot.bin build/tinyboot.bin build/runts.so
tables

limit=$(awk -v s="$secs" 'BEGIN { print int(600 + 1200 * s) }')
(cd "$dir" && RUNT_FILE=$OUT/reports/$name.txt timeout "$limit" \
	nvc --std=2008 -H 64m --work="${WORK:-$HDL/build/model/work}" -e --jit gameboy_tb \
	-gBOOTROM="$bootrom" -gVRAM_ZERO="$vram_zero" -gROM="$ROOT/$rom" -gSECS="$secs" ${TEST:+"-gTEST=$TEST"} \
	-r --load="$HDL/build/runts.so" >model.log 2>&1)
at_controls "$OUT/reports/$name.txt"
