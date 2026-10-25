#!/bin/sh
# Compares ceres with the readable model on many ROMs: the accesses and what
# the chip's blocks show each other (compare-ceres.sh with CUTS=1 and
# NOFRAMES=1), JOBS at a time (6):
#
#   sweep-cuts.sh <NAME> < roms.txt
#
# reads the ROMs' paths (relative to the repository) from standard input,
# runs each for SECS seconds (0.1) with the environment's BOOT, TEST and APU,
# and writes build/sweeps/<NAME>.txt: a line per ROM, its path, "same" or
# "busdiff" for the accesses ("timeout" or "toolarge" when compare-ceres.sh
# stopped a side), and "cuts" or the names whose changes differ
# (compare-ceres.sh's output is in build/compare/<ROM>/compare.txt). Then it
# prints how many ROMs are which, the differences of the cuts grouped across
# ROMs (with up to four ROMs each; the pairs an M-cycle off and the brief
# values left out), and how many ROMs have which difference for the DMA's
# start and end, which a ROM has only once each.

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
[ $# -eq 1 ] || { echo "usage: $0 <NAME> < roms.txt" >&2; exit 2; }
out=$HDL/build/sweeps/$1.txt
mkdir -p "$HDL/build/sweeps"

# Build once, not in every job.
make -C "$HDL" -s model build/fastboot.bin build/tinyboot.bin
cargo build -q --release --manifest-path "$ROOT/Cargo.toml" -p ceres-test-runner --example bus_trace

cd "$HDL"
export CUTS=1 NOFRAMES=1
xargs -P "${JOBS:-6}" -I{} sh -c '
	rom=$1
	dir=build/compare/${rom%.*}
	mkdir -p "$dir"
	./compare-ceres.sh "$rom" "${SECS:-0.1}" >"$dir/compare.txt" 2>&1 || true
	bus=busdiff
	grep -qx same "$dir/compare.txt" && bus=same
	grep -q "^timed out: " "$dir/compare.txt" && bus=timeout
	grep -q "^too large: " "$dir/compare.txt" && bus=toolarge
	cuts=$(grep -E "^[A-Z]+ +model .* not paired, " "$dir/compare.txt" | awk "{print \$1}" |
		tr "\n" , | sed "s/,\$//")
	echo "$rom $bus ${cuts:-cuts}"
' sh {} >"$out"
sort -o "$out" "$out"

# The runs that were stopped first: their traces are incomplete.
awk '$2 == "timeout" || $2 == "toolarge" { print $2 ": " $1 }' "$out"
echo "$(wc -l <"$out") ROMs; accesses: $(awk '{print $2}' "$out" | sort | uniq -c | tr -s ' ' | tr '\n' ' ')"
echo "cuts the same in $(awk '$3 == "cuts"' "$out" | wc -l)"
echo
echo "The differences of the cuts, grouped:"
awk '{print $1}' "$out" | while read -r rom; do
	r=${rom%.*}
	awk -v r="${r##*/}" '
		/^[A-Z]+ +model/ { name = $1 }
		/^     / && !/for an M-cycle|\.\.\./ && !/\(\+1\)$|\(-1\)$/ {
			note = $0
			sub(/^ +/, "", note)
			gsub(/ at [0-9]+/, " at N", note)
			gsub(/model [0-9A-F]+ at N, ceres [0-9A-F]+ at N/, "model X, ceres Y", note)
			print name ": " note "\t" r
		}
	' "build/compare/$r/compare.txt"
done | awk -F '\t' '
	{ c[$1]++; if (c[$1] <= 4) ex[$1] = ex[$1] " " $2 }
	END { for (k in c) print c[k] "x " k " |" ex[k] }
' | sort -rn
echo
echo "The DMA's start and end, model minus ceres, in how many ROMs:"
awk '{print $1}' "$out" | while read -r rom; do
	r=${rom%.*}
	sed -n 's/^DMA .*usually \(.*\) M-cycles apart.*/\1/p' "build/compare/$r/compare.txt"
done | sort | uniq -c | sort -rn
