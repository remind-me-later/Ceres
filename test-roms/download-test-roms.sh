#!/usr/bin/env bash

# Script to download Game Boy test ROMs from the gameboy-test-roms repository
# Usage: ./download-test-roms.sh [version]
# Example: ./download-test-roms.sh v7.0

set -e

# Configuration
REPO="c-sp/gameboy-test-roms"
VERSION="${1:-v7.0}"  # Default to v7.0 if no version specified
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/game-boy-test-roms-${VERSION}.zip"
ZIP_FILE="game-boy-test-roms-${VERSION}.zip"
EXTRACT_DIR="game-boy-test-roms-${VERSION}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Downloading Game Boy Test ROMs ${VERSION}...${NC}"

# Check if ROMs are already downloaded
if [ -d "blargg" ] && [ -d "mooneye-test-suite" ]; then
    echo -e "${YELLOW}Test ROMs appear to be already downloaded.${NC}"
    read -p "Do you want to re-download? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${GREEN}Using existing test ROMs.${NC}"
        exit 0
    fi
fi

# Check if curl or wget is available
if command -v curl &> /dev/null; then
    DOWNLOADER="curl -L -o"
elif command -v wget &> /dev/null; then
    DOWNLOADER="wget -O"
else
    echo -e "${RED}Error: Neither curl nor wget is available. Please install one of them.${NC}"
    exit 1
fi

# Download the test ROMs
echo -e "${YELLOW}Downloading from: ${DOWNLOAD_URL}${NC}"
$DOWNLOADER "${ZIP_FILE}" "${DOWNLOAD_URL}"

if [ ! -f "${ZIP_FILE}" ]; then
    echo -e "${RED}Error: Download failed. File not found: ${ZIP_FILE}${NC}"
    exit 1
fi

# Extract the archive (using -o to overwrite without prompting)
echo -e "${GREEN}Extracting test ROMs...${NC}"
unzip -o -q "${ZIP_FILE}"

if [ ! -d "${EXTRACT_DIR}" ]; then
    echo -e "${RED}Error: Extraction failed. Directory not found: ${EXTRACT_DIR}${NC}"
    echo -e "${YELLOW}Contents of current directory:${NC}"
    ls -la
    exit 1
fi

# Organize the ROMs into the current directory structure
echo -e "${GREEN}Organizing test ROMs...${NC}"

# Copy all ROM directories from the extracted folder
# Exclude README.md, LICENSE, and CHANGELOG.md to avoid conflicts
for item in "${EXTRACT_DIR}"/*; do
    if [ -d "$item" ]; then
        dirname=$(basename "$item")
        echo "  - Copying ${dirname}/"
        rm -rf "./${dirname}"
        cp -r "$item" "./${dirname}"
    elif [ -f "$item" ]; then
        filename=$(basename "$item")
        # Only copy if it's not one of our local files
        if [[ "$filename" != "download-test-roms.sh" ]] && \
           [[ "$filename" != ".gitignore" ]]; then
            echo "  - Copying ${filename}"
            cp "$item" "./${filename}"
        fi
    fi
done

# Clean up
echo -e "${GREEN}Cleaning up temporary files...${NC}"
rm -f "${ZIP_FILE}"
rm -rf "${EXTRACT_DIR}"

echo -e "${GREEN}✓ Test ROMs downloaded and organized successfully!${NC}"
echo ""
echo -e "${YELLOW}Test ROMs are now available in:${NC}"
echo "  $(pwd)"
echo ""
echo -e "${YELLOW}To run tests, execute:${NC}"
echo "  cd .."
echo "  cargo test --package ceres-tests -- --ignored --include-ignored"
