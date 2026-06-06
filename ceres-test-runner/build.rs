//! Build script for Game Boy test ROMs
//!
//! This script ensures all required test ROMs are present:
//! 1. Gambatte ROMs are built from .asm sources in the gambatte-core repo
//!    (using the custom `qdgbas.py` assembler). Building from source ensures
//!    reproducibility — no prebuilt binaries.
//! 2. Other ROMs (blargg, mooneye, etc.) are downloaded from the
//!    gameboy-test-roms release if not already present.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

const REPO: &str = "c-sp/gameboy-test-roms";
const VERSION: &str = "v7.0";

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=scripts/build_gambatte_roms.py");

    // Get the repo root directory
    let repo_root = get_repo_root()?;
    let test_roms_dir = repo_root.join("external").join("test-roms");

    // Build gambatte ROMs from source (idempotent, only rebuilds stale/missing)
    build_gambatte_roms(&repo_root).context("Failed to build gambatte ROMs from source")?;

    // Check if other ROMs (blargg, mooneye, etc.) are already present
    if roms_already_downloaded(&test_roms_dir) {
        return Ok(());
    }

    // Download non-gambatte ROMs
    println!("cargo:warning=Downloading Game Boy test ROMs v{VERSION}...");
    download_and_extract_roms(&test_roms_dir).context("Failed to download test ROMs")?;
    println!("cargo:warning=Test ROMs downloaded successfully!");

    Ok(())
}

/// Get the path to the repo root directory
fn get_repo_root() -> Result<PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR not set")?;

    let repo_root = PathBuf::from(manifest_dir)
        .parent()
        .context("Failed to get parent directory")?
        .to_path_buf();

    Ok(repo_root)
}

/// Build gambatte test ROMs from .asm sources
fn build_gambatte_roms(repo_root: &Path) -> Result<()> {
    let script_path = repo_root
        .join("ceres-test-runner")
        .join("scripts")
        .join("build_gambatte_roms.py");

    if !script_path.exists() {
        // Script not present — skip (gambatte tests will fail with "ROM not found")
        return Ok(());
    }

    let output = Command::new("python3")
        .arg(&script_path)
        .output()
        .context("Failed to execute build_gambatte_roms.py")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "build_gambatte_roms.py exited with status: {}\nstdout: {}\nstderr: {}",
            output.status,
            stdout,
            stderr
        );
    }

    Ok(())
}

/// Check if non-gambatte test ROMs are already downloaded
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
