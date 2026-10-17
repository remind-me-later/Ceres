//! VHDL for dmg-sim's cells.
//!
//! The cells are simulated by dmg-sim with Icarus Verilog, and their delays
//! were fitted with it, so the VHDL copies how Icarus runs them:
//!
//! - A module path (`specify`) delays an output from the last change of the
//!   path source that changed most recently; on a tie the smaller delay
//!   wins, and a delay that has already passed applies at once. While a
//!   change of the output is pending, a new value replaces the pending one
//!   at the same time (it is not rescheduled, as an inertial delay would).
//! - Inside the cell, a continuous assignment reads such an output after its
//!   path delay (the port), but `always` blocks read the value before it.
//!   In VHDL the value before the delay is the signal `<output>_ud`.
//! - Gates and continuous assignments pick their delay by the new value,
//!   and filter pulses shorter than it (inertial delay), as VHDL does.
//! - Drive strengths map to `std_logic`: strong is `0`/`1`, pull and weak
//!   are `L`/`H`.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{Context as _, Result, bail};

use super::ast::{Edge, Expr, Item, Module, NetKind, Package, PortDir, Stmt, Strength};
use crate::names::{real_literal, vhdl_id};

/// The ports of the hand-written entities that translated cells instantiate.
pub fn builtin_ports(module: &str) -> Option<&'static [(&'static str, PortDir)]> {
    match module {
        "keeper" => Some(&[("n", PortDir::Inout)]),
        _ => None,
    }
}

/// VHDL for a real-valued expression (parameters and delays, in ns).
pub fn real_expr(e: &Expr) -> Result<String> {
    Ok(match e {
        Expr::Ident(n) => vhdl_id(n),
        Expr::Scoped(p, n) => format!("work.{}.{}", vhdl_id(p), vhdl_id(n)),
        Expr::Num(n) => real_number(n)?,
        Expr::Unary("-", a) => format!("(-{})", real_expr(a)?),
        Expr::Unary("+", a) => real_expr(a)?,
        Expr::Binary(op @ ("+" | "-" | "*" | "/"), a, b) => {
            format!("({} {op} {})", real_expr(a)?, real_expr(b)?)
        }
        Expr::Call(f, args) => {
            let args = args
                .iter()
                .map(|a| a.as_ref().map_or_else(|| Ok("0.0".to_owned()), real_expr))
                .collect::<Result<Vec<_>>>()?;
            let f = match f.split_once("::") {
                Some((p, n)) => format!("work.{}.{}", vhdl_id(p), vhdl_id(n)),
                None => vhdl_id(f),
            };
            format!("{f}({})", args.join(", "))
        }
        // VHDL-2008 has no conditional expression: rcond(cond, a, b).
        Expr::Cond(c, a, b) => format!(
            "rcond({}, {}, {})",
            real_cond(c)?,
            real_expr(a)?,
            real_expr(b)?
        ),
        _ => bail!("unsupported real expression {e:?}"),
    })
}

/// A comparison of reals, as a VHDL boolean.
fn real_cond(e: &Expr) -> Result<String> {
    match e {
        Expr::Binary(op @ ("<" | "<=" | ">" | ">="), a, b) => {
            Ok(format!("({} {op} {})", real_expr(a)?, real_expr(b)?))
        }
        Expr::Binary("==", a, b) => Ok(format!("({} = {})", real_expr(a)?, real_expr(b)?)),
        _ => bail!("unsupported real condition {e:?}"),
    }
}

/// A number in a real context: plain numbers, and time literals in ns.
fn real_number(n: &str) -> Result<String> {
    let n = n.replace('_', "");
    for (unit, scale) in [
        ("fs", 1e-6),
        ("ps", 1e-3),
        ("ns", 1.0),
        ("us", 1e3),
        ("ms", 1e6),
        ("s", 1e9),
    ] {
        if let Some(v) = n.strip_suffix(unit)
            && v.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
            let v: f64 = v.parse().with_context(|| format!("time literal {n}"))?;
            return Ok(real_literal(&format!("{}", v * scale)));
        }
    }
    if n.contains('\'') {
        bail!("based number {n} in a real expression");
    }
    Ok(real_literal(&n))
}

/// A 1-bit value: `0`, `1'b1`, `'z`.
fn bit_literal(n: &str) -> Option<char> {
    let digits = n.rsplit('\'').next().unwrap_or(n);
    let digits = digits.trim_start_matches(['b', 'B', 'h', 'H', 'd', 'D']);
    match digits {
        "0" => Some('0'),
        "1" => Some('1'),
        "z" | "Z" => Some('Z'),
        "x" | "X" => Some('X'),
        _ => None,
    }
}

