//! Translation of dmg-sim's cell library (SystemVerilog) to VHDL.

mod ast;
mod lexer;
mod parser;
mod vhdl;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

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
    let mut files: Vec<PathBuf> = Vec::new();
    for dir in ["dmg_cpu_b/cells", "sm83/cells"] {
        let dir = dmg_sim.join(dir);
        for e in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "sv") {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut lib = Library {
        packages: Vec::new(),
        modules: HashMap::new(),
        order: Vec::new(),
        unparsed: Vec::new(),
    };
    for f in files {
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
        for u in units {
            match u {
                parser::Unit::Package(pkg) => lib.packages.push(pkg),
                parser::Unit::Module(m) => {
                    lib.order.push(m.name.clone());
                    lib.modules.insert(m.name.clone(), m);
                }
            }
        }
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

/// The packages in an order where each comes after the ones it refers to.
fn package_order(pkgs: &[ast::Package]) -> Vec<&ast::Package> {
    fn refs(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Scoped(p, _) => out.push(p.clone()),
            Expr::Call(f, args) => {
                if let Some((p, _)) = f.split_once("::") {
                    out.push(p.to_owned());
                }
                args.iter().flatten().for_each(|a| refs(a, out));
            }
            Expr::Unary(_, a) => refs(a, out),
            Expr::Binary(_, a, b) | Expr::Index(a, b) => {
                refs(a, out);
                refs(b, out);
            }
            Expr::Cond(a, b, c) => {
                refs(a, out);
                refs(b, out);
                refs(c, out);
            }
            Expr::Concat(v) => v.iter().for_each(|a| refs(a, out)),
            Expr::Ident(_) | Expr::Num(_) => {}
        }
    }
    let mut done: Vec<&ast::Package> = Vec::new();
    let mut left: Vec<&ast::Package> = pkgs.iter().collect();
    while !left.is_empty() {
        let before = left.len();
        left.retain(|p| {
            let mut r = Vec::new();
            p.params.iter().for_each(|(_, e)| refs(e, &mut r));
            let ready = r.iter().all(|n| {
                n == &p.name
                    || !pkgs.iter().any(|q| &q.name == n)
                    || done.iter().any(|q| &q.name == n)
            });
            if ready {
                done.push(p);
            }
            !ready
        });
        if left.len() == before {
            // A cycle: keep the rest in file order.
            done.append(&mut left);
        }
    }
    done
}

/// The VHDL of the cell library: packages, entities and architectures, in an
/// order that can be analysed. Returns the text and the cells that could not
/// be translated, with the reason.
pub fn translate(lib: &Library) -> (String, Vec<(String, String)>) {
    let mut packages = String::new();
    let mut entities = String::new();
    let mut archs = String::new();
    let mut failed = Vec::new();
    for p in package_order(&lib.packages) {
        match vhdl::package(p) {
            Ok(s) => {
                packages.push_str(&s);
                packages.push('\n');
            }
            Err(e) => failed.push((p.name.clone(), format!("{e:#}"))),
        }
    }
    for name in &lib.order {
        if HAND_WRITTEN.contains(&name.as_str()) {
            continue;
        }
        let m = &lib.modules[name];
        match vhdl::entity(m).and_then(|e| Ok((e, vhdl::architecture(m, &lib.modules)?))) {
            Ok((e, a)) => {
                entities.push_str(&e);
                entities.push('\n');
                archs.push_str(&a);
                archs.push('\n');
            }
            Err(e) => failed.push((name.clone(), format!("{e:#}"))),
        }
    }
    let header = "-- Generated by ceres-hdlgen from the cells of dmg-sim (CC-BY-SA 4.0).\n-- Do not edit.\n\n";
    (format!("{header}{packages}{entities}{archs}"), failed)
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
        let (vhdl, failed) = translate(&lib);
        assert!(failed.is_empty(), "{failed:?}");
        vhdl
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
