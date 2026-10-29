#!/bin/sh
# The knowledge base: what the repository knows about the hardware, the
# readable model and ceres, as tables built from the sources, and the
# checks and queries on them, in Datalog (knowledge/knowledge.dl, run by
# Soufflé; it says what each table holds).
#
#   knowledge.sh                build the tables, build/knowledge/*.tsv
#   knowledge.sh check          build them, and run the checks: relations of
#                               knowledge/knowledge.dl, each of which must
#                               be empty (exits 1 if one is not)
#   knowledge.sh NAME           build them, and print the query NAME
#   knowledge.sh list           the queries and the checks, with what each
#                               finds
#   knowledge.sh run FILE       build them, and run the rules of FILE (.dl)
#                               with knowledge/knowledge.dl's: what FILE
#                               outputs to stdout (`.output r(IO=stdout)`)
#
# The sources: rtl/ and board/ (knowledge/rtl.awk, oracle/controls.awk),
# doc/facts.toml, hdl/races.toml, hdl/rules.toml and doc/assumptions.toml
# with the `@fact`, `@rule`, `@assumes` and `@discharges` tags (the example
# facts of ceres-test-runner), and the reports of find-runts.sh in
# build/runts/reports. MODELS are the chips to look at, as doc/facts.toml
# names them: by default all that rtl/ has (dmg cgb cgb-c cgb-e), which the
# checks need; MODELS="cgb cgb-c cgb-e" ./knowledge.sh borrowed-rules lists
# what the CGB's model takes from the DMG's.

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
MODELS=${MODELS:-dmg cgb cgb-c cgb-e}
TABLES=$HDL/build/knowledge
PROGRAM=$HDL/knowledge/knowledge.dl
TAB=$(printf '\t')

build() {
	mkdir -p "$TABLES"
	cd "$HDL"
	awk -v TABLE=instance -f knowledge/rtl.awk rtl/*.vhd board/*.vhd >"$TABLES/instance.tsv"
	awk -f oracle/controls.awk rtl/*.vhd | sort -u |
		awk '{ print $1 "\t" $2 "\t" $3 }' >"$TABLES/control.tsv"
	awk -v TABLE=delay -f knowledge/rtl.awk rtl/*.vhd >"$TABLES/delay.tsv"
	awk -v TABLE=signal -f knowledge/rtl.awk rtl/*.vhd | sort -u >"$TABLES/signal.tsv"
	awk -v TABLE=hw -f knowledge/rtl.awk rtl/*.vhd >"$TABLES/hw_tag.tsv"
	awk -v TABLE=rule -f knowledge/rtl.awk rtl/*.vhd >"$TABLES/rule_tag.tsv"
	for table in port portmap flow anchor; do
		awk -v TABLE=$table -f knowledge/rtl.awk rtl/*.vhd >"$TABLES/$table.tsv"
	done
	mkdir -p build/runts/reports
	find build/runts/reports -name '*.txt' -exec awk -f oracle/runts.awk "$TABLES/instance.tsv" {} + |
		awk -F '\t' -v OFS='\t' '{ $6 = $6 / 1e6; $7 = $7 / 1e12; print }' >"$TABLES/runt.tsv"
	cargo run -q --manifest-path "$ROOT/Cargo.toml" -p ceres-test-runner --example facts -- tsv "$TABLES"
	printf '%s\n' $MODELS >"$TABLES/tree_model.tsv"
}

# Runs the program on the tables, its relations into build/knowledge-dl.
datalog() {
	command -v souffle >/dev/null || { echo "knowledge.sh needs Soufflé (souffle)" >&2; exit 2; }
	out=$HDL/build/knowledge-dl
	rm -rf "$out"
	mkdir -p "$out"
	souffle -F "$TABLES" -D "$out" "$1"
}

# The checks' or the queries' facts of the program:
# RELATION("NAME",\n "TITLE"), as NAME<tab>TITLE.
dl_list() {
	sed -n "/^$1(\"/{N;s/^$1(\"\([^\"]*\)\",\n *\"\(.*\)\")\.\$/\1\t\2/p}" "$PROGRAM"
}

# Prints a relation's file: its header, then its rows in order. A first
# column named rank orders them first (numerically), and is not printed.
show() {
	if [ "$(head -n 1 "$1" | cut -f 1)" = rank ]; then
		{ head -n 1 "$1"; tail -n +2 "$1" | LC_ALL=C sort -t "$TAB" -k1,1n -k2; } | cut -f 2-
	else
		{ head -n 1 "$1"; tail -n +2 "$1" | LC_ALL=C sort; }
	fi | column -t -s "$TAB"
}

case ${1:-} in
"")
	build
	;;
list)
	dl_list query | while IFS="$TAB" read -r name title; do
		printf '%-40s %s\n' "queries/$name" "$title"
	done
	dl_list check | while IFS="$TAB" read -r name title; do
		printf '%-40s %s\n' "checks/$name" "$title"
	done
	;;
check)
	build
	datalog "$PROGRAM"
	failed=0
	while IFS="$TAB" read -r name title; do
		# A header, then a row for each thing the check finds.
		if [ "$(wc -l <"$out/$name.tsv")" -gt 1 ]; then
			echo "check $name: $title"
			show "$out/$name.tsv"
			echo
			failed=1
		fi
	done <"$out/checks.tsv"
	exit $failed
	;;
run)
	[ $# -eq 2 ] || { echo "usage: $0 run FILE.dl" >&2; exit 2; }
	file=$(cd -- "$(dirname -- "$2")" && pwd)/$(basename -- "$2")
	build
	cat "$PROGRAM" "$file" >"$HDL/build/knowledge-run.dl"
	datalog "$HDL/build/knowledge-run.dl"
	;;
*)
	title=$(dl_list query | awk -F "$TAB" -v n="$1" '$1 == n { print $2; found = 1 } END { exit !found }') ||
		{ echo "no query $1 (knowledge.sh list)" >&2; exit 2; }
	build
	datalog "$PROGRAM"
	echo "$title"
	show "$out/$1.tsv"
	;;
esac
