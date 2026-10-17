//! Reader for the netlist format of msinger's dmg-schematics (`*.nl`).
//!
//! This follows nlconv (`external/tools/nlconv`), the tool that reads these
//! files upstream, closely enough that the generated VHDL gets the same wire
//! lengths and gate widths as the SystemVerilog nlconv writes for dmg-sim:
//! the coordinates are single-precision floats, and the connections of a
//! wire are sorted the same way.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result, anyhow, bail};

/// Direction of a port of a cell type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    In,
    Out,
    Tri,
    Inout,
    /// Open-drain output: only drives low.
    Out0,
    /// Open-source output: only drives high.
    Out1,
    NotConnected,
}

#[derive(Debug)]
pub struct Port {
    pub name: String,
    pub dir: Dir,
}

#[derive(Debug)]
pub struct CellType {
    pub ports: Vec<Port>,
}

impl CellType {
    pub fn port(&self, name: &str) -> Option<&Port> {
        self.ports.iter().find(|p| p.name == name)
    }
}

#[derive(Debug)]
pub struct Cell {
    pub name: String,
    pub type_name: String,
    pub is_virtual: bool,
    /// Attributes in the order they are written (`attrib "a=b" ...`).
    pub attribs: Vec<(String, String)>,
}

/// One end of a wire: a port of a cell.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Conn {
    pub cell: String,
    pub port: String,
}

#[derive(Debug)]
pub struct Wire {
    pub name: String,
    pub signal: String,
    pub sources: Vec<Conn>,
    pub drains: Vec<Conn>,
    /// Polylines of the wire on the die, as `y, x` pairs.
    pub coords: Vec<Vec<f32>>,
    pub attribs: HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct Netlist {
    pub strings: HashMap<String, String>,
    pub types: Vec<CellType>,
    pub cells: Vec<Cell>,
    pub wires: Vec<Wire>,
    type_index: HashMap<String, usize>,
    cell_index: HashMap<String, usize>,
    /// Which wire each cell port is connected to.
    cons: HashMap<Conn, usize>,
    wire_names: std::collections::HashSet<String>,
}

impl Netlist {
    /// Reads the files of a netlist in order, with the conditional tags
    /// (`cell:codegen ...`) in `conds` set.
    pub fn read(files: &[impl AsRef<Path>], conds: &[&str]) -> Result<Self> {
        let mut nl = Self::default();
        for file in files {
            let file = file.as_ref();
            let text =
                fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
            let mut pending: Vec<Token> = Vec::new();
            for (num, line) in text.lines().enumerate() {
                let at = || format!("{}:{}", file.display(), num + 1);
                pending.extend(lex(line).with_context(at)?);
                while let Some(end) = pending.iter().position(|t| *t == Token::Eot) {
                    let stmt: Vec<Token> = pending.drain(..=end).collect();
                    nl.statement(&stmt, conds).with_context(at)?;
                }
            }
            if !pending.is_empty() {
                bail!("{}: end of file expected", file.display());
            }
        }
        nl.finish()?;
        Ok(nl)
    }

    pub fn cell(&self, name: &str) -> Option<&Cell> {
        self.cell_index.get(name).map(|&i| &self.cells[i])
    }

    /// The type of a cell.
    pub fn type_of(&self, cell: &Cell) -> &CellType {
        &self.types[self.type_index[&cell.type_name]]
    }

    /// The index of the wire a cell port is connected to.
    pub fn wire_at(&self, cell: &str, port: &str) -> Option<usize> {
        let conn = Conn {
            cell: cell.to_owned(),
            port: port.to_owned(),
        };
        self.cons.get(&conn).copied()
    }

    /// The direction of the port a connection ends at.
    pub fn dir(&self, conn: &Conn) -> Dir {
        let cell = &self.cells[self.cell_index[&conn.cell]];
        self.type_of(cell)
            .port(&conn.port)
            .map_or(Dir::NotConnected, |p| p.dir)
    }

