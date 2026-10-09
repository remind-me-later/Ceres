# Gambatte oracle

Gambatte (the `external/reference-implementations/gambatte-core` submodule)
passes every check of its own hardware test ROMs, on both the DMG and the CGB
(4674 checks). That makes it a precise reference for the cases ceres still
fails: when a ROM fails on ceres, gambatte tells what the hardware does, and
tracing both emulators side by side shows where they diverge.

## Build and run

```sh
ceres-test-runner/tools/gambatte-oracle/build.sh   # -> target/gambatte-oracle/gambatte-oracle
cd external/test-roms/gambatte
find . -name '*.gb' -o -name '*.gbc' | sed 's|^\./||' | sort \
    | ../../../target/gambatte-oracle/gambatte-oracle > /tmp/oracle.txt
```

It needs `g++` only (`JOBS=n` sets the number of parallel compilers, 4 by
default). The output has one line per check, `model path PASS|FAIL expected
actual`, exactly like the one of the ceres side:

```sh
cargo run --release -p ceres-test-runner --example gambatte_runner -- both window > /tmp/ceres.txt
grep window /tmp/oracle.txt | LC_ALL=C sort -k1,1 -k2,2 | diff - /tmp/ceres.txt
```

`gambatte_runner` takes an optional path filter, so a family can be iterated on
in seconds; the `gambatte` test (`cargo nextest run -p ceres-test-runner
--test gambatte --run-ignored only`) is still the one that guards the
`gambatte_known_failures_{dmg,cgb}.txt` lists.

## Tracing

The method that found most of the fixes: add `fprintf(stderr, ...)` lines to a
*scratch copy* of gambatte (CPU reads/writes of the registers of interest with
`ly` and the dot inside the line, interrupt and PPU events) and the equivalent
temporary prints to ceres, run the same ROM on both and look for the first
divergence. ceres' dot counter (`line_clock`) runs about 8 dots ahead of
gambatte's for PPU events and 9 for CPU accesses (11 in double speed). These
patches are not kept in the tree.

## License

Gambatte is GPLv2, ceres is MIT. `oracle.cpp` only uses gambatte's public API
and contains none of its code; the oracle binary is a local development tool
that is never distributed. Read gambatte for *behaviour*; do not copy its code
or data tables into ceres.
