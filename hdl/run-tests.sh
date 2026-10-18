#!/bin/sh
# Runs test ROMs on the gate-level model with nvc, several at a time.
#
#   run-tests.sh [PATTERN...]
#
# The tests are the lines of tests.txt (or those that contain one of the
# PATTERNs):
#
#   <kind> <ROM, relative to the repository> <time limit, e.g. 300ms>
#
# where <kind> is how the ROM reports its result (mooneye, gbmicrotest or
# blargg, see tb/gameboy_tb.vhd). Each test runs in build/runs/<ROM>/ and
# leaves there result.txt and log.txt; build/results.txt lists them all.
# The boot ROM is dmg-sim's quickboot, which skips the logo but leaves DIV
# as the real boot ROM does. JOBS sets how many run at a time (each takes
# about 650 MB).

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
JOBS=${JOBS:-6}
QUICKBOOT=$ROOT/external/reference-implementations/dmg-sim/boot/quickboot.bin
WORK=$HDL/build/nvc/work

# One test (the script calls itself for each, in parallel).
if [ "${1:-}" = --one ]; then
	kind=$2 rom=$3 limit=$4
	dir=$HDL/build/runs/${rom%.gb}
	mkdir -p "$dir"
	ln -sf "$QUICKBOOT" "$dir/bootrom.bin"
	rm -f "$dir/result.txt"
	(cd "$dir" && nvc --work="$WORK" -M 4g --std=2008 -e --no-save --jit gameboy_tb \
		-gSIM_TIME="$limit" -gROM_FILE="$ROOT/$rom" -gTEST="$kind" \
		-gTRACE_FILE=- -gRESULT_FILE=result.txt -gSERIAL_FILE=serial.txt -r \
		>log.txt 2>&1) || true
	result=$(cat "$dir/result.txt" 2>/dev/null || echo "error (see log.txt)")
	echo "$rom $result"
	exit 0
fi

make -C "$HDL" SIM=nvc >/dev/null

# grep -e PATTERN for each, or -e '' for all.
[ $# -gt 0 ] || set -- ''
for p; do set -- "$@" -e "$p"; shift; done
grep -v '^#' "$HDL/tests.txt" | grep -v '^[[:space:]]*$' | grep -F "$@" |
	while read -r kind rom limit; do
		printf '%s\0%s\0%s\0' "$kind" "$rom" "$limit"
	done |
	xargs -0 -n 3 -P "$JOBS" "$0" --one |
	tee "$HDL/build/results.new"
sort "$HDL/build/results.new" >"$HDL/build/results.txt"
rm "$HDL/build/results.new"
echo "passed: $(grep -c ' pass' "$HDL/build/results.txt" || true) of $(wc -l <"$HDL/build/results.txt")"