    #[expect(clippy::too_many_lines, reason = "one arm per statement of the format")]
    fn statement(&mut self, stmt: &[Token], conds: &[&str]) -> Result<()> {
        let mut p = Parser { t: stmt, i: 0 };
        let Token::Name(kw) = p.peek() else {
            if *p.peek() == Token::Eot {
                return Ok(());
            }
            bail!("invalid statement");
        };
        match kw.to_ascii_lowercase().as_str() {
            "type" => {
                p.next();
                if !p.conditional(conds)? {
                    return Ok(());
                }
                let name = canonicalize_bars(&p.name("type name")?);
                p.color()?;
                let mut ports = Vec::new();
                while let Token::Name(n) = p.peek() {
                    let n = canonicalize_bars(n);
                    p.next();
                    let dir = if p.eat(&Token::Colon) {
                        parse_dir(&p.name("port direction")?)?
                    } else {
                        Dir::In
                    };
                    if ports.iter().any(|q: &Port| q.name == n) {
                        bail!("port name {n} already in use");
                    }
                    ports.push(Port { name: n, dir });
                }
                // Coordinates, description and documentation are not needed.
                p.coord_list(true)?;
                p.strings();
                if p.keyword("doc") {
                    p.strings();
                }
                if self.type_index.contains_key(&name) {
                    bail!("type {name} already defined");
                }
                self.type_index.insert(name, self.types.len());
                self.types.push(CellType { ports });
            }
            "cell" => {
                p.next();
                if !p.conditional(conds)? {
                    return Ok(());
                }
                let name = canonicalize_bars(&p.name("cell name")?);
                p.expect(&Token::Colon)?;
                let type_name = canonicalize_bars(&p.name("cell type")?);
                p.orientation()?;
                p.coord_list(true)?;
                let mut is_virtual = false;
                while let Token::Name(n) = p.peek() {
                    match n.to_ascii_lowercase().as_str() {
                        "virtual" => is_virtual = true,
                        "spare" | "comp" | "trivial" => {}
                        _ => break,
                    }
                    p.next();
                }
                if p.eat(&Token::To) {
                    p.name("category name")?;
                }
                p.strings();
                let mut attribs = Vec::new();
                if p.keyword("attrib") {
                    for a in p.strings() {
                        let (k, v) = a.split_once('=').unwrap_or((a.as_str(), "1"));
                        attribs.push((k.trim().to_owned(), v.trim().to_owned()));
                    }
                }
                if self.cell_index.contains_key(&name) {
                    bail!("cell {name} already defined");
                }
                self.cell_index.insert(name.clone(), self.cells.len());
                self.cells.push(Cell {
                    name,
                    type_name,
                    is_virtual,
                    attribs,
                });
            }
            "wire" => {
                p.next();
                if !p.conditional(conds)? {
                    return Ok(());
                }
                let name = canonicalize_bars(&p.name("wire name")?);
                let signal = if p.eat(&Token::Colon) {
                    canonicalize_bars(&p.name("wire signal class")?)
                } else {
                    String::new()
                };
                while let Token::Name(n) = p.peek() {
                    if p.peek_at(1) == &Token::Dot || !n.eq_ignore_ascii_case("unchecked") {
                        break;
                    }
                    p.next();
                }
                let sources = p.connections(conds)?;
                let drains = if p.eat(&Token::To) {
                    p.connections(conds)?
                } else {
                    Vec::new()
                };
                let coords = p.coord_list(false)?.into_iter().map(|(_, c)| c).collect();
                p.strings();
                let mut attribs = HashMap::new();
                if p.keyword("attrib") {
                    for a in p.strings() {
                        let (k, v) = a.split_once('=').unwrap_or((a.as_str(), "1"));
                        attribs.insert(k.trim().to_owned(), v.trim().to_owned());
                    }
                }
                if !self.wire_names.insert(name.clone()) {
                    bail!("wire {name} already defined");
                }
                self.wires.push(Wire {
                    name,
                    signal,
                    sources,
                    drains,
                    coords,
                    attribs,
                });
            }
            "define" => {
                p.next();
                let name = p.name("string name")?;
                let value = match p.peek() {
                    Token::Str(_) => p.strings().concat(),
                    Token::Value(v) => {
                        let v = *v;
                        p.next();
                        format_f32(v)
                    }
                    _ => String::new(),
                };
                self.strings.insert(name, value);
            }
            // Signals, categories, labels and aliases only matter for the
            // drawings and the HTML pages.
            "signal" | "category" | "label" | "alias" => return Ok(()),
            _ => bail!("invalid statement {kw}"),
        }
        p.expect(&Token::Eot)
    }