const fn strength_name(s: Strength) -> &'static str {
    match s {
        Strength::Supply => "supply",
        Strength::Strong => "strong",
        Strength::Pull => "pull",
        Strength::Weak => "weak",
        Strength::HighZ => "highz",
    }
}

/// How a signal of the cell is read.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Read {
    /// By a continuous assignment, a gate or an instance: after the path
    /// delay of an output.
    Net,
    /// By an `always` block: before it.
    Procedural,
}

struct Cell<'a> {
    m: &'a Module,
    modules: &'a HashMap<String, Module>,
    /// Outputs with module paths.
    paths: HashSet<String>,
    signals: HashMap<String, NetKind>,
    /// Constants: local parameters and specify parameters.
    consts: HashSet<String>,
    /// Initial values of signals, by VHDL target name.
    init: HashMap<String, char>,
    decls: String,
    body: String,
    next_id: usize,
}

impl Cell<'_> {
    const fn id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id
    }

    fn target(&self, name: &str) -> String {
        if self.paths.contains(name) {
            format!("{}_ud", vhdl_id(name))
        } else {
            vhdl_id(name)
        }
    }

    fn read_name(&self, name: &str, read: Read) -> String {
        if read == Read::Procedural {
            self.target(name)
        } else {
            vhdl_id(name)
        }
    }

    /// VHDL for a 1-bit expression, as `std_ulogic`, and the signals it reads.
    fn logic(&self, e: &Expr, read: Read, sens: &mut Vec<String>) -> Result<String> {
        Ok(match e {
            Expr::Ident(n) => {
                if self.consts.contains(n) || self.m.params.iter().any(|p| &p.name == n) {
                    bail!("parameter {n} in a logic expression");
                }
                let s = self.read_name(n, read);
                if !sens.contains(&s) {
                    sens.push(s.clone());
                }
                format!("to_x01({s})")
            }
            Expr::Num(n) => match bit_literal(n) {
                Some(c) => format!("'{c}'"),
                None => bail!("number {n} in a logic expression"),
            },
            Expr::Unary("!" | "~", a) => format!("(not {})", self.logic(a, read, sens)?),
            Expr::Binary(op, a, b) => {
                let (a, b) = (self.logic(a, read, sens)?, self.logic(b, read, sens)?);
                match *op {
                    "&" | "&&" => format!("({a} and {b})"),
                    "|" | "||" => format!("({a} or {b})"),
                    "^" => format!("({a} xor {b})"),
                    "~^" | "^~" => format!("({a} xnor {b})"),
                    "==" => format!("veq({a}, {b})"),
                    "!=" => format!("vne({a}, {b})"),
                    "===" => format!("vceq({a}, {b})"),
                    "!==" => format!("(not vceq({a}, {b}))"),
                    _ => bail!("unsupported logic operator {op}"),
                }
            }
            Expr::Cond(c, a, b) => format!(
                "vmux({}, {}, {})",
                self.logic(c, read, sens)?,
                self.logic(a, read, sens)?,
                self.logic(b, read, sens)?
            ),
            _ => bail!("unsupported logic expression {e:?}"),
        })
    }

    fn lhs(&self, e: &Expr) -> Result<String> {
        match e {
            Expr::Ident(n) => Ok(self.target(n)),
            _ => bail!("unsupported assignment target {e:?}"),
        }
    }

    /// Delay constants for `#(...)` of a gate or an assignment, selected by
    /// the new value: rise, fall and turn-off (the smaller of rise and fall
    /// when not given).
    fn gate_delays(&mut self, delays: &[Expr]) -> Result<Option<[String; 3]>> {
        if delays.is_empty() {
            return Ok(None);
        }
        let k = self.id();
        let names = [format!("d{k}_r"), format!("d{k}_f"), format!("d{k}_z")];
        let exprs: Vec<String> = delays.iter().map(real_expr).collect::<Result<_>>()?;
        let (r, f) = (exprs[0].clone(), exprs.get(1).unwrap_or(&exprs[0]).clone());
        writeln!(
            self.decls,
            "  constant {} : delay_length := to_t({r});",
            names[0]
        )?;
        writeln!(
            self.decls,
            "  constant {} : delay_length := to_t({f});",
            names[1]
        )?;
        match exprs.get(2) {
            Some(z) => writeln!(
                self.decls,
                "  constant {} : delay_length := to_t({z});",
                names[2]
            )?,
            None => writeln!(
                self.decls,
                "  constant {} : delay_length := minimum({}, {});",
                names[2], names[0], names[1]
            )?,
        }
        if exprs.len() > 3 {
            bail!("more than three delays");
        }
        Ok(Some(names))
    }

    /// A process that drives `target` with `value` (computed from `sens`),
    /// after the gate delays.
    fn driver(
        &mut self,
        comment: &str,
        target: &str,
        value: &str,
        sens: &[String],
        delays: Option<[String; 3]>,
    ) -> Result<()> {
        writeln!(self.body, "  -- {comment}")?;
        if sens.is_empty() {
            writeln!(self.body, "  {target} <= {value};")?;
            return Ok(());
        }
        writeln!(self.body, "  process ({}) is", sens.join(", "))?;
        writeln!(self.body, "    variable vl_new : std_ulogic;")?;
        writeln!(self.body, "    variable vl_old : std_ulogic := 'U';")?;
        writeln!(self.body, "  begin")?;
        writeln!(self.body, "    vl_new := {value};")?;
        writeln!(self.body, "    if vl_new /= vl_old then")?;
        match delays {
            Some([r, f, z]) => writeln!(
                self.body,
                "      {target} <= vl_new after gdly(vl_new, {r}, {f}, {z});"
            )?,
            None => writeln!(self.body, "      {target} <= vl_new;")?,
        }
        writeln!(self.body, "      vl_old := vl_new;")?;
        writeln!(self.body, "    end if;")?;
        writeln!(self.body, "  end process;")?;
        Ok(())
    }

    fn is_real(&self, e: &Expr) -> bool {
        let Expr::Ident(n) = e else { return false };
        self.signals.get(n) == Some(&NetKind::Real)
            || self
                .m
                .ports
                .iter()
                .any(|p| &p.name == n && p.kind == NetKind::Real)
    }

    fn assign(
        &mut self,
        strength: Option<(Strength, Strength)>,
        delays: &[Expr],
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<()> {
        let target = self.lhs(lhs)?;
        if self.is_real(lhs) {
            if strength.is_some() || !delays.is_empty() {
                bail!("real assignment with strength or delay");
            }
            writeln!(self.body, "  {target} <= {};", real_expr(rhs)?)?;
            return Ok(());
        }
        let mut sens = Vec::new();
        let mut value = self.logic(rhs, Read::Net, &mut sens)?;
        if let Some((s1, s0)) = strength {
            value = format!(
                "drive({value}, {}, {})",
                strength_name(s1),
                strength_name(s0)
            );
        }
        let delays = self.gate_delays(delays)?;
        let comment = format!("assign {target}");
        self.driver(&comment, &target, &value, &sens, delays)
    }

    fn prim(
        &mut self,
        kind: &str,
        strength: Option<(Strength, Strength)>,
        delays: &[Expr],
        args: &[Expr],
    ) -> Result<()> {
        let target = self.lhs(&args[0])?;
        let mut sens = Vec::new();
        let value = match (kind, args.len()) {
            ("bufif0" | "bufif1" | "notif0" | "notif1", 3) => {
                let d = self.logic(&args[1], Read::Net, &mut sens)?;
                let c = self.logic(&args[2], Read::Net, &mut sens)?;
                format!("{kind}_v({d}, {c})")
            }
            ("nmos" | "pmos" | "rnmos" | "rpmos", 3) => {
                // A switch passes the strength of its input on.
                let d = match &args[1] {
                    Expr::Ident(n) => {
                        let s = self.read_name(n, Read::Net);
                        sens.push(s.clone());
                        s
                    }
                    e => self.logic(e, Read::Net, &mut sens)?,
                };
                let c = self.logic(&args[2], Read::Net, &mut sens)?;
                format!("{kind}_v({d}, {c})")
            }
            ("buf", 2) => self.logic(&args[1], Read::Net, &mut sens)?,
            ("not", 2) => format!("(not {})", self.logic(&args[1], Read::Net, &mut sens)?),
            _ => bail!("unsupported primitive {kind} with {} terminals", args.len()),
        };
        let value = match strength {
            Some((s1, s0)) => format!(
                "drive({value}, {}, {})",
                strength_name(s1),
                strength_name(s0)
            ),
            None => value,
        };
        let delays = self.gate_delays(delays)?;
        let comment = format!("{kind} {target}");
        self.driver(&comment, &target, &value, &sens, delays)
    }

    /// Statements of an `always` block. `assigned` collects the signals
    /// written so far, to catch a blocking assignment that a later statement
    /// reads (VHDL signals would read the old value).
    fn stmt(
        &self,
        s: &Stmt,
        indent: usize,
        out: &mut String,
        sens: &mut Vec<String>,
        assigned: &mut Vec<String>,
    ) -> Result<()> {
        let pad = " ".repeat(indent);
        match s {
            Stmt::Block(v) => {
                for s in v {
                    self.stmt(s, indent, out, sens, assigned)?;
                }
            }
            Stmt::If(c, t, e) => {
                let mut reads = Vec::new();
                let cond = self.logic(c, Read::Procedural, &mut reads)?;
                check_reads(&reads, assigned)?;
                merge(sens, reads);
                writeln!(out, "{pad}if {cond} = '1' then")?;
                self.stmt(t, indent + 2, out, sens, assigned)?;
                if let Some(e) = e {
                    writeln!(out, "{pad}else")?;
                    self.stmt(e, indent + 2, out, sens, assigned)?;
                }
                writeln!(out, "{pad}end if;")?;
            }
            Stmt::Assign { lhs, rhs, .. } => {
                let mut reads = Vec::new();
                let value = self.logic(rhs, Read::Procedural, &mut reads)?;
                check_reads(&reads, assigned)?;
                merge(sens, reads);
                let target = self.lhs(lhs)?;
                writeln!(out, "{pad}{target} <= {value};")?;
                assigned.push(target);
            }
        }
        Ok(())
    }

    fn always(&mut self, events: &[(Edge, String)], stmt: &Stmt, latch: bool) -> Result<()> {
        let mut inner = String::new();
        let mut sens = Vec::new();
        let mut assigned = Vec::new();
        let edge_triggered = events.iter().any(|(e, _)| *e != Edge::Any);
        let indent = if edge_triggered { 6 } else { 4 };
        self.stmt(stmt, indent, &mut inner, &mut sens, &mut assigned)?;
        writeln!(
            self.body,
            "  -- {}",
            if latch { "always_latch" } else { "always" }
        )?;
        if edge_triggered {
            let names: Vec<String> = events
                .iter()
                .map(|(_, n)| self.read_name(n, Read::Procedural))
                .collect();
            let conds: Vec<String> = events
                .iter()
                .zip(&names)
                .map(|((e, _), n)| match e {
                    Edge::Pos => format!("({n}'event and is_posedge({n}'last_value, {n}))"),
                    Edge::Neg => format!("({n}'event and is_negedge({n}'last_value, {n}))"),
                    Edge::Any => format!("{n}'event"),
                })
                .collect();
            writeln!(self.body, "  process ({}) is", names.join(", "))?;
            writeln!(self.body, "  begin")?;
            writeln!(self.body, "    if {} then", conds.join(" or "))?;
            self.body.push_str(&inner);
            writeln!(self.body, "    end if;")?;
        } else {
            if !events.is_empty() {
                sens = events
                    .iter()
                    .map(|(_, n)| self.read_name(n, Read::Procedural))
                    .collect();
            }
            if sens.is_empty() {
                bail!("always block without inputs");
            }
            writeln!(self.body, "  process ({}) is", sens.join(", "))?;
            writeln!(self.body, "  begin")?;
            self.body.push_str(&inner);
        }
        writeln!(self.body, "  end process;")?;
        Ok(())
    }

    fn inst(
        &mut self,
        module: &str,
        name: &str,
        params: &[(String, Expr)],
        ports: &[(String, Option<Expr>)],
    ) -> Result<()> {
        let dirs: Vec<(String, PortDir)> = if let Some(m) = self.modules.get(module) {
            m.ports.iter().map(|p| (p.name.clone(), p.dir)).collect()
        } else if let Some(b) = builtin_ports(module) {
            b.iter().map(|(n, d)| ((*n).to_owned(), *d)).collect()
        } else {
            bail!("instance {name} of unknown module {module}");
        };
        let mut generics = Vec::new();
        for (p, e) in params {
            generics.push(format!("{} => {}", vhdl_id(p), real_expr(e)?));
        }
        let mut actuals = Vec::new();
        for (k, (formal, actual)) in ports.iter().enumerate() {
            // Positional ports have an empty name.
            let (formal, dir) = if formal.is_empty() {
                dirs.get(k)
                    .cloned()
                    .with_context(|| format!("instance {name}: too many ports"))?
            } else {
                dirs.iter()
                    .find(|(n, _)| n == formal)
                    .cloned()
                    .with_context(|| format!("instance {name}: no port {formal} in {module}"))?
            };
            let a = match (actual, dir) {
                (None, PortDir::Input) => "'Z'".to_owned(),
                (None, _) => "open".to_owned(),
                (Some(Expr::Ident(n)), PortDir::Output | PortDir::Inout) => self.target(n),
                (Some(Expr::Ident(n)), PortDir::Input) => self.read_name(n, Read::Net),
                (Some(e), PortDir::Input) => self.logic(e, Read::Net, &mut Vec::new())?,
                (Some(e), _) => bail!("instance {name}: unsupported actual {e:?}"),
            };
            actuals.push(format!("{} => {a}", vhdl_id(&formal)));
        }
        writeln!(
            self.body,
            "  {} : entity work.{}",
            vhdl_id(&format!("u_{name}")),
            vhdl_id(module)
        )?;
        if !generics.is_empty() {
            writeln!(self.body, "    generic map ({})", generics.join(", "))?;
        }
        writeln!(self.body, "    port map ({});", actuals.join(", "))?;
        Ok(())
    }

    /// The process that applies the module paths of output `dst`. With
    /// `fused`, the output's only driver is a continuous assignment of this
    /// expression, which the process computes itself (one process and one
    /// signal less per cell, the bulk of the chip).
    fn path_process(
        &mut self,
        dst: &str,
        paths: &[(&[String], &[Expr])],
        fused: Option<&Expr>,
    ) -> Result<()> {
        let dst_v = vhdl_id(dst);
        let (value, mut sens) = if let Some(e) = fused {
            let mut sens = Vec::new();
            let v = self.logic(e, Read::Net, &mut sens)?;
            (v, sens)
        } else {
            let ud = self.target(dst);
            (ud.clone(), vec![ud])
        };
        let mut groups = Vec::new();
        for (k, (srcs, delays)) in paths.iter().enumerate() {
            let base = format!("p_{dst}_{k}");
            let base = vhdl_id(&base);
            let exprs: Vec<String> = delays.iter().map(real_expr).collect::<Result<_>>()?;
            // (rise, fall, turn-off); with two delays 0->z is the rise and
            // 1->z the fall delay.
            let (r, f) = (exprs[0].clone(), exprs.get(1).unwrap_or(&exprs[0]).clone());
            writeln!(
                self.decls,
                "  constant {base}_r : delay_length := to_t({r});"
            )?;
            writeln!(
                self.decls,
                "  constant {base}_f : delay_length := to_t({f});"
            )?;
            let (z0, z1) = match exprs.get(2) {
                Some(z) => {
                    writeln!(
                        self.decls,
                        "  constant {base}_z : delay_length := to_t({z});"
                    )?;
                    (format!("{base}_z"), format!("{base}_z"))
                }
                None => (format!("{base}_r"), format!("{base}_f")),
            };
            if exprs.len() > 3 {
                bail!("path to {dst}: more than three delays");
            }
            let srcs: Vec<String> = srcs.iter().map(|s| self.read_name(s, Read::Net)).collect();
            groups.push((srcs, format!("{base}_r, {base}_f, {z0}, {z1}")));
        }
        writeln!(self.body, "  -- module paths of {dst}")?;
        sens.dedup();
        writeln!(self.body, "  process ({}) is", sens.join(", "))?;
        writeln!(self.body, "    variable vl_new : std_ulogic;")?;
        writeln!(self.body, "    variable vl_old : std_ulogic := 'U';")?;
        writeln!(self.body, "    variable tm_age, tm_src : time;")?;
        writeln!(self.body, "    variable dl_sel, dl_try : delay_length;")?;
        writeln!(self.body, "    variable tm_pend : time := 0 fs;")?;
        writeln!(self.body, "  begin")?;
        writeln!(self.body, "    vl_new := {value};")?;
        writeln!(self.body, "    if vl_new /= vl_old and now < tm_pend then")?;
        writeln!(self.body, "      {dst_v} <= vl_new after tm_pend - now;")?;
        writeln!(self.body, "      vl_old := vl_new;")?;
        writeln!(self.body, "    elsif vl_new /= vl_old then")?;
        writeln!(self.body, "      tm_age := time'high;")?;
        writeln!(self.body, "      dl_sel := delay_length'high;")?;
        for (srcs, delays) in &groups {
            let mut a = format!("{}'last_event", srcs[0]);
            for s in &srcs[1..] {
                a = format!("minimum({a}, {s}'last_event)");
            }
            writeln!(self.body, "      tm_src := {a};")?;
            writeln!(self.body, "      dl_try := pdly(vl_old, vl_new, {delays});")?;
            writeln!(
                self.body,
                "      if tm_src < tm_age or (tm_src = tm_age and dl_try < dl_sel) then"
            )?;
            writeln!(self.body, "        tm_age := tm_src;")?;
            writeln!(self.body, "        dl_sel := dl_try;")?;
            writeln!(self.body, "      end if;")?;
        }
        writeln!(self.body, "      dl_sel := aged(dl_sel, tm_age);")?;
        writeln!(self.body, "      {dst_v} <= vl_new after dl_sel;")?;
        writeln!(self.body, "      tm_pend := now + dl_sel;")?;
        writeln!(self.body, "      vl_old := vl_new;")?;
        writeln!(self.body, "    end if;")?;
        writeln!(self.body, "  end process;")?;
        Ok(())
    }
}

