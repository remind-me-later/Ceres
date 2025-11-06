//! Build script to download Game Boy test ROMs
//!
//! This script automatically downloads test ROMs from the gameboy-test-roms
//! repository if they're not already present. This is particularly useful for
//! CI/CD pipelines where the ROMs need to be downloaded automatically.

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Download and extract test ROMs using curl and unzip
fn download_and_extract_roms(test_roms_dir: &Path) -> Result<()> {
    let url = format!(
        "https://github.com/{REPO}/releases/download/{VERSION}/game-boy-test-roms-{VERSION}.zip"
    );

    println!("cargo:warning=Downloading from: {url}");

    // Create test-roms directory if it doesn't exist
    std::fs::create_dir_all(test_roms_dir).context("Failed to create test-roms directory")?;

    // Create a temporary file for the download
    let temp_zip = test_roms_dir.with_file_name("test-roms-temp.zip");

    // Download using curl
    let download_status = Command::new("curl")
        .arg("-L") // Follow redirects
        .arg("-f") // Fail on HTTP errors
        .arg("-o")
        .arg(&temp_zip)
        .arg(&url)
        .status()
        .context("Failed to execute curl")?;

    if !download_status.success() {
        anyhow::bail!("Download failed with curl exit code: {download_status}");
    }

    println!("cargo:warning=Extracting test ROMs...");

    // Extract using unzip directly to test-roms directory
    let extract_status = Command::new("unzip")
        .arg("-q") // Quiet mode
        .arg("-o") // Overwrite without prompting
        .arg(&temp_zip)
        .arg("-d")
        .arg(test_roms_dir)
        .status()
        .context("Failed to execute unzip")?;

    if !extract_status.success() {
        anyhow::bail!("Extraction failed with unzip exit code: {extract_status}");
    }

    // Clean up the zip file
    let _ = std::fs::remove_file(&temp_zip);

    Ok(())
}
