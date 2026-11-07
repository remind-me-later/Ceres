#!/bin/sh
# Translates the steps that ceres-core's Kani proofs prove for every
# argument, timing::check::step_agrees and interrupts::check::step_agrees,
# with what they call (ceres-netlist's transcriptions included), to Lean:
# Ceres/Gen.
#
#   translate.sh         write Ceres/Gen
#   translate.sh check   translate into a temporary folder and compare it
#                        with Ceres/Gen (exits 1 if they differ)
#
# CHARON and AENEAS name the two programs (default: charon and aeneas on
# PATH): Charon at commit 8cae8fd1 and Aeneas at nightly-2026.11.07-c5eb4a2,
# the Aeneas whose Lean library lakefile.toml requires (README.md). Charon
# builds ceres-core with its own nightly toolchain, through rustup, in
# target/charon. The translation must leave no function as an axiom.
set -eu

LEAN=$(cd "$(dirname "$0")" && pwd)
ROOT=$(dirname "$LEAN")
CHARON=${CHARON:-charon}
AENEAS=${AENEAS:-aeneas}
case ${1:-write} in
write | check) mode=${1:-write} ;;
*)
	echo "usage: translate.sh [check]" >&2
	exit 2
	;;
esac

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$ROOT"
CARGO_TARGET_DIR=$ROOT/target/charon "$CHARON" cargo --preset=aeneas \
	--start-from crate::timing::check::step_agrees \
	--start-from crate::interrupts::check::step_agrees \
	--include ceres_netlist \
	--dest-file "$work/ceres.llbc" -- -p ceres-core --features check >"$work/charon.log" 2>&1 || {
	tail -n 30 "$work/charon.log" >&2
	exit 1
}
"$AENEAS" -backend lean -split-files -subdir Ceres/Gen -dest "$work/out" \
	"$work/ceres.llbc" >"$work/aeneas.log" 2>&1 || {
	tail -n 30 "$work/aeneas.log" >&2
	exit 1
}
out=$work/out/Ceres/Gen
for external in "$out"/*External*; do
	[ -e "$external" ] || continue
	echo "translate.sh: Aeneas left these as axioms:" >&2
	grep '^axiom' "$external" >&2 || true
	exit 1
done

if [ "$mode" = check ]; then
	diff -r "$out" "$LEAN/Ceres/Gen" >&2 || {
		echo "translate.sh: Ceres/Gen is not the translation of the sources: run translate.sh" >&2
		exit 1
	}
	echo "Ceres/Gen is up to date"
else
	rm -rf "$LEAN/Ceres/Gen"
	mkdir -p "$LEAN/Ceres"
	cp -r "$out" "$LEAN/Ceres/Gen"
	echo "wrote Ceres/Gen"
fi