/// The sources and delays of one module path declaration.
type PathGroup = (Vec<String>, Vec<Expr>);

fn merge(sens: &mut Vec<String>, reads: Vec<String>) {
    for r in reads {
        if !sens.contains(&r) {
            sens.push(r);
        }
    }
}

fn check_reads(reads: &[String], assigned: &[String]) -> Result<()> {
    if let Some(r) = reads.iter().find(|r| assigned.contains(r)) {
        bail!("{r} is read after an assignment in the same block");
    }
    Ok(())
}

/// Outputs with module paths whose only driver is a plain continuous
/// assignment, and that no `always` block reads: their assignment can move
/// into the process of the paths.
fn fusable(
    items: &[Item],
    paths: &HashSet<String>,
    init: &HashMap<String, char>,
) -> HashMap<String, Expr> {
    fn stmt_names(s: &Stmt, out: &mut Vec<String>) {
        match s {
            Stmt::Block(v) => v.iter().for_each(|s| stmt_names(s, out)),
            Stmt::If(c, t, e) => {
                expr_names(c, out);
                stmt_names(t, out);
                if let Some(e) = e {
                    stmt_names(e, out);
                }
            }
            Stmt::Assign { lhs, rhs } => {
                expr_names(lhs, out);
                expr_names(rhs, out);
            }
        }
    }
    fn expr_names(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Ident(n) => out.push(n.clone()),
            Expr::Scoped(..) | Expr::Num(_) => {}
            Expr::Unary(_, a) => expr_names(a, out),
            Expr::Binary(_, a, b) | Expr::Index(a, b) => {
                expr_names(a, out);
                expr_names(b, out);
            }
            Expr::Cond(a, b, c) => {
                expr_names(a, out);
                expr_names(b, out);
                expr_names(c, out);
            }
            Expr::Call(_, args) => args.iter().flatten().for_each(|a| expr_names(a, out)),
            Expr::Concat(v) => v.iter().for_each(|a| expr_names(a, out)),
        }
    }
    let mut out = HashMap::new();
    for o in paths {
        if init.contains_key(o) {
            continue;
        }
        let mut assigns = Vec::new();
        let mut other = false;
        for it in items {
            match it {
                Item::Assign {
                    lhs: Expr::Ident(n),
                    rhs,
                    strength,
                    delays,
                } if n == o => {
                    if strength.is_some() || !delays.is_empty() {
                        other = true;
                    }
                    assigns.push(rhs.clone());
                }
                Item::Prim { args, .. } if args.first() == Some(&Expr::Ident(o.clone())) => {
                    other = true;
                }
                Item::Inst { ports, .. }
                    if ports
                        .iter()
                        .any(|(_, a)| a.as_ref() == Some(&Expr::Ident(o.clone()))) =>
                {
                    other = true;
                }
                Item::AlwaysLatch(st) | Item::Always { stmt: st, .. } => {
                    let mut names = Vec::new();
                    stmt_names(st, &mut names);
                    if let Item::Always { events, .. } = it {
                        names.extend(events.iter().map(|(_, n)| n.clone()));
                    }
                    if names.contains(o) {
                        other = true;
                    }
                }
                _ => {}
            }
        }
        if !other && assigns.len() == 1 {
            out.insert(o.clone(), assigns.remove(0));
        }
    }
    out
}