    /// The checks and the bookkeeping nlconv does after reading all files.
    fn finish(&mut self) -> Result<()> {
        for cell in &self.cells {
            if !self.type_index.contains_key(&cell.type_name) {
                bail!("cell {}: type {} not found", cell.name, cell.type_name);
            }
        }
        for wi in 0..self.wires.len() {
            // A bidirectional source also is a drain.
            let mut extra = Vec::new();
            for c in self.wires[wi].sources.iter().chain(&self.wires[wi].drains) {
                let cell = self.cell(&c.cell).ok_or_else(|| {
                    anyhow!("wire {}: cell {} not found", self.wires[wi].name, c.cell)
                })?;
                if self.type_of(cell).port(&c.port).is_none() {
                    bail!(
                        "wire {}: cell {} has no port {}",
                        self.wires[wi].name,
                        c.cell,
                        c.port
                    );
                }
            }
            for c in &self.wires[wi].sources {
                if self.dir(c) == Dir::Inout {
                    extra.push(c.clone());
                }
            }
            self.wires[wi].drains.extend(extra);
        }
        for (wi, w) in self.wires.iter().enumerate() {
            let mut both: Vec<&Conn> = w.sources.iter().collect();
            both.extend(w.drains.iter().filter(|c| !w.sources.contains(c)));
            for c in both {
                if self.cons.insert(c.clone(), wi).is_some() {
                    bail!("connection {}.{} made twice", c.cell, c.port);
                }
            }
        }
        for w in &mut self.wires {
            w.sources.sort_by(conn_order);
            w.drains.sort_by(conn_order);
        }
        Ok(())
    }
}

/// nlconv sorts the connections of a wire by cell, then port, without bars.
fn conn_order(a: &Conn, b: &Conn) -> std::cmp::Ordering {
    without_bars(&a.cell)
        .cmp(&without_bars(&b.cell))
        .then_with(|| without_bars(&a.port).cmp(&without_bars(&b.port)))
}

fn parse_dir(s: &str) -> Result<Dir> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "in" => Dir::In,
        "out" => Dir::Out,
        "tri" => Dir::Tri,
        "inout" => Dir::Inout,
        "out0" => Dir::Out0,
        "out1" => Dir::Out1,
        "nc" => Dir::NotConnected,
        _ => bail!("invalid port direction {s}"),
    })
}

/// Formats a float like nlconv does (Mono's `float.ToString()`, format
/// "G7"): 7 significant digits, without trailing zeros.
pub fn format_f32(v: f32) -> String {
    if v == 0.0 {
        return "0".to_owned();
    }
    // The exact decimal value of the float, rounded to 7 significant digits
    // half away from zero, as Mono does.
    let sci = format!("{:.40e}", f64::from(v).abs());
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let mut exp: i32 = exp.parse().unwrap_or(0);
    let all: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    let mut digits: Vec<u8> = all[..7].to_vec();
    if all[7] >= 5 {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                digits.pop();
                exp += 1;
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }
    let digits: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
    let mut out = String::new();
    if v < 0.0 {
        out.push('-');
    }
    if exp < 0 {
        out.push_str("0.");
        out.push_str(&"0".repeat(usize::try_from(-exp - 1).unwrap_or(0)));
        out.push_str(&digits);
    } else {
        let int_len = usize::try_from(exp + 1).unwrap_or(1);
        if digits.len() <= int_len {
            out.push_str(&digits);
            out.push_str(&"0".repeat(int_len - digits.len()));
        } else {
            out.push_str(&digits[..int_len]);
            out.push('.');
            out.push_str(&digits[int_len..]);
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Comma,
    Plus,
    Minus,
    To,
    Colon,
    Eot,
    Dot,
    At,
    Str(String),
    Name(String),
    Value(f32),
}

const fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '~' | '{' | '}' | '[' | ']')
}

