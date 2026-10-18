//! Translation of dmg-sim's cell library (SystemVerilog) to VHDL.

mod ast;
mod lexer;
mod parser;
mod vhdl;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use crate::File;

pub use ast::{Expr, Module};
pub use vhdl::real_expr;

/// Parses a SystemVerilog expression (the `sv:` attributes of the netlist).
pub fn parse_expr(src: &str) -> Result<Expr> {
    let mut p = parser::Parser::new(lexer::lex(src, &HashSet::new())?);
    let e = p.expr()?;
    if !p.at_end() {
        anyhow::bail!("trailing tokens in expression {src}");
    }
    Ok(e)
}

/// The parsed cell library.
pub struct Library {
    pub packages: Vec<ast::Package>,
    pub modules: HashMap<String, Module>,
    /// Module names in file order.
    pub order: Vec<String>,
    /// Files that could not be parsed, with the reason.
    pub unparsed: Vec<(String, String)>,
    /// The source files, in order, with what they define.
    pub sources: Vec<Source>,
}

/// A cell file of dmg-sim (`dmg_cpu_b/cells/nand2.sv`).
pub struct Source {
    /// `dmg` or `sm83`.
    pub group: &'static str,
    /// The file name without `.sv`.
    pub stem: String,
    pub packages: Vec<String>,
    pub modules: Vec<String>,
}

/// Cells that are written by hand in VHDL (`hdl/gate/cells`): memories,
/// ROMs and the analog audio, which use arrays, loops and real numbers
/// beyond the subset the translator handles.
pub const HAND_WRITTEN: &[&str] = &[
    "dmg_audio",
    "dmg_boot_rom",
    "dmg_generic_sram",
    "dmg_high_ram",
    "dmg_oam",
    "dmg_simplified_sram",
    "dmg_wave_ram",
];

/// Reads the cell files of dmg-sim (`dmg_cpu_b/cells` and `sm83/cells`).
pub fn read_library(dmg_sim: &Path, defines: &HashSet<&str>) -> Result<Library> {
    let mut files: Vec<(&'static str, PathBuf)> = Vec::new();
    for (group, dir) in [("dmg", "dmg_cpu_b/cells"), ("sm83", "sm83/cells")] {
        let dir = dmg_sim.join(dir);
        for e in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "sv") {
                files.push((group, p));
            }
        }
    }
    files.sort();
    let mut lib = Library {
        packages: Vec::new(),
        modules: HashMap::new(),
        order: Vec::new(),
        unparsed: Vec::new(),
        sources: Vec::new(),
    };
    for (group, f) in files {
        let src = fs::read_to_string(&f).with_context(|| format!("reading {}", f.display()))?;
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Keep what parses before an error: the package of a hand-written
        // cell is still generated.
        let mut units = Vec::new();
        let result = lexer::lex(&src, defines).and_then(|toks| {
            let mut p = parser::Parser::new(toks);
            while !p.at_end() {
                units.push(p.unit()?);
            }
            Ok(())
        });
        let mut source = Source {
            group,
            stem: name.strip_suffix(".sv").unwrap_or(&name).to_owned(),
            packages: Vec::new(),
            modules: Vec::new(),
        };
        for u in units {
            match u {
                parser::Unit::Package(pkg) => {
                    source.packages.push(pkg.name.clone());
                    lib.packages.push(pkg);
                }
                parser::Unit::Module(m) => {
                    source.modules.push(m.name.clone());
                    lib.order.push(m.name.clone());
                    lib.modules.insert(m.name.clone(), m);
                }
            }
        }
        lib.sources.push(source);
        if let Err(e) = result
            && !HAND_WRITTEN
                .iter()
                .any(|h| name.strip_suffix(".sv").is_some_and(|n| h.ends_with(n)))
        {
            lib.unparsed.push((name, format!("{e:#}")));
        }
    }
    Ok(lib)
}

/// The VHDL of the cell library: a file for each cell file of dmg-sim, with
/// its packages, entities and architectures, in an order that can be
/// analysed (a file after the files it uses). Returns the files and the
/// cells that could not be translated, with the reason.
pub fn translate(lib: &Library) -> (Vec<File>, Vec<(String, String)>) {
    let mut failed = Vec::new();
    // Each file with the names it defines, in lower case.
    let mut files: Vec<(File, Vec<String>)> = Vec::new();
    for src in &lib.sources {
        let mut body = String::new();
        let mut defines = Vec::new();
        for p in lib
            .packages
            .iter()
            .filter(|p| src.packages.contains(&p.name))
        {
            match vhdl::package(p) {
                Ok(s) => {
                    body.push_str(&s);
                    body.push('\n');
                    defines.push(p.name.to_ascii_lowercase());
                }
                Err(e) => failed.push((p.name.clone(), format!("{e:#}"))),
            }
        }
        for name in &src.modules {
            if HAND_WRITTEN.contains(&name.as_str()) {
                continue;
            }
            let m = &lib.modules[name];
            match vhdl::entity(m).and_then(|e| Ok((e, vhdl::architecture(m, &lib.modules)?))) {
                Ok((e, a)) => {
                    body.push_str(&e);
                    body.push('\n');
                    body.push_str(&a);
                    body.push('\n');
                    defines.push(name.to_ascii_lowercase());
                }
                Err(e) => failed.push((name.clone(), format!("{e:#}"))),
            }
        }
        if body.is_empty() {
            continue;
        }
        let dir = if src.group == "dmg" {
            "dmg_cpu_b"
        } else {
            "sm83"
        };
        let text = format!(
            "-- Generated by ceres-hdlgen from dmg-sim's {dir}/cells/{}.sv (CC-BY-SA 4.0).\n\
             -- Do not edit.\n\n{body}",
            src.stem
        );
        let path = format!("cells/{}/{}.vhd", src.group, src.stem);
        files.push((File { path, text }, defines));
    }
    (analysis_order(files), failed)
}

