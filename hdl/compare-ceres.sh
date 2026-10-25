#!/bin/sh
# Compares ceres with the readable model of the chip: the CPU's memory
# accesses, M-cycle by M-cycle, from power-on.
#
#   compare-ceres.sh <ROM> [SECS]
#
# runs the ROM (a path relative to the repository) for SECS seconds after
# the chip's reset (0.1 by default) on the model (board/gameboy_tb.vhd,
# BUS_TRACE) and on ceres (its example bus_trace), writes both traces to
# build/compare/<ROM>/model.txt and ceres.txt, and prints where they first
# differ. A line is an access: the M-cycle, counted from the CPU's first
# access, R or W, the address and the byte.
#
# The boot ROM is build/fastboot.bin, which turns the LCD on and jumps to
# the cartridge right after the reset: both start from the same state,
# which is all the comparison needs, without the 150 ms of the real boot.
# (VRAM and OAM then keep what they power up with: arbitrary bytes on the
# model's board, FF in the model's OAM, zeros in ceres. A ROM that reads
# them before it writes them differs there.)
# BOOT=quick runs dmg-sim's quickboot instead, which leaves the machine as
# the real boot ROM does (for the tests of that state, such as gbmicrotest's
# poweron_* and Mooneye's boot_*). TEST=gbmicrotest (or mooneye, blargg)
# ends the run when the test reports its result, usually within a frame.
# BOOT=tiny runs build/tinyboot.bin, which leaves the machine as quickboot does
# (the same accesses, in the 513 gbmicrotest and the 1575 Gambatte ROMs
# compared) in a ninth of the time: 17.8 thousand M-cycles, not 158 thousand.
# It is not for sound tests; APU=0 leaves out the four sound channels (15 %
# faster, nothing sounds, their registers read as unconnected).
# BOOT=real runs the real DMG boot ROM (external/gb-bootroms), for the ROMs
# that need the state it leaves and do nothing after quickboot: Mealybug's
# m3_lcdc_obj_en_change, m3_obp0_change and m3_scx_low_3_bits, which draw
# after about 1.1 s of its logo.
#
# PIXELS=1 also compares the pixel pipeline's timeline (model-pixels.txt and
# ceres-pixels.txt, compare-pixels.sh).
#
# CUTS=1 also compares what the blocks show each other (model-cuts.txt and
# ceres-cuts.txt, compare-cuts.sh): IF, TIMA, the OAM DMA, LY, STAT's mode,
# the OAM's and VRAM's being busy for the CPU (OAMB, VRAMB) and the
# divider's jumps, change by change.
#
# The screen is compared too: every whole frame on the LCD, as the model's
# LCD pins draw it (FRAMES) and as ceres presents it (bus_trace --frames),
# in model-frames.txt and ceres-frames.txt (a line of 160 digits per row, 0
# white to 3 black; the first frame after the LCD is turned on is left out),
# compared by compare-frames.sh: the first picture that
# differs and how many differ. NOFRAMES=1 skips this. Without TEST, give SECS
# enough for the ROM to draw. Use BOOT=quick for pictures: fastboot leaves
# VRAM as the model's board powers up (arbitrary bytes), quickboot zeroes it
# and sets the palette as the real boot ROM does.

set -eu

HDL=$(cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(cd -- "$HDL/.." && pwd)
BOOTROM=$HDL/build/fastboot.bin
[ "${BOOT:-fast}" = quick ] && BOOTROM=$ROOT/external/reference-implementations/dmg-sim/boot/quickboot.bin
[ "${BOOT:-fast}" = real ] && BOOTROM=$ROOT/external/gb-bootroms/bin/dmg.bin
# BOOT=tiny: quickboot's machine in a ninth of its time, the VRAM zeroed by the
# board (boot/tinyboot.asm); for the model, which simulates the boot slowly.
vram_zero=
if [ "${BOOT:-fast}" = tiny ]; then
	BOOTROM=$HDL/build/tinyboot.bin
	vram_zero=true
fi

[ $# -ge 1 ] || { echo "usage: $0 <ROM> [SECS]" >&2; exit 2; }
rom=$1
secs=${2:-0.1}
dir=$HDL/build/compare/${rom%.gb}
mkdir -p "$dir"

make -C "$HDL" -s model build/fastboot.bin build/tinyboot.bin
cargo build -q --release --manifest-path "$ROOT/Cargo.toml" -p ceres-test-runner --example bus_trace

frames_model= frames_ceres=
if [ -z "${NOFRAMES:-}" ]; then
	frames_model=$dir/model-frames.txt
	frames_ceres=$dir/ceres-frames.txt
fi

pixels_model= pixels_ceres=
if [ -n "${PIXELS:-}" ]; then
	pixels_model=$dir/model-pixels.txt
	pixels_ceres=$dir/ceres-pixels.txt
fi

cuts_model= cuts_ceres=
if [ -n "${CUTS:-}" ]; then
	cuts_model=$dir/model-cuts.txt
	cuts_ceres=$dir/ceres-cuts.txt
fi

(cd "$dir" && nvc --std=2008 -H 64m --work="$HDL/build/model/work" -e --jit gameboy_tb \
	-gBOOTROM="$BOOTROM" ${vram_zero:+"-gVRAM_ZERO=$vram_zero"} ${APU:+"-gWITH_CHANNELS=false"} -gROM="$ROOT/$rom" -gSECS="$secs" -gBUS_TRACE="$dir/model.txt" \
	${TEST:+"-gTEST=$TEST"} ${frames_model:+"-gFRAMES=$frames_model"} \
	${pixels_model:+"-gPIXELS=$pixels_model"} ${cuts_model:+"-gCUTS=$cuts_model"} -r >model.log 2>&1)

# The same M-cycles from ceres: up to the model's last, which may be cut.
mcycles=$(tail -n 1 "$dir/model.txt" | cut -d ' ' -f 1)
sed -i "/^$mcycles /,\$d" "$dir/model.txt"
"$ROOT/target/release/examples/bus_trace" "$ROOT/$rom" --bootrom "$BOOTROM" \
	--mcycles "$mcycles" --out "$dir/ceres.txt" ${frames_ceres:+--frames "$frames_ceres"} \
	${pixels_ceres:+--pixels "$pixels_ceres"} ${cuts_ceres:+--cuts "$cuts_ceres"}

status=0
echo "$rom: $(wc -l <"$dir/model.txt") accesses in $mcycles M-cycles"
if cmp -s "$dir/ceres.txt" "$dir/model.txt"; then
	echo "same"
else
	# The first difference, with the accesses before it.
	diff -U 8 "$dir/ceres.txt" "$dir/model.txt" | awk '/^@@/ { n++ } n == 1' | head -n 40
	status=1
fi

# What the pixel pipeline did, a line at a time (compare-pixels.sh), with
# PIXELS=1.
if [ -n "$pixels_model" ]; then
	"$HDL/compare-pixels.sh" "$pixels_model" "$pixels_ceres" || status=1
fi

# What the blocks show each other (compare-cuts.sh), with CUTS=1, up to the
# model's last M-cycle.
if [ -n "$cuts_model" ]; then
	awk -v last="$mcycles" '$1 < last' "$cuts_model" >"$cuts_model.cut"
	mv "$cuts_model.cut" "$cuts_model"
	"$HDL/compare-cuts.sh" "$cuts_model" "$cuts_ceres" "$dir/model.txt" || status=1
fi

# The frames (compare-frames.sh).
if [ -n "$frames_model" ]; then
	"$HDL/compare-frames.sh" "$frames_model" "$frames_ceres" || status=1
fi
exit $status
