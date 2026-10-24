#!/bin/sh
# Compares what the blocks of the chip show each other on the model and on
# ceres (compare-ceres.sh, CUTS=1):
#
#   compare-cuts.sh <model-cuts.txt> <ceres-cuts.txt>
#
# A line of a file is a change: "<M-cycle> <name> <value>", the name IF,
# TIMA, DMA, LY, MODE or DIV (see board/gameboy_tb.vhd, CUTS). For each name,
# the two files' changes are paired in order, by value: where one side has a
# change the other has not (up to three in a row), it is reported and the
# pairing goes on after it; where the values differ, both are reported. (A
# DIV change's value is the divider where it jumped, which depends on where
# in an M-cycle a side looks: DIV changes pair by order only.) The M-cycles
# of a pair may differ by a constant, since the two sides see a change at
# different points of an M-cycle, so the usual difference (model minus ceres)
# is found first, for each kind of change apart: for IF, DMA and MODE each
# value (a mode can begin at another point of an M-cycle than the next), for
# TIMA and LY a count (+1) and a change that sets them (a write, a reload).
# The first line of a name, the state the trace starts with, does not count.
# Prints, for each name, how many changes each side has, the usual
# difference of each kind (and the other differences, with how many changes
# have them), and the first changes that do not pair or are off the usual
# difference. Exits 1 if any name differs.

set -eu

[ $# -eq 2 ] || { echo "usage: $0 <model-cuts.txt> <ceres-cuts.txt>" >&2; exit 2; }

awk '
	# The kind of the model'"'"'s change i: for IF, DMA and MODE its value; for
	# TIMA and LY whether it counts on (+1) or is set (a write, a reload, line
	# 153'"'"'s LY 0).
	function kind(name, i,    v, before) {
		v = value[1, name, i]
		if (name == "IF" || name == "DMA" || name == "MODE") return name "=" v
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
	function same(name, i, j) {
		return name == "DIV" || value[1, name, i] == value[2, name, j]
	}
	function note(text) {
		if (shown < 8) print "     " text
		shown++
	}
	FNR == 1 { side++ }
	{
		name = $2
		names[name] = 1
		n[side, name]++
		at[side, name, n[side, name]] = $1
		value[side, name, n[side, name]] = $3
	}
	END {
		status = 0
		for (name in names) {
			m = n[1, name] + 0
			c = n[2, name] + 0
			# Pair the changes, skipping those only one side has.
			pairs = 0
			unpaired = 0
			delete pm
			delete pc
			i = 1
			j = 1
			while (i <= m && j <= c) {
				if (same(name, i, j)) {
					pairs++
					pm[pairs] = i
					pc[pairs] = j
					i++
					j++
					continue
				}
				skip = 0
				for (k = 1; k <= 3 && !skip; k++) {
					if (j + k <= c && same(name, i, j + k)) skip = -k
					else if (i + k <= m && same(name, i + k, j)) skip = k
				}
				unpaired++
				if (skip < 0) {
					misses[unpaired] = sprintf("ceres only: %s at %d", value[2, name, j], at[2, name, j])
					j -= skip
				} else if (skip > 0) {
					misses[unpaired] = sprintf("model only: %s at %d", value[1, name, i], at[1, name, i])
					i += skip
				} else {
					misses[unpaired] = sprintf("model %s at %d, ceres %s at %d", \
						value[1, name, i], at[1, name, i], value[2, name, j], at[2, name, j])
					i++
					j++
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
			off = 0
			for (p = 2; p <= pairs; p++) {
				d = at[1, name, pm[p]] - at[2, name, pc[p]]
				if (d != best[kind(name, pm[p])]) off++
			}
			printf "%-4s model %d, ceres %d changes; usually %s M-cycles apart", name, m, c, usual == "" ? "?" : usual
			if (unpaired == 0 && off == 0) {
				print "; the same"
				continue
			}
			status = 1
			printf "; %d not paired, %d paired off the usual difference\n", unpaired, off
			shown = 0
			for (u = 1; u <= unpaired; u++) note(misses[u])
			for (p = 2; p <= pairs; p++) {
				d = at[1, name, pm[p]] - at[2, name, pc[p]]
				u = best[kind(name, pm[p])]
				if (d != u)
					note(sprintf("%s: model at %d, ceres at %d (%+d)", value[1, name, pm[p]], \
						at[1, name, pm[p]], at[2, name, pc[p]], d - u))
			}
			if (shown > 8) print "     ..."
		}
		exit status
	}
' "$1" "$2"