/// The files in an order where each comes after the files that define a
/// name it uses, otherwise in their order.
fn analysis_order(files: Vec<(File, Vec<String>)>) -> Vec<File> {
    let uses: Vec<HashSet<String>> = files
        .iter()
        .map(|(f, _)| {
            f.text
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .collect();
    let deps: Vec<Vec<usize>> = (0..files.len())
        .map(|i| {
            (0..files.len())
                .filter(|&j| j != i && files[j].1.iter().any(|n| uses[i].contains(n)))
                .collect()
        })
        .collect();
    let mut done = vec![false; files.len()];
    let mut order = Vec::with_capacity(files.len());
    while order.len() < files.len() {
        // The first file whose dependencies are done; on a cycle, the first
        // file left.
        let next = (0..files.len())
            .find(|&i| !done[i] && deps[i].iter().all(|&j| done[j]))
            .or_else(|| (0..files.len()).find(|&i| !done[i]))
            .unwrap_or_default();
        done[next] = true;
        order.push(next);
    }
    let mut files: Vec<Option<File>> = files.into_iter().map(|(f, _)| Some(f)).collect();
    order.into_iter().filter_map(|i| files[i].take()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate_one(src: &str) -> String {
        let toks = lexer::lex(src, &HashSet::new()).unwrap_or_default();
        let mut p = parser::Parser::new(toks);
        let mut lib = Library {
            packages: Vec::new(),
            modules: HashMap::new(),
            order: Vec::new(),
            unparsed: Vec::new(),
            sources: Vec::new(),
        };
        while !p.at_end() {
            match p.unit() {
                Ok(parser::Unit::Module(m)) => {
                    lib.order.push(m.name.clone());
                    lib.modules.insert(m.name.clone(), m);
                }
                Ok(parser::Unit::Package(pkg)) => lib.packages.push(pkg),
                Err(e) => panic!("{e:#}"),
            }
        }
        lib.sources.push(Source {
            group: "dmg",
            stem: "test".to_owned(),
            packages: lib.packages.iter().map(|p| p.name.clone()).collect(),
            modules: lib.order.clone(),
        });
        let (files, failed) = translate(&lib);
        assert!(failed.is_empty(), "{failed:?}");
        files.into_iter().map(|f| f.text).collect()
    }

    #[test]
    fn path_output_reads_and_fusion() {
        let v = translate_one(
            "module inv(input logic a, output logic y, output logic y_n);
                assign y = !a;
                assign y_n = !y;
                specify
                    (a *> y) = (1.0, 2.0);
                    (a *> y_n) = (3.0, 4.0);
                endspecify
            endmodule",
        );
        // y is computed in its path process; y_n reads y after its delay.
        assert!(v.contains("vl_new := (not to_x01(a));"));
        assert!(v.contains("vl_new := (not to_x01(y));"));
        assert!(!v.contains("y_ud"));
        // A pending change is replaced at its time.
        assert!(v.contains("y <= vl_new after tm_pend - now;"));
    }

    #[test]
    fn latch_reads_value_before_the_path_delay() {
        let v = translate_one(
            "module lat(input logic d, en, output logic q, q_n);
                initial q = 0;
                always_latch if (en) q = d;
                assign q_n = !q;
                specify
                    (d, en *> q) = (1.0, 1.0);
                endspecify
            endmodule",
        );
        assert!(v.contains("signal q_ud : std_logic := '0';"));
        assert!(v.contains("q_ud <= to_x01(d);"));
        // The continuous assignment reads the port, after the delay.
        assert!(v.contains("vl_new := (not to_x01(q));"));
    }

    #[test]
    fn tri_state_gates_and_strengths() {
        let v = translate_one(
            "module pad(input logic d, ena, output tri logic y);
                bufif1 (highz1, strong0) #(1.0, 2.0, 0.5) (y, d, ena);
            endmodule",
        );
        assert!(v.contains("y : out std_logic := 'Z'"));
        assert!(v.contains("drive(bufif1_v(to_x01(d), to_x01(ena)), highz, strong)"));
        assert!(v.contains("after gdly(vl_new,"));
    }
}
