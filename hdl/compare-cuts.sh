#!/bin/sh
# Compares what the blocks of the chip show each other on the model and on
# ceres (compare-ceres.sh, CUTS=1):
#
#   compare-cuts.sh <model-cuts.txt> <ceres-cuts.txt> [<model.txt>]
#
# A line of a file is a change: "<M-cycle> <name> <value>", the name IF,
# TIMA, DMA, LY, MODE, OAMB, VRAMB or DIV (see board/gameboy_tb.vhd, CUTS). A
# ceres line can have a fourth field: ceres sees the state at the end of each
# step of its time, and a step can be longer than an M-cycle (an
# instruction's internal ones); the field is the step's first M-cycle, where
# the change may have been as well.
#
# For each name, the two files' changes are paired in time: a change pairs
# with the other side's next one if their values are the same (DIV's do not
# count: they are the divider where it jumped, which depends on where in an
# M-cycle a side looks) and they are at most WINDOW M-cycles apart (3);
# otherwise the earlier one is a change that only its side has. A change
# only one side has that its next change replaces within an M-cycle is
# counted apart, as brief: the other side samples once an M-cycle and can
# miss it (a count of TIMA that a write overrides); so is the next change,
# if it takes the brief value back.
#
# The two sides see a change at different points of an M-cycle, so the M-cycles
# of a pair may differ by a constant; the usual difference (model minus ceres)
# is found for each kind of change apart: for IF, DMA, MODE, OAMB and VRAMB
# each value (a mode can begin at another point of an M-cycle than the next);
# for TIMA and LY a count (+1) and a change that sets them (a reload, line
# 153's LY 0); and, given the model's bus trace (model.txt), a change in the
# M-cycle of a write to its register or in the next (TIMA, IF, DIV, FF46 for
# DMA and OAMB, LCDC for LY, MODE, OAMB and VRAMB). The first line of a name,
# the state the trace starts with, does not count. A kind with a single
# change (a DMA's start) has that change's difference as its usual one: it
# can only be judged against other ROMs'.
#
# Prints, for each name, how many changes each side has, the usual difference
# of each kind (and the others, with how many changes have them), and the
# changes only one side has and the pairs off the usual difference, those an
# M-cycle off counted apart (a change can fall between the two sides' points
# of sampling). Exits 1 if a change has no pair or a pair is more than an
# M-cycle off.

set -eu

