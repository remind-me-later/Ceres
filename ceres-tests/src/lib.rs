//! Integration tests for the Ceres Game Boy emulator
//!
//! This crate provides infrastructure for running Game Boy test ROMs
//! and validating the emulator's accuracy.

pub mod test_runner;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Get the path to the test-roms directory
#[inline]
pub fn test_roms_dir() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("test-roms")
}

/// Check if test ROMs are available
#[inline]
pub fn test_roms_available() -> bool {
    let test_roms = test_roms_dir();
    test_roms.exists() && test_roms.is_dir()
}

/// Load a test ROM file
#[inline]
pub fn load_test_rom(relative_path: &str) -> Result<Vec<u8>> {
    let rom_path = test_roms_dir().join(relative_path);

    if !rom_path.exists() {
        anyhow::bail!(
            "Test ROM not found: {}\n\n\
             Please download test ROMs by running:\n\
             cd test-roms && ./download-test-roms.sh",
            rom_path.display()
        );
    }

    std::fs::read(&rom_path)
        .with_context(|| format!("Failed to read test ROM: {}", rom_path.display()))
}

/// List all available test ROMs in a directory
#[inline]
pub fn list_test_roms(dir: &str) -> Result<Vec<PathBuf>> {
    fn collect_roms(dir: &Path, roms: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension()
                    && (ext == "gb" || ext == "gbc")
                {
                    roms.push(path);
                }
            } else if path.is_dir() {
                collect_roms(&path, roms)?;
            } else {
                // Ignore other types (symlinks, etc.)
            }
        }
        Ok(())
    }

    let test_dir = test_roms_dir().join(dir);

    if !test_dir.exists() {
        return Ok(Vec::new());
    }

    let mut roms = Vec::new();

    collect_roms(&test_dir, &mut roms)?;
    roms.sort();

    Ok(roms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roms_directory_exists() {
        let dir = test_roms_dir();
        assert!(
            dir.exists(),
            "test-roms directory not found at: {}\n\
             Please run: cd test-roms && ./download-test-roms.sh",
            dir.display()
        );
    }

    #[test]
    #[ignore = "Only run if test ROMs are downloaded"]
    fn test_can_list_blargg_roms() {
        let roms = list_test_roms("blargg").expect("Failed to list ROMs");
        assert!(!roms.is_empty(), "No Blargg test ROMs found");
    }
}
