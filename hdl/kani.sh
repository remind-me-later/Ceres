#!/bin/sh
# Kani's proofs of ceres-core, run and recorded.
#
# A harness's verdict holds for the sources it was run on. kani-results.tsv
# keeps, per harness, the hash of those sources with the verdict, the time
# and the solver, so that a rerun takes only what is not verified yet, and
# a verdict can be traced to what it was about.
#
#   kani.sh list            the harnesses, by their full names
#   kani.sh status          each harness: verified on these sources, stale
#                           (verified on others) or never run; exits 1
#                           unless all are verified
#   kani.sh run [NAME...]   run the harnesses that are not verified on these
#                           sources (those whose names contain a NAME, if
#                           given), and record their verdicts; exits 1 if
#                           one fails
#   kani.sh rerun NAME...   run those harnesses again, verified or not
#   kani.sh faults [NAME]   the planted faults, kani-faults/*.patch: each
#                           names a harness that must fail with it applied,
#                           and is recorded as caught or missed; exits 1 if
#                           one is missed
#   kani.sh sync            bring the proofs' worktree to this one
#
# The runs go in a second worktree (KANI_WORKTREE, made on first use), a
# snapshot of this one with its uncommitted changes, so that the sources
# here can change while a run is going: Kani compiles each harness from
# the sources it finds.
#
# The sources a verdict is about: ceres-core and ceres-netlist (every file
# git knows or would add) and the workspace's Cargo.toml and Cargo.lock.
# That is everything a harness could reach, so any change there makes
# every verdict stale.
#
# A run is one `cargo kani` with KANI_JOBS harnesses at a time (2), under
# a memory cap where systemd-run is there (KANI_MEMORY, 9G; no swap), on
# KANI_CORES if given (taskset's list), with KANI_SOLVER (cadical).
# Without Kani's concrete playback: it multiplies a harness's memory, and
# only a failing harness needs it, for its counterexample:
#
#   cargo kani -p ceres-core --harness NAME --exact \
#       -Z concrete-playback --concrete-playback=print

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
RESULTS=$HDL/kani-results.tsv
FAULTS=$HDL/kani-faults
WT=${KANI_WORKTREE:-$ROOT-kani}
JOBS=${KANI_JOBS:-2}
MEMORY=${KANI_MEMORY:-9G}
SOLVER=${KANI_SOLVER:-cadical}
LOGS=$WT/target/kani-logs
SOURCES="ceres-core ceres-netlist Cargo.toml Cargo.lock"

