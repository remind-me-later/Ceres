#!/bin/sh
# The knowledge database: what the repository knows about the hardware, the
# readable model and ceres, as SQLite tables to query (knowledge/schema.sql
# says what each holds), and the checks on it, in Datalog.
#
#   knowledge.sh                build build/knowledge.db from the sources
#   knowledge.sh check          build it, and run the checks: the relations of
#                               knowledge/knowledge.dl (Soufflé), each of
#                               which must be empty (exits 1 if one is not)
#   knowledge.sh NAME           build it, and run knowledge/queries/NAME.sql
#   knowledge.sh list           the queries and the checks, with what each
#                               finds
#
# The sources: rtl/ and board/ (knowledge/rtl.awk, oracle/controls.awk),
# doc/facts.toml, hdl/races.toml and hdl/rules.toml with the `@fact` and
# `@rule` tags (the example facts of ceres-test-runner), and the reports of
# find-runts.sh in build/runts/reports. MODELS are the chips to look at, as
# doc/facts.toml names them: by default all that rtl/ has (dmg cgb cgb-c
# cgb-e), which the checks need; MODELS="cgb cgb-c cgb-e" ./knowledge.sh
# borrowed-rules lists what the CGB's model takes from the DMG's.
# Open it with
# `sqlite3 build/knowledge.db` for queries of your own.

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
DB=$HDL/build/knowledge.db
MODELS=${MODELS:-dmg cgb cgb-c cgb-e}
TABLES=$HDL/build/knowledge

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
	mkdir -p build/runts/reports
	find build/runts/reports -name '*.txt' -exec awk -f oracle/runts.awk "$TABLES/instance.tsv" {} + |
		awk -F '\t' -v OFS='\t' '{ $6 = $6 / 1e6; $7 = $7 / 1e12; print }' >"$TABLES/runt.tsv"
	cargo run -q --manifest-path "$ROOT/Cargo.toml" -p ceres-test-runner --example facts -- tsv "$TABLES"
	printf '%s\n' $MODELS >"$TABLES/tree_model.tsv"

	rm -f "$DB.new"
	{
		cat knowledge/schema.sql
		echo ".mode tabs"
		for table in instance signal control delay hw_tag runt fact fact_model fact_test fact_open \
			fact_need fact_tag race race_model race_fact race_delay race_signal rule rule_model rule_fact rule_race rule_tag tree_model; do
			echo ".import $TABLES/$table.tsv $table"
		done
	} | sqlite3 "$DB.new"
	mv "$DB.new" "$DB"
}

# Runs a query file, with its first comment line as a title.
query() {
	sed -n '1s/^-- //p' "$1"
	sqlite3 -header -column "$DB" <"$1"
}

case ${1:-} in
"")
	build
	;;
list)
	for f in "$HDL"/knowledge/queries/*.sql; do
		printf '%-40s %s\n' "queries/$(basename "$f" .sql)" "$(sed -n '1s/^-- //p' "$f")"
	done
	# The checks' facts: check("NAME",\n "TITLE").
	sed -n '/^check("/{N;s/^check("\([^"]*\)",\n *"\(.*\)")\.$/\1\t\2/p}' "$HDL/knowledge/knowledge.dl" |
		while IFS='	' read -r name title; do
			printf '%-40s %s\n' "checks/$name" "$title"
		done
	;;
check)
	command -v souffle >/dev/null || { echo "knowledge.sh check needs Soufflé (souffle)" >&2; exit 2; }
	build
	out=$HDL/build/knowledge-checks
	rm -rf "$out"
	mkdir -p "$out"
	souffle -F "$TABLES" -D "$out" "$HDL/knowledge/knowledge.dl"
	failed=0
	while IFS='	' read -r name title; do
		# A header, then a row for each thing the check finds.
		if [ "$(wc -l <"$out/$name.tsv")" -gt 1 ]; then
			echo "check $name: $title"
			column -t -s '	' "$out/$name.tsv"
			echo
			failed=1
		fi
	done <"$out/checks.tsv"
	exit $failed
	;;
*)
	[ -f "$HDL/knowledge/queries/$1.sql" ] || { echo "no query $1 (knowledge.sh list)" >&2; exit 2; }
	build
	query "$HDL/knowledge/queries/$1.sql"
	;;
esac
