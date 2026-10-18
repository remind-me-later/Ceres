//! Generates the gate-level VHDL model of the Game Boy (`hdl/`).
//!
//! The structure comes from the netlist of the DMG-CPU B traced from the die
//! (`external/docs/dmg-schematics/netlist`), the cell models and their
//! delays from dmg-sim (`external/reference-implementations/dmg-sim`).
//!
//! ```text
//! ceres-hdlgen [--netlist DIR] [--dmg-sim DIR] [--out DIR]
//! ```

mod names;
mod netlist;
mod structure;
mod sv;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

/// A generated file: its path in the output directory and its text.
pub struct File {
    pub path: String,
    pub text: String,
}

/// Writes generated files under `out`, and their list in `list`, in order.
fn write_files(out: &Path, list: &str, files: &[File]) -> Result<()> {
    let mut order = String::new();
    for f in files {
        let path = out.join(&f.path);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::write(&path, &f.text).with_context(|| format!("writing {}", path.display()))?;
        order.push_str(&f.path);
        order.push('\n');
    }
    fs::write(out.join(list), order)?;
    Ok(())
}

struct Args {
    netlist: PathBuf,
    dmg_sim: PathBuf,
    out: PathBuf,
}

fn args() -> Result<Args> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut a = Args {
        netlist: root.join("external/docs/dmg-schematics/netlist"),
        dmg_sim: root.join("external/reference-implementations/dmg-sim"),
        out: root.join("hdl/build/gen"),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().with_context(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--netlist" => a.netlist = value()?.into(),
            "--dmg-sim" => a.dmg_sim = value()?.into(),
            "--out" => a.out = value()?.into(),
            _ => bail!("unknown argument {arg}"),
        }
    }
    Ok(a)
}

fn main() -> Result<()> {
    let a = args()?;
    fs::create_dir_all(&a.out).with_context(|| format!("creating {}", a.out.display()))?;

    // dmg-sim's defaults: the simplified OAM and wave RAM.
    let defines: HashSet<&str> = ["SIMPLIFIED_OAM", "SIMPLIFIED_WAVERAM"]
        .into_iter()
        .collect();
    let lib = sv::read_library(&a.dmg_sim, &defines)?;
    // Earlier layouts: one file for the cells, one for each module.
    for old in [
        "cells.vhd",
        "sm83.vhd",
        "dmg_cpu_b.vhd",
        "cells",
        "sm83",
        "dmg_cpu_b",
    ] {
        let p = a.out.join(old);
        if p.is_dir() {
            fs::remove_dir_all(&p).with_context(|| format!("removing {}", p.display()))?;
        } else if p.exists() {
            fs::remove_file(&p).with_context(|| format!("removing {}", p.display()))?;
        }
    }
    let (cells, failed) = sv::translate(&lib);
    write_files(&a.out, "cells.txt", &cells)?;
    for (name, why) in lib.unparsed.iter().chain(&failed) {
        eprintln!("not translated: {name}: {why}");
    }
    eprintln!(
        "{} cells, {} files not parsed, {} not translated",
        lib.order.len(),
        lib.unparsed.len(),
        failed.len()
    );

    // The SM83 core, then the chip, as nlconv makes them for dmg-sim (the
    // file lists of the netlist Makefile; the chip with the tag `codegen`).
    // Each in its blocks; chip.txt lists the files in the order they can be
    // analysed.
    let makefile = a.netlist.join("Makefile");
    let mut chip = Vec::new();
    for (var, conds, module) in [
        ("SM83_NETLIST_FILES", &[][..], "sm83"),
        ("NETLIST_FILES", &["codegen"][..], "dmg_cpu_b"),
    ] {
        let files: Vec<PathBuf> = netlist::makefile_list(&makefile, var)?
            .iter()
            .map(|f| a.netlist.join(f))
            .collect();
        let nl = netlist::Netlist::read(&files, conds)?;
        let vhdl = structure::emit(&nl).with_context(|| format!("generating {module}"))?;
        eprintln!(
            "{module}: {} cells, {} wires, {} blocks",
            nl.cells.len(),
            nl.wires.len(),
            vhdl.len() - 2
        );
        chip.extend(vhdl);
    }
    write_files(&a.out, "chip.txt", &chip)?;
    Ok(())
}