fn lex(line: &str) -> Result<Vec<Token>> {
    let chars: Vec<char> = line.chars().collect();
    let mut t = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let simple = match c {
            '#' => break,
            ',' => Some(Token::Comma),
            '+' => Some(Token::Plus),
            '-' if chars.get(i + 1) == Some(&'>') => {
                i += 1;
                Some(Token::To)
            }
            '-' => Some(Token::Minus),
            ':' => Some(Token::Colon),
            ';' => Some(Token::Eot),
            '.' => Some(Token::Dot),
            '@' => Some(Token::At),
            _ => None,
        };
        if let Some(tok) = simple {
            t.push(tok);
            i += 1;
            continue;
        }
        if c == ' ' || c == '\t' || c == '\r' || c == '\u{a0}' {
            i += 1;
            continue;
        }
        if c == '"' {
            let mut s = String::new();
            let mut escaped = false;
            i += 1;
            loop {
                let Some(&c) = chars.get(i) else {
                    bail!("unterminated string")
                };
                i += 1;
                if escaped {
                    s.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        c => c,
                    });
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    break;
                } else {
                    s.push(c);
                }
            }
            t.push(Token::Str(s));
            continue;
        }
        if is_name_char(c) {
            let (mut is_name, mut is_val) = (false, false);
            let mut s = String::new();
            while let Some(&c) = chars.get(i) {
                if !(is_name_char(c) || c == '-' || c == '.') {
                    break;
                }
                if c == '.' {
                    if is_name {
                        break;
                    }
                    is_val = true;
                }
                if is_val
                    && (c.is_ascii_alphabetic()
                        || matches!(c, '_' | '~' | '{' | '}' | '[' | ']' | '-'))
                {
                    break;
                }
                s.push(c);
                if !is_val && !c.is_ascii_digit() {
                    is_name = true;
                }
                i += 1;
            }
            if is_name {
                t.push(Token::Name(s));
            } else {
                t.push(Token::Value(
                    s.parse().map_err(|_| anyhow!("invalid number {s}"))?,
                ));
            }
            continue;
        }
        bail!("unknown character {c:?}");
    }
    Ok(t)
}

