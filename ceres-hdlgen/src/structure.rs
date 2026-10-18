//! The structural VHDL of a netlist: one entity whose architecture
//! instantiates a cell for each cell of the netlist and a signal for each
//! wire.
//!
//! The wire lengths (`L_*`) and the gate widths of the loads (`W_gate_*`)
//! passed to the cells are computed as nlconv computes them for dmg-sim, so
//! that the delays are the same: the length of the wire from its drawing on
//! the die (single-precision floats), plus the lengths of the cell pins.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{Context as _, Result, bail};

use crate::names::{prefixed, real_literal, vhdl_id};
use crate::netlist::{Dir, Netlist, Wire, format_f32, index_of, sv_name};
use crate::sv;

/// Direction of a port of the generated entity.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PortDir {
    Input,
    Output,
    Inout,
}

struct ModPort {
    /// Name in the netlist (`d[0]`).
    name: String,
    dir: PortDir,
    wire: usize,
}

struct Ctx<'a> {
    nl: &'a Netlist,
    module: String,
    cell_prefix: String,
    noprefix: Vec<String>,
    port_cell: String,
    lconv: f32,
}

impl Ctx<'_> {
    fn entity_of(&self, cell_type: &str) -> String {
        if self.noprefix.iter().any(|t| t == cell_type) {
            sv_name(cell_type, "")
        } else {
            sv_name(cell_type, &self.cell_prefix)
        }
    }

    /// `pkg::L_port` terms for the cell pins on a wire, and `pkg::W_gate_port`
    /// for the inputs among them.
    fn pin_terms(&self, w: &Wire) -> (Vec<String>, Vec<String>) {
        let mut wcs: Vec<_> = w.sources.iter().collect();
        wcs.extend(w.drains.iter().filter(|c| !w.sources.contains(c)));
        let (mut l, mut g) = (Vec::new(), Vec::new());
        for c in wcs {
            let Some(cell) = self.nl.cell(&c.cell) else {
                continue;
            };
            if cell.is_virtual {
                continue;
            }
            let pkg = format!(
                "work.{}",
                vhdl_id(&format!("{}_param", self.entity_of(&cell.type_name)))
            );
            let dir = self.nl.dir(c);
            l.push(format!("{pkg}.{}", vhdl_id(&sv_name(&c.port, "L_"))));
            if matches!(dir, Dir::In | Dir::Inout) {
                g.push(format!("{pkg}.{}", vhdl_id(&sv_name(&c.port, "W_gate_"))));
            }
        }
        (l, g)
    }

    /// Length of the wire on the die: the `L` attribute, or the length of
    /// its drawing; plus `L_extra`.
    #[expect(clippy::suboptimal_flops, reason = "must round like nlconv")]
    fn length(&self, w: &Wire) -> Result<(f32, bool)> {
        let given = w.attribs.get("L").map(|l| l.parse::<f32>()).transpose();
        let given = given.with_context(|| format!("wire {}: attribute L", w.name))?;
        let mut sum = given.unwrap_or(0.0);
        if given.is_none() {
            for l in &w.coords {
                let mut prev = (l[0], l[1]);
                for &[y, x] in l[2..].as_chunks::<2>().0 {
                    sum += Self::distance(prev, (y, x)) * self.lconv;
                    prev = (y, x);
                }
            }
        }
        if let Some(e) = w.attribs.get("L_extra") {
            let e: f32 = e
                .parse()
                .with_context(|| format!("wire {}: attribute L_extra", w.name))?;
            sum += e;
        }
        Ok((sum, given.is_some()))
    }

    /// The distance between two points as nlconv computes it (.NET's
    /// `Vector2.Distance`): a fused multiply-add or `hypot` would change the
    /// last bit of some lengths.
    #[expect(clippy::imprecise_flops, reason = "must round like nlconv")]
    fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
        let (dy, dx) = (b.0 - a.0, b.1 - a.1);
        (dx * dx + dy * dy).sqrt()
    }

    fn w_extra(w: &Wire) -> Result<f32> {
        w.attribs.get("W_gate_extra").map_or(Ok(0.0), |s| {
            s.parse()
                .with_context(|| format!("wire {}: attribute W_gate_extra", w.name))
        })
    }
}