# The harnesses of ceres-core, by their full names: each `#[kani::proof]`
# with the module its file is.
harnesses() {
    cd "$ROOT/ceres-core/src"
    grep -rl --include='*.rs' '#\[kani::proof\]' . | sort | while read -r file; do
        module=$(printf '%s\n' "$file" |
            sed -e 's|^\./||' -e 's|\.rs$||' -e 's|/mod$||' -e 's|^lib$||' -e 's|/|::|g')
        awk -v module="$module" '
            /#\[kani::proof\]/ { proof = 1; next }
            proof && /^[[:space:]]*(pub(\([a-z]+\))? )?fn / {
                name = $0
                sub(/^.*fn /, "", name)
                sub(/\(.*$/, "", name)
                print (module == "" ? name : module "::" name)
                proof = 0
            }' "$file"
    done
}

# The hash of the sources, as the tree at $1 has them.
sources_hash() {
    cd "$1"
    # shellcheck disable=SC2086
    git ls-files -co --exclude-standard -- $SOURCES | LC_ALL=C sort |
        xargs sha256sum | sha256sum | cut -c1-16
}

# The worktree, as this tree is: its commit, its uncommitted changes and
# the files git would add.
sync() {
    if pgrep -x cbmc >/dev/null || pgrep -x kani-driver >/dev/null; then
        echo "a Kani run is going: not syncing the worktree" >&2
        exit 1
    fi
    if [ ! -d "$WT" ]; then
        git -C "$ROOT" worktree add -q --detach "$WT" HEAD
        # The boot ROMs ceres-core includes are a submodule's.
        ln -s "$ROOT/external/gb-bootroms/bin" "$WT/external/gb-bootroms/bin"
    fi
    git -C "$WT" checkout -q -- .
    # shellcheck disable=SC2086
    git -C "$WT" clean -qfd -- $SOURCES
    git -C "$WT" checkout -q --detach "$(git -C "$ROOT" rev-parse HEAD)"
    git -C "$ROOT" diff HEAD | git -C "$WT" apply --allow-empty
    # shellcheck disable=SC2086
    (cd "$ROOT" && git ls-files -o --exclude-standard -z -- $SOURCES |
        xargs -0 -r cp --parents -t "$WT")
}

# The recorded line of a harness (or a fault), if any.
recorded() {
    if [ -f "$RESULTS" ]; then
        awk -F '\t' -v name="$1" '$1 == name' "$RESULTS"
    fi
}

# Records name, hash, verdict, seconds: replaces the name's line.
record() {
    {
        if [ -f "$RESULTS" ]; then
            grep -v '^#' "$RESULTS" | awk -F '\t' -v name="$1" '$1 != name'
        fi
        printf '%s\t%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" "$SOLVER"
    } | LC_ALL=C sort >"$RESULTS.new"
    {
        echo "# Kani's verdicts, written by hdl/kani.sh: the harness (or the planted"
        echo "# fault), the hash of the sources it ran on, the verdict, the seconds"
        echo "# and the solver."
        cat "$RESULTS.new"
    } >"$RESULTS"
    rm "$RESULTS.new"
}

# The harnesses of the standard input in their state on the sources $1:
# "verified", "stale" or "new", then the name.
states() {
    while read -r name; do
        line=$(recorded "$name")
        if [ -z "$line" ]; then
            echo "new $name"
        elif [ "$(printf '%s' "$line" | cut -f2,3)" = "$(printf '%s\tpass' "$1")" ]; then
            echo "verified $name"
        else
            echo "stale $name"
        fi
    done
}

# Kani's terse output as "name verdict seconds" lines.
verdicts() {
    awk '
        /Thread [0-9]+: Checking harness/ {
            name = $NF
            sub(/\.\.\.$/, "", name)
            running[$2] = name
            next
        }
        /^Checking harness/ { name = $NF; sub(/\.\.\.$/, "", name); running["one"] = name; last = "one"; next }
        /^Thread [0-9]+: *$/ { last = $2; next }
        /VERIFICATION:- / { verdict = ($2 == "SUCCESSFUL" ? "pass" : "fail") }
        /^Verification Time:/ {
            seconds = $3
            sub(/s$/, "", seconds)
            printf "%s %s %.0f\n", running[last], verdict, seconds
        }' "$1"
}

# Runs the harnesses of the standard input in the worktree; prints
# "name verdict seconds" for each that finished.
kani() {
    mkdir -p "$LOGS"
    log=$LOGS/run.log
    set --
    while read -r name; do
        set -- "$@" --harness "$name"
    done
    [ $# -gt 0 ] || return 0
    set -- cargo kani -p ceres-core "$@" --exact -j "$JOBS" \
        --output-format terse --solver "$SOLVER"
    if [ -n "${KANI_CORES:-}" ]; then
        set -- taskset -c "$KANI_CORES" "$@"
    fi
    if command -v systemd-run >/dev/null; then
        set -- systemd-run --user --scope -q -p "MemoryMax=$MEMORY" -p MemorySwapMax=0 "$@"
    fi
    (cd "$WT" && nice "$@" >"$log" 2>&1) || true
    verdicts "$log"
}

# Runs the harnesses of the standard input and records their verdicts.
run_and_record() {
    hash=$1
    failed=0
    kani >"$LOGS.verdicts" 2>/dev/null || true
    while read -r name verdict seconds; do
        record "$name" "$hash" "$verdict" "$seconds"
        echo "$verdict $name (${seconds}s)"
        [ "$verdict" = pass ] || failed=1
    done <"$LOGS.verdicts"
    rm -f "$LOGS.verdicts"
    return "$failed"
}

case "${1:-}" in
list)
    harnesses
    ;;
status)
    hash=$(sources_hash "$ROOT")
    harnesses | states "$hash" | LC_ALL=C sort
    ! harnesses | states "$hash" | grep -qv '^verified '
    ;;
sync)
    sync
    echo "the worktree $WT is at $(sources_hash "$WT")"
    ;;
run | rerun)
    command=$1
    shift
    sync
    hash=$(sources_hash "$WT")
    mkdir -p "$LOGS"
    harnesses | states "$hash" | while read -r state name; do
        [ "$command" = rerun ] || [ "$state" != verified ] || continue
        if [ $# -eq 0 ]; then
            echo "$name"
        else
            for wanted in "$@"; do
                case "$name" in *"$wanted"*) echo "$name" && break ;; esac
            done
        fi
    done >"$LOGS/todo"
    echo "$(wc -l <"$LOGS/todo") harnesses to run on $hash, $JOBS at a time"
    run_and_record "$hash" <"$LOGS/todo"
    ;;
faults)
    shift
    missed=0
    for patch in "$FAULTS"/*.patch; do
        fault=fault/$(basename "$patch" .patch)
        case "$fault" in *"${1:-}"*) ;; *) continue ;; esac
        harness=$(sed -n 's/^# harness: //p' "$patch")
        sync
        hash=$(sources_hash "$WT")
        if [ "$(recorded "$fault" | cut -f2,3)" = "$(printf '%s\tcaught' "$hash")" ]; then
            echo "caught $fault (recorded)"
            continue
        fi
        git -C "$WT" apply "$patch"
        verdict=$(harnesses | grep -F "$harness" | kani | cut -d' ' -f2 | sort -u | tr '\n' ' ')
        case "$verdict" in
        "fail ") outcome=caught ;;
        *) outcome=missed && missed=1 ;;
        esac
        record "$fault" "$hash" "$outcome" 0
        echo "$outcome $fault ($harness: ${verdict:-no verdict})"
    done
    sync
    exit "$missed"
    ;;
*)
    sed -n '2,/^$/s/^# \{0,1\}//p' "$0"
    exit 2
    ;;
esac
