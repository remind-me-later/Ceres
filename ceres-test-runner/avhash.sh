#!/usr/bin/env bash
# The A/V hash between two trees: builds examples/avhash.rs on REV (HEAD by
# default) and on the working tree, runs both on the same ROMs, and diffs
# their outputs. No output from diff means the change kept the sound, the
# picture and the final state of every run (see examples/avhash.rs).
#
#   ceres-test-runner/avhash.sh [REV] [-- avhash's options]
#
# REV's tree is checked out in target/avhash/base, with this tree's
# examples/avhash.rs (so both run the same harness) and this tree's boot
# ROMs and test ROMs. The outputs stay in target/avhash/{base,new}.txt. Set
# CARGO_BUILD_JOBS to bound the builds; --jobs bounds the runs.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

rev=HEAD
if [ $# -gt 0 ] && [ "$1" != "--" ]; then
	rev=$1
	shift
fi
[ "${1-}" = "--" ] && shift

out=target/avhash
base=$out/base
mkdir -p "$out"

sha=$(git rev-parse --verify "$rev^{commit}")
if [ -d "$base" ]; then
	git -C "$base" checkout -q --detach "$sha"
	git -C "$base" checkout -q -- .
else
	git worktree add -q --detach "$base" "$sha"
fi
cp ceres-test-runner/examples/avhash.rs "$base/ceres-test-runner/examples/avhash.rs"
for dir in gb-bootroms test-roms; do
	if [ ! -L "$base/external/$dir" ]; then
		rm -rf "$base/external/$dir"
		ln -s "$root/external/$dir" "$base/external/$dir"
	fi
done

build() {
	cargo build -q -p ceres-test-runner --release --example avhash \
		--manifest-path "$1/Cargo.toml" --target-dir "$2"
}
build "$base" "$out/target-base"
build . target

roms=$root/external/test-roms
echo "avhash: $(git rev-parse --short "$sha") ..." >&2
"$out/target-base/release/examples/avhash" --roms "$roms" "$@" >"$out/base.txt"
echo "avhash: working tree ..." >&2
target/release/examples/avhash --roms "$roms" "$@" >"$out/new.txt"

if diff "$out/base.txt" "$out/new.txt"; then
	echo "avhash: $(wc -l <"$out/new.txt") runs, all the same" >&2
else
	echo "avhash: runs differ (columns: audio, video, state)" >&2
	exit 1
fi
