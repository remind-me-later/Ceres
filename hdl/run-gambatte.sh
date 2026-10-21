#!/bin/sh
# Runs Gambatte's test ROMs for the DMG on the readable model of the chip.
#
#   run-gambatte.sh [PATTERN...]
#
# A ROM's file name says what it shows on the screen on the hardware it was
# checked on: "..._dmg08_out3.gbc" shows the digit 3 (the name has other
# parts for other models; audio tests, which compare sound, are left out).
# The model runs each ROM, in build/gambatte/<ROM>/, from quickboot, which
# leaves the machine as the real boot ROM does, for SECS seconds from the
# chip's reset (0.42, which is 16 frames after the ROM starts, as ceres's
# gambatte_dmg test runs them), reads the digits in the top row of the last
# frame on the LCD with Gambatte's glyphs, and compares them. The results
# are in build/gambatte/results.txt: PASS or FAIL, the ROM, the expected
# digits and the digits read. Only the ROMs whose path contains one of the
# PATTERNs run, if there are any, and with STRIDE=n only every n-th of them
# (the 1747 ROMs take most of a day). JOBS sets how many run at a time (6).

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
GAMBATTE=external/test-roms/gambatte
OUT=$HDL/build/gambatte
QUICKBOOT=$ROOT/external/reference-implementations/dmg-sim/boot/quickboot.bin
SECS=${SECS:-0.42}

# One ROM (the script calls itself for each, in parallel).
if [ "${1:-}" = --one ]; then
	rom=$2
	stem=${rom##*/}
	stem=${stem%.*}
	case $stem in
	*dmg08_cgb04c_out*) key=dmg08_cgb04c_out ;;
	*dmg08_out*) key=dmg08_out ;;
	*) exit 0 ;;
	esac
	rest=${stem#*"$key"}
	case $rest in audio*) exit 0 ;; esac
	expected=$(printf '%s' "$rest" | sed -E 's/^([0-9a-fA-F]*).*/\1/' | tr 'a-f' 'A-F')
	[ -n "$expected" ] || exit 0
	dir=$OUT/${rom#"$GAMBATTE"/}
	dir=${dir%.*}
	mkdir -p "$dir"
	(cd "$dir" && nvc --std=2008 -H 64m --work="$HDL/build/model/work" -e --jit gameboy_tb \
		-gBOOTROM="$QUICKBOOT" -gROM="$ROOT/$rom" -gSECS="$SECS" -gFRAMES="$dir/frames.txt" -r \
		>log.txt 2>&1) || true
	got=$(awk -v n=${#expected} '
		BEGIN {
			# Gambatte'"'"'s glyphs, a row a byte, bit 7 the leftmost pixel.
			split("00 7F 41 41 41 41 41 7F  00 08 08 08 08 08 08 08  00 7F 01 01 7F 40 40 7F  00 7F 01 01 3F 01 01 7F  00 41 41 41 7F 01 01 01  00 7F 40 40 7E 01 01 7E  00 7F 40 40 7F 41 41 7F  00 7F 01 02 04 08 10 10  00 3E 41 41 3E 41 41 3E  00 7F 41 41 7F 01 01 7F  00 08 22 41 7F 41 41 41  00 7E 41 41 7E 41 41 7E  00 3E 41 40 40 40 41 3E  00 7E 41 41 41 41 41 7E  00 7F 40 40 7F 40 40 7F  00 7F 40 40 7F 40 40 40", g, " ")
			for (i = 1; i <= 128; i++) h[i] = index("0123456789ABCDEF", substr(g[i], 1, 1)) * 16 + index("0123456789ABCDEF", substr(g[i], 2, 1)) - 17
		}
		/^frame / { y = 0; next }
		{ row[y++] = $0 }
		END {
			out = ""
			for (c = 0; c < n; c++) {
				found = "?"
				for (d = 0; d < 16; d++) {
					ok = 1
					for (r = 0; r < 8 && ok; r++) {
						v = h[d * 8 + r + 1]
						for (x = 0; x < 8; x++) {
							black = int(v / (2 ^ (7 - x))) % 2
							px = substr(row[r], c * 8 + x + 1, 1)
							if ((black && px != "3") || (!black && px != "0")) { ok = 0; break }
						}
					}
					if (ok) { found = substr("0123456789ABCDEF", d + 1, 1); break }
				}
				out = out found
			}
			print out
		}' "$dir/frames.txt" 2>/dev/null || true)
	if [ "$got" = "$expected" ]; then
		echo "PASS ${rom#"$GAMBATTE"/} $expected $got"
	else
		echo "FAIL ${rom#"$GAMBATTE"/} $expected ${got:-none}"
	fi
	exit 0
fi

mkdir -p "$OUT"
make -C "$HDL" -s model >/dev/null
cd "$ROOT"
: >"$OUT/results.txt"
find "$GAMBATTE" \( -name '*.gb' -o -name '*.gbc' \) | sort |
	if [ $# -gt 0 ]; then grep -F "$(printf '%s\n' "$@")"; else cat; fi |
	awk -v s="${STRIDE:-1}" 'NR % s == 0 || s == 1' |
	xargs -P "${JOBS:-6}" -I{} "$HDL/run-gambatte.sh" --one {} | tee -a "$OUT/results.txt"
