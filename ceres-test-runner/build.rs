//! Build script for the Game Boy test ROMs.
//!
//! Downloads the whole `c-sp/gameboy-test-roms` release (all the suites the
//! tests use) into `external/test-roms` with `curl` and `unzip`, unless the
//! Mooneye and Wilbertpol ROMs are already there. It only reruns when this
//! file changes: to download again, delete the directory and run `cargo
//! clean --package ceres-test-runner`.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

const REPO: &str = "c-sp/gameboy-test-roms";
const VERSION: &str = "v7.0";

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    let repo_root = get_repo_root()?;
    let test_roms_dir = repo_root.join("external").join("test-roms");

    if roms_already_downloaded(&test_roms_dir) {
        return Ok(());
    }

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

/// Check if the mooneye/wilbertpol test ROMs are already downloaded
fn roms_already_downloaded(test_roms_dir: &std::path::Path) -> bool {
    let mooneye = test_roms_dir
        .join("mooneye-test-suite")
        .join("acceptance")
        .join("add_sp_e_timing.gb");
    let wilbertpol = test_roms_dir
        .join("mooneye-test-suite-wilbertpol")
        .join("acceptance")
        .join("add_sp_e_timing.gb");

    mooneye.exists() && wilbertpol.exists()
}

/// Download and extract test ROMs using curl and unzip
fn download_and_extract_roms(test_roms_dir: &std::path::Path) -> Result<()> {
    let url = format!(
        "https://github.com/{REPO}/releases/download/{VERSION}/game-boy-test-roms-{VERSION}.zip"
    );

    println!("cargo:warning=Downloading from: {url}");

    std::fs::create_dir_all(test_roms_dir).context("Failed to create test-roms directory")?;

    let temp_zip = test_roms_dir.with_file_name("test-roms-temp.zip");

    let download_status = Command::new("curl")
        .arg("-L")
        .arg("-f")
        .arg("-o")
        .arg(&temp_zip)
        .arg(&url)
        .status()
        .context("Failed to execute curl")?;

    if !download_status.success() {
        anyhow::bail!("Download failed with curl exit code: {download_status}");
    }

    println!("cargo:warning=Extracting test ROMs...");

    let extract_status = Command::new("unzip")
        .arg("-q")
        .arg("-o")
        .arg(&temp_zip)
        .arg("-d")
        .arg(test_roms_dir)
        .status()
        .context("Failed to execute unzip")?;

    if !extract_status.success() {
        anyhow::bail!("Extraction failed with unzip exit code: {extract_status}");
    }

    let _ = std::fs::remove_file(&temp_zip);

    Ok(())
}