[ $# -ge 2 ] || { echo "usage: $0 <model-cuts.txt> <ceres-cuts.txt> [<model.txt>]" >&2; exit 2; }

awk -v window="${WINDOW:-3}" '
	# The kind of the model'"'"'s change i: a write; for IF, DMA, MODE, OAMB and
	# VRAMB its value; for TIMA and LY whether it counts on (+1) or is set.
	function kind(name, i,    v, t, before) {
		v = value[1, name, i]
		t = at[1, name, i]
		if ((name, t) in written || (name, t - 1) in written) return "write"
		if (name == "IF" || name == "DMA" || name == "MODE" || name == "OAMB" || name == "VRAMB") return name "=" v
		if (name == "TIMA" || name == "LY") {
			before = value[1, name, i - 1]
			return (hex(before) + 1) % 256 == hex(v) ? "+1" : "set"
		}
		return name
	}
	function hex(s,    i, d) {
		d = 0
		for (i = 1; i <= length(s); i++) d = d * 16 + index("0123456789ABCDEF", substr(s, i, 1)) - 1
		return d
	}
	function pairs_with(name, i, j,    t) {
		t = at[1, name, i]
		return (name == "DIV" || value[1, name, i] == value[2, name, j]) &&
			t >= from[2, name, j] - window && t <= at[2, name, j] + window
	}
	# How far pair p is off the usual difference of its kind: 0 if the model'"'"'s
	# M-cycle, less the usual difference, is in the step ceres saw it in.
	function off_by(name, p,    t) {
		t = at[1, name, pm[p]] - best[kind(name, pm[p])]
		if (t < from[2, name, pc[p]]) return t - from[2, name, pc[p]]
		if (t > at[2, name, pc[p]]) return t - at[2, name, pc[p]]
		return 0
	}
	# A change only side s has (index k), brief or not. Returns how many of
	# the side'"'"'s changes it covers: two for a brief value that the next
	# change takes back.
	function alone(s, name, k, last) {
		if (k < last && at[s, name, k + 1] - at[s, name, k] <= 1) {
			brief++
			briefs[brief] = sprintf("%s only, for an M-cycle: %s at %d", s == 1 ? "model" : "ceres", \
				value[s, name, k], at[s, name, k])
			return k > 1 && value[s, name, k + 1] == value[s, name, k - 1] ? 2 : 1
		} else {
			unpaired++
			misses[unpaired] = sprintf("%s only: %s at %d", s == 1 ? "model" : "ceres", \
				value[s, name, k], at[s, name, k])
			return 1
		}
	}
	function note(text) {
		if (shown < 8) print "     " text
		shown++
	}
	FNR == 1 { side++ }
	# The bus trace: the CPU'"'"'s writes to the registers that set a name.
	side == 3 {
		if ($2 == "W") {
			if ($3 == "FF05") written["TIMA", $1] = 1
			else if ($3 == "FF0F") written["IF", $1] = 1
			else if ($3 == "FF04") written["DIV", $1] = 1
			else if ($3 == "FF46") { written["DMA", $1] = 1; written["OAMB", $1] = 1 }
			else if ($3 == "FF40") {
				written["LY", $1] = 1
				written["MODE", $1] = 1
				written["OAMB", $1] = 1
				written["VRAMB", $1] = 1
			}
		}
		next
	}
	{
		name = $2
		names[name] = 1
		n[side, name]++
		at[side, name, n[side, name]] = $1
		value[side, name, n[side, name]] = $3
		from[side, name, n[side, name]] = NF > 3 ? $4 : $1
	}
	END {
		status = 0
		for (name in names) {
			m = n[1, name] + 0
			c = n[2, name] + 0
			pairs = 0
			unpaired = 0
			brief = 0
			delete pm
			delete pc
			i = 1
			j = 1
			while (i <= m && j <= c) {
				if (pairs_with(name, i, j)) {
					pairs++
					pm[pairs] = i
					pc[pairs] = j
					i++
					j++
				} else if (at[1, name, i] < from[2, name, j] || \
					(at[1, name, i] <= at[2, name, j] && value[1, name, i] != value[2, name, j])) {
					i += alone(1, name, i, m)
				} else {
					j += alone(2, name, j, c)
				}
			}
			# The usual difference of each kind of change, and how often each
			# difference comes.
			delete count
			delete best
			delete kinds
			delete diffs
			for (p = 2; p <= pairs; p++) {
				k = kind(name, pm[p])
				d = at[1, name, pm[p]] - at[2, name, pc[p]]
				kinds[k] = 1
				diffs[d] = 1
				count[k, d]++
				if (!(k in best) || count[k, d] > count[k, best[k]]) best[k] = d
			}
			usual = ""
			for (k in kinds) {
				usual = usual (usual == "" ? "" : "; ") (k == name ? "" : k ": ") best[k]
				for (d in diffs)
					if (d != best[k] && count[k, d] > 0) usual = usual ", " d " (" count[k, d] "x)"
			}
			near = 0
			far = 0
			for (p = 2; p <= pairs; p++) {
				o = off_by(name, p)
				if (o == 1 || o == -1) near++
				else if (o != 0) far++
			}
			printf "%-4s model %d, ceres %d changes; usually %s M-cycles apart", name, m, c, usual == "" ? "?" : usual
			if (brief > 0) printf "; %d brief", brief
			if (near > 0) printf "; %d an M-cycle off", near
			if (unpaired == 0 && far == 0) {
				print near == 0 ? "; the same" : ""
			} else {
				status = 1
				printf "; %d not paired, %d more off\n", unpaired, far
			}
			shown = 0
			for (u = 1; u <= unpaired; u++) note(misses[u])
			for (p = 2; p <= pairs; p++) {
				o = off_by(name, p)
				if (o > 1 || o < -1)
					note(sprintf("%s: model at %d, ceres at %d (%+d)", value[1, name, pm[p]], \
						at[1, name, pm[p]], at[2, name, pc[p]], o))
			}
			if (shown > 8) print "     ..."
			shown = 0
			for (p = 2; p <= pairs; p++) {
				o = off_by(name, p)
				if ((o == 1 || o == -1) && shown < 3) {
					print "     " sprintf("%s: model at %d, ceres at %d (%+d)", value[1, name, pm[p]], \
						at[1, name, pm[p]], at[2, name, pc[p]], o)
					shown++
				}
			}
			for (u = 1; u <= brief && u <= 3; u++) print "     " briefs[u]
		}
		exit status
	}
' "$1" "$2" ${3:+"$3"}
