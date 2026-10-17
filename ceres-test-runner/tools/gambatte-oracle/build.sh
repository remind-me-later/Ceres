#!/bin/sh
# Builds the reference emulator (gambatte) plus oracle.cpp into
# target/gambatte-oracle/gambatte-oracle (or into $1).
#
# Needs g++. The sources are the ones listed in the submodule's SConstruct
# (scons itself is not needed). JOBS sets the number of parallel compilers.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
gambatte="$repo/external/reference-implementations/gambatte-core"
out=${1:-$repo/target/gambatte-oracle}
jobs=${JOBS:-4}

if [ ! -f "$gambatte/libgambatte/SConstruct" ]; then
	echo "gambatte submodule missing: git submodule update --init external/reference-implementations/gambatte-core" >&2
	exit 1
fi

mkdir -p "$out/obj"
cd "$gambatte/libgambatte"

# The core of the SConstruct's source list, plus the two files its zlib
# branch leaves out.
{
	sed -n "/^sourceFiles = Split/,/^\t\t   ''')/p" SConstruct | grep -o 'src/[A-Za-z0-9_/]*\.cpp'
	echo src/file/file.cpp
	echo src/file/crc32.cpp
} > "$out/sources.txt"

flags="-O2 -std=c++11 -fno-exceptions -fno-rtti -DHAVE_STDINT_H -Isrc -Iinclude -I../common -w"

# shellcheck disable=SC2016
xargs -P "$jobs" -I{} sh -c '
	obj="$1/obj/$(echo "$2" | tr / _).o"
	if [ ! -f "$obj" ] || [ "$2" -nt "$obj" ]; then
		g++ '"$flags"' -c "$2" -o "$obj"
	fi' _ "$out" {} < "$out/sources.txt"

# shellcheck disable=SC2086
g++ $flags "$here/oracle.cpp" "$out"/obj/*.o -o "$out/gambatte-oracle"
echo "built $out/gambatte-oracle"