struct Parser<'a> {
    t: &'a [Token],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> &Token {
        self.t.get(self.i).unwrap_or(&Token::Eot)
    }

    fn peek_at(&self, n: usize) -> &Token {
        self.t.get(self.i + n).unwrap_or(&Token::Eot)
    }

    const fn next(&mut self) {
        self.i += 1;
    }

    fn eat(&mut self, t: &Token) -> bool {
        if self.peek() == t {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: &Token) -> Result<()> {
        if self.eat(t) {
            Ok(())
        } else {
            bail!("expected {t:?}, found {:?}", self.peek())
        }
    }

    fn name(&mut self, what: &str) -> Result<String> {
        if let Token::Name(n) = self.peek() {
            let n = n.clone();
            self.i += 1;
            Ok(n)
        } else {
            bail!("{what} expected, found {:?}", self.peek())
        }
    }

    fn keyword(&mut self, kw: &str) -> bool {
        if let Token::Name(n) = self.peek()
            && n.eq_ignore_ascii_case(kw)
        {
            self.i += 1;
            return true;
        }
        false
    }

    fn strings(&mut self) -> Vec<String> {
        let mut v = Vec::new();
        while let Token::Str(s) = self.peek() {
            v.push(s.clone());
            self.i += 1;
        }
        v
    }

    /// `:tag`, `:+tag` or `:-tag` after `type`, `cell` and `wire`.
    fn conditional(&mut self, conds: &[&str]) -> Result<bool> {
        if !self.eat(&Token::Colon) {
            return Ok(true);
        }
        let negate = if self.eat(&Token::Minus) {
            true
        } else {
            self.eat(&Token::Plus);
            false
        };
        let tag = self.name("conditional tag")?;
        Ok(conds.contains(&tag.as_str()) != negate)
    }

    fn color(&mut self) -> Result<()> {
        if self.eat(&Token::Colon) {
            self.name("color")?;
        }
        Ok(())
    }

    fn orientation(&mut self) -> Result<()> {
        if let Token::Name(n) = self.peek()
            && matches!(
                n.to_ascii_lowercase().as_str(),
                "rot0" | "rot90" | "rot180" | "rot270"
            )
        {
            self.i += 1;
            if self.eat(&Token::Comma) && !self.keyword("flip") {
                bail!("invalid cell orientation");
            }
        }
        Ok(())
    }

    fn float(&mut self) -> Result<f32> {
        let sign = if self.eat(&Token::Minus) {
            -1.0f32
        } else {
            self.eat(&Token::Plus);
            1.0
        };
        if let Token::Value(v) = self.peek() {
            let v = *v;
            self.i += 1;
            Ok(sign * v)
        } else {
            bail!("float number expected")
        }
    }

    fn coord_list(&mut self, named: bool) -> Result<Vec<(String, Vec<f32>)>> {
        let mut l = Vec::new();
        loop {
            let mut name = String::new();
            if named
                && let Token::Name(n) = self.peek()
                && self.peek_at(1) == &Token::At
            {
                name = canonicalize_bars(n);
                self.i += 1;
            }
            if !self.eat(&Token::At) {
                break;
            }
            let mut v = vec![self.float()?];
            while self.eat(&Token::Comma) {
                v.push(self.float()?);
            }
            l.push((name, v));
        }
        Ok(l)
    }

    fn connections(&mut self, conds: &[&str]) -> Result<Vec<Conn>> {
        let mut l = Vec::new();
        while let (Token::Name(cell), Token::Dot, Token::Name(port)) =
            (self.peek(), self.peek_at(1), self.peek_at(2))
        {
            let conn = Conn {
                cell: canonicalize_bars(cell),
                port: canonicalize_bars(port),
            };
            self.i += 3;
            if self.conditional(conds)? {
                l.push(conn);
            }
        }
        Ok(l)
    }
}

/// Splits `name[3]` into `name` and `[3]`.
fn split_index(n: &str) -> (&str, &str) {
    if let Some(open) = n.rfind('[')
        && n.ends_with(']')
        && n[open + 1..n.len() - 1].trim().parse::<i32>().is_ok()
    {
        return n.split_at(open);
    }
    (n, "")
}

/// The index of a name like `d[3]`.
pub fn index_of(n: &str) -> Option<i32> {
    let (_, idx) = split_index(n);
    if idx.is_empty() {
        None
    } else {
        idx[1..idx.len() - 1].trim().parse().ok()
    }
}

/// nlconv's handling of overbars: `~reset` and `~{a b}` are names with a bar
/// over (part of) them, `~~` is a literal tilde.
fn process_bars(
    n: &str,
    bar_on: &str,
    bar_off: &str,
    ws: Option<&str>,
    tilde_map: Option<&str>,
) -> String {
    let (base, index_part) = split_index(n);
    let tilde = tilde_map.unwrap_or("~");
    let mut sb = String::new();
    let mut mode = 0;
    for c in base.chars() {
        if c == '~' && mode == 0 {
            mode = 1;
            continue;
        } else if mode == 1 {
            if c == '~' {
                mode = 0;
                sb.push_str(tilde);
                continue;
            }
            sb.push_str(bar_on);
            if c == '{' {
                mode = 2;
                continue;
            }
            mode = 3;
        } else if mode == 2 {
            if c == '~' {
                mode = 5;
                continue;
            } else if c == '}' {
                mode = 0;
                sb.push_str(bar_off);
                continue;
            }
        } else if c == '~' && mode == 3 {
            mode = 4;
            continue;
        } else if mode == 4 {
            if c == '~' {
                mode = 3;
                sb.push_str(tilde);
                continue;
            }
            sb.push_str(bar_off);
            mode = 0;
        } else if mode == 5 {
            sb.push_str(tilde);
            mode = 2;
            if c == '~' {
                continue;
            } else if c == '}' {
                mode = 0;
                sb.push_str(bar_off);
                continue;
            }
        }
        if c == '~'
            && let Some(m) = tilde_map
        {
            sb.push_str(m);
        } else if let Some(ws) = ws
            && c.is_whitespace()
        {
            sb.push_str(ws);
        } else if !c.is_control() || (ws.is_none() && c.is_whitespace()) {
            sb.push(c);
        }
    }
    if mode == 1 || mode == 5 {
        sb.push_str(tilde);
    } else if mode > 1 {
        sb.push_str(bar_off);
    }
    sb.push_str(index_part);
    sb
}