/// Evaluates the condition of a generate `if`: dmg-sim's `nodelay` is off.
fn gen_cond(e: &Expr) -> Result<bool> {
    match e {
        Expr::Ident(n) if n == "nodelay" => Ok(false),
        Expr::Unary("!", a) => Ok(!gen_cond(a)?),
        _ => bail!("unsupported generate condition {e:?}"),
    }
}

fn flatten(items: &[Item], out: &mut Vec<Item>) -> Result<()> {
    for it in items {
        if let Item::GenIf {
            cond,
            then,
            otherwise,
        } = it
        {
            flatten(if gen_cond(cond)? { then } else { otherwise }, out)?;
        } else {
            out.push(it.clone());
        }
    }
    Ok(())
}

/// The context clause of a design unit that imports `imports`.
fn context(imports: &[String]) -> String {
    let mut s =
        String::from("library ieee;\nuse ieee.std_logic_1164.all;\nuse work.verilog.all;\n");
    for i in imports {
        let _ = writeln!(s, "use work.{}.all;", vhdl_id(i));
    }
    s
}

/// VHDL package for a `<cell>_param` package.
pub fn package(p: &Package) -> Result<String> {
    let mut s = context(&p.imports);
    writeln!(s, "\npackage {} is", vhdl_id(&p.name))?;
    for (n, e) in &p.params {
        writeln!(s, "  constant {} : real := {};", vhdl_id(n), real_expr(e)?)?;
    }
    writeln!(s, "end package;")?;
    Ok(s)
}

