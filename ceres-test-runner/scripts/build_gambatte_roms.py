#!/usr/bin/env python3
"""Build gambatte test ROMs from .asm sources.

This script finds all .asm files in the gambatte hwtests directory and
builds them into .gb/.gbc ROMs in the test-roms/gambatte directory.
It only rebuilds ROMs that are missing or older than their source.

The gambatte tests use a custom assembler (qdgbas.py) that is part of
the gambatte-core reference implementation. Building from source ensures
the ROMs are reproducible and not prebuilt binaries.
"""

import os
import shutil
import sys
import warnings
from pathlib import Path

# Suppress the SyntaxWarnings from qdgbas.py (Python 3.12+ deprecates \w)
warnings.filterwarnings("ignore", category=SyntaxWarning, message=r"\\w")

# Add gambatte-core test directory to path so we can import qdgbas
# Script lives at ceres-test-runner/scripts/, so we go up two levels to the repo root
GAMBATTE_TEST_DIR = Path(__file__).resolve().parent.parent.parent / "external" / "reference-implementations" / "gambatte-core" / "test"
sys.path.insert(0, str(GAMBATTE_TEST_DIR))

from qdgbas import assembleFile, readDataFromFile, outFilenameFromInFilename  # noqa: E402

HWTESTS_DIR = GAMBATTE_TEST_DIR / "hwtests"
# Go up three levels from GAMBATTE_TEST_DIR to get to the repo root:
# gambatte-core/test -> gambatte-core -> reference-implementations -> external
OUTPUT_DIR = GAMBATTE_TEST_DIR.parent.parent.parent / "test-roms" / "gambatte"


def find_asm_files():
    """Find all .asm files in hwtests/."""
    return sorted(HWTESTS_DIR.rglob("*.asm"))


def compute_output_path(asm_path: Path) -> Path:
    """Compute the output ROM path for a given .asm file.

    Maps hwtests/window/foo.asm -> test-roms/gambatte/window/foo.gbc (or .gb).
    The .gbc vs .gb extension is determined by the CGB flag in the ROM header,
    so we build to a temp location first and then move to the final location.
    """
    rel = asm_path.relative_to(HWTESTS_DIR)
    # Output goes to test-roms/gambatte/<same relative path, but with .gb/.gbc>
    return OUTPUT_DIR / rel


def build_rom(asm_path: Path, force: bool = False) -> bool:
    """Build a single ROM from its .asm source.

    Returns True if the ROM was built, False if it was already up to date.
    """
    out_path = compute_output_path(asm_path)
    out_dir = out_path.parent
    out_dir.mkdir(parents=True, exist_ok=True)

    # Use a temp filename that doesn't start with '.' (qdgbas's outFilenameFromInFilename
    # uses rsplit('.', 1) which would treat a leading dot as the extension separator).
    # We suffix with a safe marker and rename the output after building.
    tmp_asm = out_dir / f"__build_tmp_{asm_path.stem}.asm"
    shutil.copy(asm_path, tmp_asm)

    try:
        outdata = assembleFile(readDataFromFile(str(tmp_asm)))
        # Determine the correct extension from the CGB flag in the ROM header
        ext = ".gbc" if (outdata[0x143] & 0x80) else ".gb"
        final_out = out_dir / f"{asm_path.stem}{ext}"

        # Check if rebuild is needed
        if not force and final_out.exists():
            asm_mtime = asm_path.stat().st_mtime
            rom_mtime = final_out.stat().st_mtime
            if rom_mtime >= asm_mtime:
                return False  # Up to date

        # Write the ROM
        final_out.write_bytes(bytes(outdata))
        return True
    finally:
        if tmp_asm.exists():
            tmp_asm.unlink()


def main():
    if not HWTESTS_DIR.exists():
        print(f"Error: hwtests directory not found: {HWTESTS_DIR}", file=sys.stderr)
        sys.exit(1)

    asm_files = find_asm_files()
    print(f"Found {len(asm_files)} .asm files in {HWTESTS_DIR}")

    built = 0
    skipped = 0
    failed = 0

    for asm_path in asm_files:
        try:
            if build_rom(asm_path):
                built += 1
            else:
                skipped += 1
        except Exception as e:
            print(f"  FAILED: {asm_path.relative_to(HWTESTS_DIR)}: {e}", file=sys.stderr)
            failed += 1

    print(f"Built: {built}, Skipped (up to date): {skipped}, Failed: {failed}")
    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
