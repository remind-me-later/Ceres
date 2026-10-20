#!/bin/sh
# Compares what the pixel pipeline did on the model and on ceres
# (compare-ceres.sh, PIXELS=1):
#
#   compare-pixels.sh <model-pixels.txt> <ceres-pixels.txt> [LINE]
#
# A line of a file is an event: "<line> P <x> <t>", a pixel pushed to the
# LCD (column x), or "<line> T <w|b> <t>", a tile fetch begun (the window's
# or the background's), or "<line> W <40|42|43|47|4B> <value> <t>", a write
# to LCDC, SCY, SCX, BGP or WX, t in half dots. A line of the picture is the events
# between two changes of the line number, and a frame starts at line 0.
# Times are counted from the line's first pixel (column 0), since the two
# clocks start a line at different places. The model sees a tile fetch begin
# half a dot before ceres does (its tile-map cycle starts a half dot
# sooner), which is added, and one more before the line (the fetch the line
# restarts), which is left out: only fetches from 16 half dots before the
# first pixel on are listed. A write that the same register's
# next write follows within a dot is left out: the model's bus is precharged
# (it shows FF first) and ceres writes the old value first. Prints, for the first lines
# (of LINE only, if given) where the pixels' gaps or the tile fetches
# differ, both timelines, and how many lines differ. Exits 1 if any does.

set -eu

[ $# -ge 2 ] || { echo "usage: $0 <model-pixels.txt> <ceres-pixels.txt> [LINE]" >&2; exit 2; }

awk -v only="${3:-}" '
	FNR == 1 { id++; rec = 0; prev = -1; frame = 0 }
	{
		line = $1
		if (line != prev) {
			rec++
			if (line == 0) frame++
			key[id, rec] = frame "/" line
			prev = line
			nt[id, rec] = 0
		}
		if ($2 == "P") { px[id, rec, $3] = $4; seen[id, rec, $3] = 1 }
		else if ($2 == "W") { nt[id, rec]++; tk[id, rec, nt[id, rec]] = "W" $3 "=" $4; tt[id, rec, nt[id, rec]] = $5 }
		else { nt[id, rec]++; tk[id, rec, nt[id, rec]] = $3; tt[id, rec, nt[id, rec]] = $4 }
		last[id] = rec
	}
	function timeline(i, r,    x, s, k, t0) {
		t0 = px[i, r, 0]
		s = "pixels:"
		for (x = 1; x < 160; x++)
			if (seen[i, r, x] && seen[i, r, x - 1] && px[i, r, x] - px[i, r, x - 1] != 2)
				s = s sprintf(" x%d+%d", x, px[i, r, x] - px[i, r, x - 1])
		s = s "  fetches:"
		for (k = 1; k <= nt[i, r]; k++) {
			t = tt[i, r, k] - t0 + (i == 1 ? 1 : 0)
			# A write is followed, within a dot, by the final value of the
			# same register: the bus of the model is precharged, and ceres
			# writes the old value first.
			superseded = 0
			if (substr(tk[i, r, k], 1, 1) == "W")
				for (j = k + 1; j <= nt[i, r] && tt[i, r, j] - tt[i, r, k] <= 2; j++)
					if (substr(tk[i, r, j], 1, 3) == substr(tk[i, r, k], 1, 3)) superseded = 1
			if (superseded) continue
			if (t >= -16) s = s sprintf(" %s@%d", tk[i, r, k], t)
		}
		return s
	}
	END {
		for (r = 1; r <= last[2]; r++) by[key[2, r]] = r
		for (r = 1; r <= last[1]; r++) {
			k = key[1, r]
			if (!(k in by) || !seen[1, r, 0]) continue
			c = by[k]
			if (!seen[2, c, 0]) continue
			split(k, kf, "/")
			if (only != "" && kf[2] != only) continue
			total++
			m = timeline(1, r); e = timeline(2, c)
			if (m != e) {
				bad++
				perframe[kf[1]]++
				if (bad <= 6 || verbose) printf "frame %d line %d\n  model: %s\n  ceres: %s\n", kf[1], kf[2], m, e
			}
		}
		if (bad) {
			printf "pixel timelines: %d of %d lines differ", bad, total
			n = 0
			for (f = 0; f <= 1000 && n < 6; f++)
				if (f in perframe) { printf "%s frame %d: %d", n ? "," : " (", f, perframe[f]; n++ }
			printf "%s\n", n ? ")" : ""
			exit 1
		}
		printf "pixel timelines: same in %d lines\n", total
	}' "$1" "$2"