/// VHDL entity declaration of a cell.
pub fn entity(m: &Module) -> Result<String> {
    let mut s = context(&m.imports);
    writeln!(s, "\nentity {} is", vhdl_id(&m.name))?;
    if !m.params.is_empty() {
        let gens: Vec<String> = m
            .params
            .iter()
            .map(|p| {
                let ty = if p.is_int { "integer" } else { "real" };
                Ok(format!(
                    "{} : {ty} := {}",
                    vhdl_id(&p.name),
                    real_expr(&p.default)?
                ))
            })
            .collect::<Result<_>>()?;
        writeln!(s, "  generic (\n    {}\n  );", gens.join(";\n    "))?;
    }
    let mut items = Vec::new();
    flatten(&m.items, &mut items)?;
    let mut init = HashMap::new();
    initial_values(&items, &mut init)?;
    let ports: Vec<String> = m
        .ports
        .iter()
        .map(|p| {
            if p.vector {
                bail!("vector port {}", p.name);
            }
            let dir = match p.dir {
                PortDir::Input => "in",
                PortDir::Output => "out",
                PortDir::Inout => "inout",
            };
            let ty = if p.kind == NetKind::Real {
                "real"
            } else {
                "std_logic"
            };
            let has_path = items
                .iter()
                .any(|i| matches!(i, Item::Path(pd) if pd.dsts.contains(&p.name)));
            // An undriven inout or tri port floats; an output variable
            // starts unknown, or at its initial value.
            let def = match init.get(&p.name) {
                Some(c) if p.dir == PortDir::Output && !has_path => format!(" := '{c}'"),
                _ if p.kind == NetKind::Logic && (p.dir == PortDir::Inout || p.tri) => {
                    " := 'Z'".to_owned()
                }
                _ => String::new(),
            };
            Ok(format!("{} : {dir} {ty}{def}", vhdl_id(&p.name)))
        })
        .collect::<Result<_>>()?;
    if !ports.is_empty() {
        writeln!(s, "  port (\n    {}\n  );", ports.join(";\n    "))?;
    }
    writeln!(s, "end entity;")?;
    Ok(s)
}

