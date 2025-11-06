//! Build script to download Game Boy test ROMs
//!
//! This script automatically downloads test ROMs from the gameboy-test-roms
//! repository if they're not already present. This is particularly useful for
//! CI/CD pipelines where the ROMs need to be downloaded automatically.

#![allow(clippy::implicit_clone)]

use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const REPO: &str = "c-sp/gameboy-test-roms";
const VERSION: &str = "v7.0";

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    // Get the test-roms directory path
    let test_roms_dir = get_test_roms_dir()?;

    // Check if ROMs are already downloaded (check for key directories)
    if roms_already_downloaded(&test_roms_dir) {
        // Already have ROMs, nothing to do
        return Ok(());
    }

    // Always download when building/testing this crate
    println!("cargo:warning=Downloading Game Boy test ROMs v{VERSION}...");

    download_and_extract_roms(&test_roms_dir).context("Failed to download test ROMs")?;

    println!("cargo:warning=Test ROMs downloaded successfully!");

    Ok(())
}

/// Get the path to the test-roms directory
fn get_test_roms_dir() -> Result<PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR not set")?;

    let test_roms_dir = PathBuf::from(manifest_dir)
        .parent()
        .context("Failed to get parent directory")?
        .join("test-roms");

    Ok(test_roms_dir)
}

/// Check if test ROMs are already downloaded
fn roms_already_downloaded(test_roms_dir: &Path) -> bool {
    // Check for the presence of key ROM directories
    let blargg_dir = test_roms_dir.join("blargg");
    let cpu_instrs = blargg_dir.join("cpu_instrs");

    cpu_instrs.exists() && cpu_instrs.join("cpu_instrs.gb").exists()
}

/// Download and extract test ROMs
fn download_and_extract_roms(test_roms_dir: &Path) -> Result<()> {
    let url = format!(
        "https://github.com/{REPO}/releases/download/{VERSION}/game-boy-test-roms-{VERSION}.zip"
    );

    println!("cargo:warning=Downloading from: {url}");

    // Download the ZIP file
    let response =
        reqwest::blocking::get(&url).with_context(|| format!("Failed to download from {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed with status: {}", response.status());
    }

    let bytes = response.bytes().context("Failed to read response bytes")?;

    println!("cargo:warning=Extracting test ROMs...");

    // Extract the ZIP file
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("Failed to open ZIP archive")?;

    let extract_prefix = format!("game-boy-test-roms-{VERSION}/");

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .context("Failed to get file from archive")?;

        let Some(outpath) = file.enclosed_name().map(|p| p.to_owned()) else {
            continue;
        };

        // Strip the version prefix from the path
        let relative_path = outpath
            .strip_prefix(&extract_prefix)
            .map_or_else(|_| outpath.clone(), std::path::Path::to_path_buf);

        // Skip if it's just the root directory
        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let dest_path = test_roms_dir.join(&relative_path);

        if file.is_dir() {
            fs::create_dir_all(&dest_path)
                .with_context(|| format!("Failed to create directory: {}", dest_path.display()))?;
        } else {
            // Create parent directories if they don't exist
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }

            // Extract the file
            let mut outfile = File::create(&dest_path)
                .with_context(|| format!("Failed to create file: {}", dest_path.display()))?;

            io::copy(&mut file, &mut outfile)
                .with_context(|| format!("Failed to extract file: {}", dest_path.display()))?;
        }
    }

    Ok(())
}
