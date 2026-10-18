#!/bin/sh
# Runs test ROMs on the oracle (dmg-sim on Icarus Verilog), several at a
# time.
#
#   run-tests.sh [PATTERN...]
#
# The tests are the lines of tests.txt (or those that contain one of the
# PATTERNs):
#
#   <kind> <ROM, relative to the repository> <SECS>
#
# where <kind> is how the ROM reports its result (mooneye, gbmicrotest or
# blargg, see oracle/probe_results.sv) and SECS the time limit after the
# chip's reset. Each test runs in build/runs/<ROM>/ and leaves there
# result.txt, log.txt and serial.txt; build/results.txt lists them all. The
# boot ROM is dmg-sim's quickboot, which skips the logo but leaves DIV, the
# PPU and the APU as the real boot ROM does. JOBS sets how many run at a
# time. With PHASES=1, each test also leaves phases.txt, the signals that
# the readable model is checked against (oracle/probe_phases.sv); with
# BOOT=fast, the boot ROM is build/fastboot.bin instead, which reaches the
# cartridge right after the reset, and the tests run in build/runs-fast/
# (their results do not count: the boot ROM's timing is missing).

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
JOBS=${JOBS:-6}
PHASES=${PHASES:-0}
QUICKBOOT=$ROOT/external/reference-implementations/dmg-sim/boot/quickboot.bin
RUNS=$HDL/build/runs
RESULTS=$HDL/build/results
if [ "${BOOT:-}" = fast ]; then
	QUICKBOOT=$HDL/build/fastboot.bin
	RUNS=$HDL/build/runs-fast
	RESULTS=$HDL/build/results-fast
fi
VVP=$HDL/build/dmg-sim/dmg_cpu_b_gameboy.vvp

# One test (the script calls itself for each, in parallel).
if [ "${1:-}" = --one ]; then
	kind=$2 rom=$3 secs=$4
	dir=$RUNS/${rom%.gb}
	mkdir -p "$dir"
	rm -f "$dir/result.txt"
	phases=
	[ "$PHASES" = 1 ] && phases=+PHASES_FILE=phases.txt
	(cd "$dir" && vvp -N "$VVP" -none +BOOTROM="$QUICKBOOT" +ROM="$ROOT/$rom" \
		+SECS="$secs" +TEST="$kind" +RESULT_FILE=result.txt +SERIAL_FILE=serial.txt $phases \
		>log.txt 2>&1) || true
	result=$(cat "$dir/result.txt" 2>/dev/null || echo "error (see log.txt)")
	echo "$rom $result"
	exit 0
fi

make -C "$HDL" >/dev/null

# grep -e PATTERN for each, or -e '' for all.
[ $# -gt 0 ] || set -- ''
for p; do set -- "$@" -e "$p"; shift; done
grep -v '^#' "$HDL/tests.txt" | grep -v '^[[:space:]]*$' | grep -F "$@" |
	while read -r kind rom secs; do
		printf '%s\0%s\0%s\0' "$kind" "$rom" "$secs"
	done |
	xargs -0 -n 3 -P "$JOBS" "$0" --one |
	tee "$RESULTS.new"
sort "$RESULTS.new" >"$RESULTS.txt"
rm "$RESULTS.new"
echo "passed: $(grep -c ' pass' "$RESULTS.txt" || true) of $(wc -l <"$RESULTS.txt")"