fn initial_values(items: &[Item], init: &mut HashMap<String, char>) -> Result<()> {
    fn walk(s: &Stmt, init: &mut HashMap<String, char>) -> Result<()> {
        match s {
            Stmt::Block(v) => v.iter().try_for_each(|s| walk(s, init)),
            Stmt::Assign {
                lhs: Expr::Ident(n),
                rhs: Expr::Num(v),
                ..
            } => {
                let c = bit_literal(v).with_context(|| format!("initial value {v}"))?;
                init.insert(n.clone(), c);
                Ok(())
            }
            _ => bail!("unsupported initial statement {s:?}"),
        }
    }
    for it in items {
        if let Item::Initial(s) = it {
            walk(s, init)?;
        }
    }
    Ok(())
}

/// VHDL architecture of a cell.
#[expect(
    clippy::too_many_lines,
    reason = "collects the items, then emits them in order"
)]
pub fn architecture(m: &Module, modules: &HashMap<String, Module>) -> Result<String> {
    let mut items = Vec::new();
    flatten(&m.items, &mut items)?;
    let mut cell = Cell {
        m,
        modules,
        paths: HashSet::new(),
        signals: HashMap::new(),
        consts: HashSet::new(),
        init: HashMap::new(),
        decls: String::new(),
        body: String::new(),
        next_id: 0,
    };
    let mut init = HashMap::new();
    initial_values(&items, &mut init)?;
    for it in &items {
        match it {
            Item::Path(p) => cell.paths.extend(p.dsts.iter().cloned()),
            Item::LocalParam { name, .. } | Item::SpecParam { name, .. } => {
                cell.consts.insert(name.clone());
            }
            Item::Decl {
                kind,
                vector,
                names,
            } => {
                if *vector {
                    bail!("vector signal {}", names.join(", "));
                }
                for n in names {
                    cell.signals.insert(n.clone(), *kind);
                }
            }
            _ => {}
        }
    }
    for p in &m.ports {
        if cell.paths.contains(&p.name) && p.dir != PortDir::Output {
            bail!(
                "module path to {} port {}",
                if p.dir == PortDir::Inout {
                    "inout"
                } else {
                    "input"
                },
                p.name
            );
        }
    }
    for (n, c) in &init {
        let t = cell.target(n);
        cell.init.insert(t, *c);
    }
    let fused = fusable(&items, &cell.paths, &init);

    // Constants first: delays refer to them.
    for it in &items {
        match it {
            Item::LocalParam { name, value } | Item::SpecParam { name, value } => {
                writeln!(
                    cell.decls,
                    "  constant {} : real := {};",
                    vhdl_id(name),
                    real_expr(value)?
                )?;
            }
            _ => {}
        }
    }
    let mut sigs: Vec<(&String, &NetKind)> = cell.signals.iter().collect();
    sigs.sort_by_key(|(n, _)| *n);
    for (n, k) in sigs {
        let ty = if *k == NetKind::Real {
            "real"
        } else {
            "std_logic"
        };
        let def = cell
            .init
            .get(&vhdl_id(n))
            .map(|c| format!(" := '{c}'"))
            .unwrap_or_default();
        writeln!(cell.decls, "  signal {} : {ty}{def};", vhdl_id(n))?;
    }
    let mut ud: Vec<&String> = cell
        .paths
        .iter()
        .filter(|n| !fused.contains_key(*n))
        .collect();
    ud.sort();
    for n in ud {
        let t = cell.target(n);
        let def = cell
            .init
            .get(&t)
            .map(|c| format!(" := '{c}'"))
            .unwrap_or_default();
        writeln!(cell.decls, "  signal {t} : std_logic{def};")?;
    }

    let mut paths: Vec<(String, Vec<PathGroup>)> = Vec::new();
    for it in &items {
        match it {
            Item::Assign {
                lhs: Expr::Ident(n),
                ..
            } if fused.contains_key(n) => {}
            Item::Assign {
                strength,
                delays,
                lhs,
                rhs,
            } => cell.assign(*strength, delays, lhs, rhs)?,
            Item::Prim {
                kind,
                strength,
                delays,
                args,
            } => cell.prim(kind, *strength, delays, args)?,
            Item::AlwaysLatch(s) => cell.always(&[], s, true)?,
            Item::Always { events, stmt } => cell.always(events, stmt, false)?,
            Item::Inst {
                module,
                name,
                params,
                ports,
            } => cell.inst(module, name, params, ports)?,
            Item::Path(p) => {
                for d in &p.dsts {
                    let entry = if let Some(e) = paths.iter_mut().find(|(n, _)| n == d) {
                        e
                    } else {
                        paths.push((d.clone(), Vec::new()));
                        paths.last_mut().context("paths")?
                    };
                    entry.1.push((p.srcs.clone(), p.delays.clone()));
                }
            }
            Item::Decl { .. }
            | Item::LocalParam { .. }
            | Item::SpecParam { .. }
            | Item::Initial(_) => {}
            Item::GenIf { .. } => bail!("nested generate"),
        }
    }
    for (dst, groups) in &paths {
        let refs: Vec<(&[String], &[Expr])> = groups
            .iter()
            .map(|(s, d)| (s.as_slice(), d.as_slice()))
            .collect();
        cell.path_process(dst, &refs, fused.get(dst))?;
    }

    let mut s = context(&m.imports);
    writeln!(s, "\narchitecture sim of {} is", vhdl_id(&m.name))?;
    s.push_str(&cell.decls);
    writeln!(s, "begin")?;
    s.push_str(&cell.body);
    writeln!(s, "end architecture;")?;
    Ok(s)
}
