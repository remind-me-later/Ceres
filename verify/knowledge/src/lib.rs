//! The knowledge base's sources, read and checked: the facts about the
//! hardware (`facts.toml`), the races of the readable model
//! (`races.toml`), the rules it follows beyond the die (`rules.toml`) and
//! what the proofs against the netlist assume (`assumptions.toml`), all in
//! this folder. The binary prints lists from them and writes them as tables
//! for the Datalog program (`knowledge.sh`, `knowledge.dl`).

pub mod assumptions;
pub mod facts;
pub mod races;
pub mod rules;

use std::path::PathBuf;

/// The test ROMs, which the facts' evidence names.
#[must_use]
pub fn test_roms_dir() -> PathBuf {
    facts::repo_root().join("external").join("test-roms")
}
