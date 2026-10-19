#!/bin/sh
# Compares ceres with the readable model of the chip: the CPU's memory
# accesses, M-cycle by M-cycle, from power-on.
#
#   compare-ceres.sh <ROM> [SECS]
#
# runs the ROM (a path relative to the repository) for SECS seconds after
# the chip's reset (0.1 by default) on the model (board/gameboy_tb.vhd,
# BUS_TRACE) and on ceres (its example bus_trace), writes both traces to
# build/compare/<ROM>/model.txt and ceres.txt, and prints where they first
# differ. A line is an access: the M-cycle, counted from the CPU's first
# access, R or W, the address and the byte.
#
# The boot ROM is build/fastboot.bin, which turns the LCD on and jumps to
# the cartridge right after the reset: both start from the same state,
# which is all the comparison needs, without the 150 ms of the real boot.
# (VRAM and OAM then keep what they power up with: arbitrary bytes on the
# model's board, FF in the model's OAM, zeros in ceres. A ROM that reads
# them before it writes them differs there.)
# BOOT=quick runs dmg-sim's quickboot instead, which leaves the machine as
# the real boot ROM does (for the tests of that state, such as gbmicrotest's
# poweron_* and Mooneye's boot_*). TEST=gbmicrotest (or mooneye, blargg)
# ends the run when the test reports its result, usually within a frame.

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
BOOTROM=$HDL/build/fastboot.bin
[ "${BOOT:-fast}" = quick ] && BOOTROM=$ROOT/external/reference-implementations/dmg-sim/boot/quickboot.bin

[ $# -ge 1 ] || { echo "usage: $0 <ROM> [SECS]" >&2; exit 2; }
rom=$1
secs=${2:-0.1}
dir=$HDL/build/compare/${rom%.gb}
mkdir -p "$dir"

make -C "$HDL" -s model build/fastboot.bin
cargo build -q --release --manifest-path "$ROOT/Cargo.toml" -p ceres-test-runner --example bus_trace

(cd "$dir" && nvc --std=2008 -H 64m --work="$HDL/build/model/work" -e --jit gameboy_tb \
	-gBOOTROM="$BOOTROM" -gROM="$ROOT/$rom" -gSECS="$secs" -gTEST="${TEST:-}" -gBUS_TRACE="$dir/model.txt" -r \
	>model.log 2>&1)

# The same M-cycles from ceres: up to the model's last, which may be cut.
mcycles=$(tail -n 1 "$dir/model.txt" | cut -d ' ' -f 1)
sed -i "/^$mcycles /,\$d" "$dir/model.txt"
"$ROOT/target/release/examples/bus_trace" "$ROOT/$rom" --bootrom "$BOOTROM" \
	--mcycles "$mcycles" --out "$dir/ceres.txt"

echo "$rom: $(wc -l <"$dir/model.txt") accesses in $mcycles M-cycles"
if cmp -s "$dir/ceres.txt" "$dir/model.txt"; then
	echo "same"
else
	# The first difference, with the accesses before it.
	diff -U 8 "$dir/ceres.txt" "$dir/model.txt" | awk '/^@@/ { n++ } n == 1' | head -n 40
	exit 1
fi
