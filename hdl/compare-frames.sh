#!/bin/sh
# Compares the frames that the model and ceres wrote (compare-ceres.sh):
#
#   compare-frames.sh <model-frames.txt> <ceres-frames.txt>
#
# A line has 160 pixels of one character each (the DMG's shades) or of four
# (the CGB's 15-bit colours, in hexadecimal). Neither side writes the first
# frame after the LCD is turned on, which the LCD does not show; the frames
# of a single colour before the first picture are left out on both sides,
# since the number of them can differ. (A frame is complete on
# the model when its last line is latched, on ceres when STAT's mode becomes
# 1: ceres may have one more at the cut.)
#
# Prints how many frames each has, the first picture that differs, with its
# first pixels, and how many pictures differ, the last among them. Exits 1
# if any differs.

set -eu

[ $# -eq 2 ] || { echo "usage: $0 <model-frames.txt> <ceres-frames.txt>" >&2; exit 2; }

awk '
	FNR == 1 { file++ }
	/^frame / { frame[file] = $2; y = 0; if ($2 + 1 > count[file]) count[file] = $2 + 1; next }
	{
		w = length($0) / 160
		if (y == 0) colour = substr($0, 1, w)
		pix[file, frame[file], y++] = $0
		rest = $0
		gsub(colour, "", rest)
		if (rest != "") drawn[file, frame[file]] = 1
	}
	END {
		for (i = 1; i <= 2; i++) {
			first[i] = 0
			while (first[i] < count[i] && !drawn[i, first[i]]) first[i]++
		}
		printf "frames: %d on the model, %d on ceres; %d and %d of one colour at the start left out\n",
			count[1], count[2], first[1], first[2]
		shown = count[1] - first[1]
		other = count[2] - first[2]
		if (shown == 0 && other == 0) { print "no picture on either (SECS too short?)"; exit 0 }
		if (shown == 0) { print "no picture on the model, " other " on ceres"; exit 1 }
		if (other == 0) { print "no picture on ceres, " shown " on the model"; exit 1 }
		if (other < shown) {
			printf "ceres has fewer pictures (%d, the model %d): comparing %d\n", other, shown, other
			shown = other
			fewer = 1
		}
		differing = 0
		for (f = 0; f < shown; f++) {
			n = 0
			for (y = 0; y < 144; y++) {
				a = pix[1, f + first[1], y] ""; b = pix[2, f + first[2], y] ""   # as strings: awk compares digits as numbers
				if (a == b) continue
				w = length(a) / 160
				for (x = 0; x < 160; x++) {
					m = substr(a, x * w + 1, w); c = substr(b, x * w + 1, w)
					if (m == c) continue
					if (n++ < 8 && !differing)
						printf "picture %d: x=%d y=%d model %s ceres %s\n", f, x, y, m, c
				}
			}
			if (n) {
				if (!differing) printf "picture %d differs in %d pixels\n", f, n
				differing++
				last = f; last_n = n
			}
		}
		if (!differing) { printf "same pictures (%d)\n", shown; exit fewer }
		printf "%d of %d pictures differ; the last, %d, in %d pixels%s\n", differing, shown, last, last_n,
			last == shown - 1 ? "" : " (the picture at the end is the same)"
		exit 1
	}' "$1" "$2"