fn canonicalize_bars(n: &str) -> String {
    process_bars(n, "~{", "}", Some(" "), Some("~~"))
}

fn without_bars(n: &str) -> String {
    process_bars(n, "", "", Some(" "), None)
}

/// nlconv's SystemVerilog name for a netlist name, unvectorized: `~reset`
/// is `reset_n`, `d[3]` is `d3` and `ch1[2]` is `ch1_2`. With a prefix,
/// `L_` and `d[3]` make `L_d3`.
pub fn sv_name(s: &str, prefix: &str) -> String {
    let (base, idx) = split_index(s);
    let mut name = process_bars(prefix, "", "_n", Some("_"), None);
    name.push_str(&process_bars(base, "", "_n", Some("_"), None));
    if let Some(i) = index_of(s).filter(|_| !idx.is_empty()) {
        if name.ends_with(|c: char| c.is_ascii_digit()) {
            name.push('_');
        }
        let _ = write!(name, "{i}");
    }
    name
}

/// The file lists of a Makefile variable (`NAME = a.nl b.nl \ ...`).
pub fn makefile_list(makefile: &Path, var: &str) -> Result<Vec<String>> {
    let text =
        fs::read_to_string(makefile).with_context(|| format!("reading {}", makefile.display()))?;
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let body = if inside {
            line
        } else if let Some(rest) = line
            .strip_prefix(var)
            .and_then(|r| r.trim_start().strip_prefix('='))
        {
            inside = true;
            rest
        } else {
            continue;
        };
        let cont = body.trim_end().ends_with('\\');
        out.extend(
            body.trim_end()
                .trim_end_matches('\\')
                .split_whitespace()
                .map(str::to_owned),
        );
        if !cont {
            break;
        }
    }
    if out.is_empty() {
        bail!("{}: no list {var}", makefile.display());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars() {
        assert_eq!(sv_name("~reset", ""), "reset_n");
        assert_eq!(sv_name("d[3]", "L_"), "L_d3");
        assert_eq!(sv_name("ch1[2]", ""), "ch1_2");
        assert_eq!(canonicalize_bars("~reset"), "~{reset}");
    }

    #[test]
    fn formats_like_nlconv() {
        assert_eq!(format_f32(5376.7827), "5376.783");
        assert_eq!(format_f32(10013.0), "10013");
        assert_eq!(format_f32(2292.875), "2292.875");
        assert_eq!(format_f32(0.5), "0.5");
        assert_eq!(format_f32(26.5625), "26.5625");
        assert_eq!(format_f32(84.077_896), "84.0779");
        assert_eq!(format_f32(-12.25), "-12.25");
        assert_eq!(format_f32(474.40625), "474.4063");
    }

    #[test]
    fn lexes_coordinates_and_names() {
        let t = lex("cell rony:dlatch rot0 @-62.94,25.60 ->bus-data;").unwrap_or_default();
        assert_eq!(t[1], Token::Name("rony".into()));
        assert_eq!(t[6], Token::Minus);
        assert_eq!(t[7], Token::Value(62.94));
        assert_eq!(t[10], Token::To);
        assert_eq!(t[11], Token::Name("bus-data".into()));
    }
}