/// Generates the VHDL of a netlist module (package of default lengths,
/// entity, and architecture `gates`).
#[expect(
    clippy::too_many_lines,
    reason = "follows nlconv's ToSystemVerilog step by step"
)]
pub fn emit(nl: &Netlist) -> Result<String> {
    let strings = &nl.strings;
    let module = strings
        .get("hdl-module")
        .cloned()
        .unwrap_or_else(|| "top".to_owned());
    let ctx = Ctx {
        nl,
        cell_prefix: strings
            .get("hdl-cell-prefix")
            .cloned()
            .unwrap_or_else(|| format!("{module}_")),
        noprefix: strings
            .get("hdl-noprefix-cells")
            .map(|s| s.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default(),
        port_cell: strings
            .get("hdl-port")
            .cloned()
            .unwrap_or_else(|| "port".to_owned()),
        lconv: strings
            .get("hdl-length-conv")
            .map_or(Ok(1.0), |s| s.parse())
            .context("hdl-length-conv")?,
        module,
    };
    let real_signals: Vec<&str> = strings
        .get("hdl-real-signals")
        .map(|s| s.split_whitespace().collect())
        .unwrap_or_default();

    let is_real: Vec<bool> = nl
        .wires
        .iter()
        .map(|w| real_signals.contains(&w.signal.as_str()))
        .collect();

    // Ports of the module: the connections of the wires to the port cell.
    let mut ports: Vec<ModPort> = Vec::new();
    let mut wire_ports: Vec<Vec<usize>> = vec![Vec::new(); nl.wires.len()];
    for (wi, w) in nl.wires.iter().enumerate() {
        for c in &w.sources {
            if c.cell != ctx.port_cell {
                continue;
            }
            let dir = match nl.dir(c) {
                Dir::Inout => PortDir::Inout,
                Dir::Out | Dir::Tri | Dir::Out0 | Dir::Out1 => PortDir::Input,
                d => bail!("port {}: invalid direction {d:?}", c.port),
            };
            wire_ports[wi].push(ports.len());
            ports.push(ModPort {
                name: c.port.clone(),
                dir,
                wire: wi,
            });
        }
        for c in &w.drains {
            if c.cell != ctx.port_cell {
                continue;
            }
            match nl.dir(c) {
                Dir::In => {
                    wire_ports[wi].push(ports.len());
                    ports.push(ModPort {
                        name: c.port.clone(),
                        dir: PortDir::Output,
                        wire: wi,
                    });
                }
                Dir::Inout => {}
                d => bail!("port {}: invalid direction {d:?}", c.port),
            }
        }
    }

    // Wires driven only by tri-state outputs keep their value with a bus
    // keeper; for a wire on an inout port, the keeper belongs to the level
    // above. The bits of a (0-based) vector all get one if one needs it.
    let mut keeper: Vec<bool> = nl
        .wires
        .iter()
        .enumerate()
        .map(|(wi, w)| {
            !w.sources.is_empty()
                && !is_real[wi]
                && !w
                    .sources
                    .iter()
                    .any(|c| matches!(nl.dir(c), Dir::Out | Dir::Out0 | Dir::Out1))
                && !wire_ports[wi]
                    .iter()
                    .any(|&p| ports[p].dir == PortDir::Inout)
        })
        .collect();
    let mut vectors: HashMap<String, Vec<usize>> = HashMap::new();
    for (wi, w) in nl.wires.iter().enumerate() {
        if !is_real[wi] && index_of(&w.name).is_some() {
            let base = sv_name(&w.name[..w.name.rfind('[').unwrap_or(w.name.len())], "");
            vectors.entry(base).or_default().push(wi);
        }
    }
    for bits in vectors.values() {
        if bits
            .iter()
            .filter_map(|&wi| index_of(&nl.wires[wi].name))
            .min()
            != Some(0)
        {
            continue;
        }
        if bits.iter().any(|&wi| keeper[wi]) {
            for &wi in bits {
                keeper[wi] = true;
            }
        }
    }

    let mut s = String::new();
    let module_id = vhdl_id(&ctx.module);
    writeln!(
        s,
        "-- Generated by ceres-hdlgen from the netlist of dmg-schematics (CC-BY-SA 4.0)."
    )?;
    writeln!(s, "-- Do not edit.\n")?;

    // Default lengths and gate widths of the module ports.
    writeln!(
        s,
        "package {} is",
        vhdl_id(&format!("{}_param", ctx.module))
    )?;
    for p in &ports {
        let w = &nl.wires[p.wire];
        let (l_terms, w_terms) = ctx.pin_terms(w);
        let (sum, _) = ctx.length(w)?;
        let l = match (sum != 0.0, l_terms.is_empty()) {
            (true, false) => format!(
                "{} + {}",
                real_literal(&format_f32(sum)),
                l_terms.join(" + ")
            ),
            (true, true) => real_literal(&format_f32(sum)),
            (false, false) => l_terms.join(" + "),
            (false, true) => "0.0".to_owned(),
        };
        writeln!(
            s,
            "  constant {} : real := {l};",
            vhdl_id(&sv_name(&p.name, "L_"))
        )?;
        if p.dir == PortDir::Output {
            continue;
        }
        let extra = Ctx::w_extra(w)?;
        let g = match (extra != 0.0, w_terms.is_empty()) {
            (true, false) => format!(
                "{} + {}",
                real_literal(&format_f32(extra)),
                w_terms.join(" + ")
            ),
            (true, true) => real_literal(&format_f32(extra)),
            (false, false) => w_terms.join(" + "),
            (false, true) => "0.0".to_owned(),
        };
        writeln!(
            s,
            "  constant {} : real := {g};",
            vhdl_id(&sv_name(&p.name, "W_gate_"))
        )?;
    }
    writeln!(s, "end package;\n")?;

    // Entity
    writeln!(s, "library ieee;\nuse ieee.std_logic_1164.all;\n")?;
    writeln!(s, "entity {module_id} is")?;
    let pkg = format!("work.{}", vhdl_id(&format!("{}_param", ctx.module)));
    let generics: Vec<String> = ports
        .iter()
        .filter(|p| p.dir != PortDir::Input)
        .flat_map(|p| {
            let l = vhdl_id(&sv_name(&p.name, "L_"));
            let g = vhdl_id(&sv_name(&p.name, "W_gate_"));
            let gdef = if p.dir == PortDir::Output {
                "0.0".to_owned()
            } else {
                format!("{pkg}.{g}")
            };
            [
                format!("{l} : real := {pkg}.{l}"),
                format!("{g} : real := {gdef}"),
            ]
        })
        .collect();
    if !generics.is_empty() {
        writeln!(s, "  generic (\n    {}\n  );", generics.join(";\n    "))?;
    }
    let port_decls: Vec<String> = ports
        .iter()
        .map(|p| {
            let dir = match p.dir {
                PortDir::Input => "in",
                PortDir::Output => "out",
                PortDir::Inout => "inout",
            };
            let ty = if is_real[p.wire] { "real" } else { "std_logic" };
            format!("{} : {dir} {ty}", vhdl_id(&sv_name(&p.name, "")))
        })
        .collect();
    writeln!(s, "  port (\n    {}\n  );", port_decls.join(";\n    "))?;
    writeln!(s, "end entity;\n")?;

    // Architecture
    let hdl_name: Vec<String> = nl
        .wires
        .iter()
        .enumerate()
        .map(|(wi, w)| {
            wire_ports[wi].first().map_or_else(
                || prefixed("n_", &sv_name(&w.name, "")),
                |&p| vhdl_id(&sv_name(&ports[p].name, "")),
            )
        })
        .collect();
    let mut seen = HashSet::new();
    for n in &hdl_name {
        if !seen.insert(n.to_ascii_lowercase()) {
            bail!("two wires are called {n} in VHDL");
        }
    }
    writeln!(s, "library ieee;\nuse ieee.std_logic_1164.all;\n")?;
    writeln!(s, "architecture gates of {module_id} is")?;
    for (wi, n) in hdl_name.iter().enumerate() {
        if wire_ports[wi].is_empty() {
            // A wire with one plain output on it needs no resolution.
            let w = &nl.wires[wi];
            let single = w.sources.len() == 1 && nl.dir(&w.sources[0]) == Dir::Out && !keeper[wi];
            let ty = if is_real[wi] {
                "real"
            } else if single {
                "std_ulogic"
            } else {
                "std_logic"
            };
            writeln!(s, "  signal {n} : {ty};")?;
        }
    }
    writeln!(s, "begin")?;
    // A wire on several ports: the other output ports copy the first one.
    for (wi, ps) in wire_ports.iter().enumerate() {
        for &p in ps.iter().skip(1) {
            match ports[p].dir {
                PortDir::Output => writeln!(
                    s,
                    "  {} <= {};",
                    vhdl_id(&sv_name(&ports[p].name, "")),
                    hdl_name[wi]
                )?,
                _ => bail!(
                    "wire {} is connected to several input ports",
                    nl.wires[wi].name
                ),
            }
        }
    }

    for cell in &nl.cells {
        if cell.name == ctx.port_cell {
            continue;
        }
        let t = nl.type_of(cell);
        let mut generics = Vec::new();
        for p in &t.ports {
            if matches!(p.dir, Dir::In | Dir::NotConnected) {
                continue;
            }
            let Some(wi) = nl.wire_at(&cell.name, &p.name) else {
                continue;
            };
            let w = &nl.wires[wi];
            // A wire on an output or inout port of the module takes the
            // length and width of the port, which the level above passes.
            let mut port_par = None;
            let mut is_bidir = false;
            for &mp in &wire_ports[wi] {
                if ports[mp].dir == PortDir::Inout {
                    is_bidir = true;
                }
                if ports[mp].dir != PortDir::Input && port_par.is_none() {
                    port_par = Some(&ports[mp].name);
                }
            }
            let (sum, given) = ctx.length(w)?;
            if w.coords.is_empty() && !given && port_par.is_none() {
                continue;
            }
            let (l_terms, w_terms) = ctx.pin_terms(w);
            let l_formal = vhdl_id(&sv_name(&p.name, "L_"));
            let l = match port_par {
                Some(mp) => vhdl_id(&sv_name(mp, "L_")),
                None if l_terms.is_empty() => real_literal(&format_f32(sum)),
                None => format!(
                    "{} + {}",
                    real_literal(&format_f32(sum)),
                    l_terms.join(" + ")
                ),
            };
            generics.push(format!("{l_formal} => {l}"));
            if w_terms.is_empty() && port_par.is_none() {
                continue;
            }
            let extra = Ctx::w_extra(w)?;
            let mut terms = Vec::new();
            if let Some(mp) = port_par {
                terms.push(vhdl_id(&sv_name(mp, "W_gate_")));
            }
            if !is_bidir && extra != 0.0 {
                terms.push(real_literal(&format_f32(extra)));
            }
            if !is_bidir || port_par.is_none() {
                terms.extend(w_terms);
            }
            let w_formal = vhdl_id(&sv_name(&p.name, "W_gate_"));
            generics.push(format!(
                "{w_formal} => {}",
                if terms.is_empty() {
                    "0.0".to_owned()
                } else {
                    terms.join(" + ")
                }
            ));
        }
        for (k, v) in &cell.attribs {
            if let Some(name) = k.strip_prefix("sv:") {
                let e = sv::parse_expr(v)
                    .with_context(|| format!("cell {}: attribute {k}", cell.name))?;
                generics.push(format!(
                    "{} => {}",
                    vhdl_id(name.trim()),
                    sv::real_expr(&e)?
                ));
            }
        }
        let mut actuals = Vec::new();
        for p in &t.ports {
            let formal = vhdl_id(&sv_name(&p.name, ""));
            let actual = match nl.wire_at(&cell.name, &p.name) {
                Some(wi) => hdl_name[wi].clone(),
                None if p.dir == Dir::In => "'Z'".to_owned(),
                None if p.dir == Dir::NotConnected => continue,
                None => "open".to_owned(),
            };
            actuals.push(format!("{formal} => {actual}"));
        }
        writeln!(
            s,
            "  {} : entity work.{}",
            prefixed("u_", &sv_name(&cell.name, "")),
            vhdl_id(&ctx.entity_of(&cell.type_name))
        )?;
        if !generics.is_empty() {
            writeln!(
                s,
                "    generic map (\n      {}\n    )",
                generics.join(",\n      ")
            )?;
        }
        if actuals.is_empty() {
            // Only for cells without ports (protection diodes).
            s.truncate(s.trim_end().len());
            s.push_str(";\n");
        } else {
            writeln!(
                s,
                "    port map (\n      {}\n    );",
                actuals.join(",\n      ")
            )?;
        }
    }

    for (wi, &k) in keeper.iter().enumerate() {
        if k {
            writeln!(
                s,
                "  {} : entity work.keeper port map (n => {});",
                prefixed("k_", &sv_name(&nl.wires[wi].name, "")),
                hdl_name[wi]
            )?;
        }
    }
    writeln!(s, "end architecture;")?;
    Ok(s)
}
