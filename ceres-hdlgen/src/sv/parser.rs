//! Parser for the SystemVerilog subset of dmg-sim's cells.

use anyhow::{Result, anyhow, bail};

use super::ast::{
    Edge, Expr, Item, Module, NetKind, Package, Param, PathDecl, PortDecl, PortDir, Stmt, Strength,
};
use super::lexer::Tok;

pub struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
}

const PRIMITIVES: &[&str] = &[
    "bufif0", "bufif1", "notif0", "notif1", "nmos", "pmos", "rnmos", "rpmos", "buf", "not", "and",
    "or", "nand", "nor", "xor",
];

/// Binary operators by precedence, lowest first.
const BINARY: &[&[&str]] = &[
    &["||"],
    &["&&"],
    &["|"],
    &["^", "~^", "^~"],
    &["&"],
    &["==", "!=", "===", "!=="],
    &["<", "<=", ">", ">="],
    &["<<", ">>", "<<<", ">>>"],
    &["+", "-"],
    &["*", "/", "%"],
];

impl Parser {
    pub const fn new(toks: Vec<(Tok, usize)>) -> Self {
        Self { toks, i: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|t| &t.0)
    }

    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.i + n).map(|t| &t.0)
    }

    fn line(&self) -> usize {
        self.toks
            .get(self.i)
            .or_else(|| self.toks.last())
            .map_or(0, |t| t.1)
    }

    fn err<T>(&self, msg: &str) -> Result<T> {
        bail!("line {}: {msg}, found {:?}", self.line(), self.peek())
    }

    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if *o == op)
    }

    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s == kw)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.is_kw(kw) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn op(&mut self, op: &str) -> Result<()> {
        if self.eat_op(op) {
            Ok(())
        } else {
            self.err(&format!("expected {op}"))
        }
    }

    fn kw(&mut self, kw: &str) -> Result<()> {
        if self.eat_kw(kw) {
            Ok(())
        } else {
            self.err(&format!("expected {kw}"))
        }
    }

    fn ident(&mut self) -> Result<String> {
        if let Some(Tok::Ident(s)) = self.peek() {
            let s = s.clone();
            self.i += 1;
            Ok(s)
        } else {
            self.err("identifier expected")
        }
    }

    pub const fn at_end(&self) -> bool {
        self.i >= self.toks.len()
    }

    /// One `package` or `module` of a file.
    pub fn unit(&mut self) -> Result<Unit> {
        if self.eat_kw("package") {
            Ok(Unit::Package(self.package()?))
        } else if self.eat_kw("module") {
            Ok(Unit::Module(self.module()?))
        } else {
            self.err("package or module expected")
        }
    }

    fn package(&mut self) -> Result<Package> {
        let name = self.ident()?;
        self.op(";")?;
        let mut pkg = Package {
            name,
            imports: Vec::new(),
            params: Vec::new(),
        };
        while !self.eat_kw("endpackage") {
            if self.eat_kw("import") {
                pkg.imports.push(self.import()?);
            } else if self.eat_kw("parameter") || self.eat_kw("localparam") {
                self.data_type();
                loop {
                    let n = self.ident()?;
                    self.op("=")?;
                    pkg.params.push((n, self.expr()?));
                    if !self.eat_op(",") {
                        break;
                    }
                }
                self.op(";")?;
            } else {
                return self.err("unsupported package item");
            }
        }
        Ok(pkg)
    }

    fn import(&mut self) -> Result<String> {
        let pkg = self.ident()?;
        self.op("::")?;
        if !self.eat_op("*") {
            self.ident()?;
        }
        self.op(";")?;
        Ok(pkg)
    }

    /// Skips a data type (`real`, `int`, `realtime`, `bit`...) and tells
    /// whether it is an integer type.
    fn data_type(&mut self) -> bool {
        for (kw, is_int) in [
            ("real", false),
            ("realtime", false),
            ("time", true),
            ("int", true),
            ("bit", true),
        ] {
            if self.eat_kw(kw) {
                return is_int;
            }
        }
        false
    }

    fn module(&mut self) -> Result<Module> {
        let name = self.ident()?;
        let mut m = Module {
            name,
            imports: Vec::new(),
            params: Vec::new(),
            ports: Vec::new(),
            items: Vec::new(),
        };
        if self.eat_op("#") {
            self.op("(")?;
            while !self.eat_op(")") {
                self.kw("parameter")?;
                let is_int = self.data_type();
                let name = self.ident()?;
                self.op("=")?;
                m.params.push(Param {
                    name,
                    is_int,
                    default: self.expr()?,
                });
                self.eat_op(",");
            }
        }
        self.op("(")?;
        let mut dir = PortDir::Input;
        let mut kind = NetKind::Logic;
        let mut vector = false;
        let mut tri = false;
        while !self.eat_op(")") {
            let new_dir = if self.eat_kw("input") {
                Some(PortDir::Input)
            } else if self.eat_kw("output") {
                Some(PortDir::Output)
            } else if self.eat_kw("inout") {
                Some(PortDir::Inout)
            } else {
                None
            };
            if let Some(d) = new_dir {
                dir = d;
                tri = self.eat_kw("tri");
                kind = if self.eat_kw("real") {
                    NetKind::Real
                } else {
                    self.eat_kw("logic");
                    self.eat_kw("wire");
                    NetKind::Logic
                };
                vector = self.range()?.is_some();
            }
            let name = self.ident()?;
            m.ports.push(PortDecl {
                name,
                dir,
                kind,
                vector,
                tri,
            });
            self.eat_op(",");
        }
        self.op(";")?;
        while !self.eat_kw("endmodule") {
            if self.eat_kw("import") {
                m.imports.push(self.import()?);
                continue;
            }
            self.item(&mut m.items)?;
        }
        Ok(m)
    }

    /// An optional `[msb:lsb]`.
    fn range(&mut self) -> Result<Option<(Expr, Expr)>> {
        if !self.eat_op("[") {
            return Ok(None);
        }
        let a = self.expr()?;
        self.op(":")?;
        let b = self.expr()?;
        self.op("]")?;
        Ok(Some((a, b)))
    }

    fn strength(&mut self) -> Result<Option<(Strength, Strength)>> {
        let is_strength = matches!(self.peek_at(1), Some(Tok::Ident(s))
            if ["supply", "strong", "pull", "weak", "highz"].iter().any(|p| s.starts_with(p)));
        if !self.is_op("(") || !is_strength {
            return Ok(None);
        }
        self.op("(")?;
        let mut s1 = Strength::Strong;
        let mut s0 = Strength::Strong;
        for _ in 0..2 {
            let s = self.ident()?;
            let (base, val) = s.split_at(s.len() - 1);
            let st = match base {
                "supply" => Strength::Supply,
                "strong" => Strength::Strong,
                "pull" => Strength::Pull,
                "weak" => Strength::Weak,
                "highz" => Strength::HighZ,
                _ => return self.err("drive strength expected"),
            };
            if val == "1" {
                s1 = st;
            } else {
                s0 = st;
            }
            self.eat_op(",");
        }
        self.op(")")?;
        Ok(Some((s1, s0)))
    }

    fn delays(&mut self) -> Result<Vec<Expr>> {
        if !self.eat_op("#") {
            return Ok(Vec::new());
        }
        if self.eat_op("(") {
            let mut v = vec![self.expr()?];
            while self.eat_op(",") {
                v.push(self.expr()?);
            }
            self.op(")")?;
            Ok(v)
        } else {
            Ok(vec![self.primary()?])
        }
    }

    fn label(&mut self) -> Result<()> {
        if self.eat_op(":") {
            self.ident()?;
        }
        Ok(())
    }

    #[expect(clippy::too_many_lines, reason = "one arm per kind of module item")]
    fn item(&mut self, items: &mut Vec<Item>) -> Result<()> {
        let Some(Tok::Ident(kw)) = self.peek() else {
            return self.err("module item expected");
        };
        let kw = kw.clone();
        match kw.as_str() {
            "logic" | "tri" | "wire" | "real" => {
                self.i += 1;
                self.eat_kw("logic");
                let kind = if kw == "real" {
                    NetKind::Real
                } else {
                    NetKind::Logic
                };
                let vector = self.range()?.is_some();
                let mut names = vec![self.ident()?];
                while self.eat_op(",") {
                    names.push(self.ident()?);
                }
                self.op(";")?;
                items.push(Item::Decl {
                    kind,
                    vector,
                    names,
                });
            }
            "localparam" => {
                self.i += 1;
                self.data_type();
                loop {
                    let name = self.ident()?;
                    self.op("=")?;
                    items.push(Item::LocalParam {
                        name,
                        value: self.expr()?,
                    });
                    if !self.eat_op(",") {
                        break;
                    }
                }
                self.op(";")?;
            }
            "assign" => {
                self.i += 1;
                let strength = self.strength()?;
                let delays = self.delays()?;
                loop {
                    let lhs = self.postfix()?;
                    self.op("=")?;
                    items.push(Item::Assign {
                        strength,
                        delays: delays.clone(),
                        lhs,
                        rhs: self.expr()?,
                    });
                    if !self.eat_op(",") {
                        break;
                    }
                }
                self.op(";")?;
            }
            "initial" => {
                self.i += 1;
                items.push(Item::Initial(self.stmt()?));
            }
            "always_latch" | "always_comb" => {
                self.i += 1;
                items.push(Item::AlwaysLatch(self.stmt()?));
            }
            "always_ff" | "always" => {
                self.i += 1;
                self.op("@")?;
                let mut events = Vec::new();
                if !self.eat_op("*") {
                    self.op("(")?;
                    if !self.eat_op("*") {
                        loop {
                            let edge = if self.eat_kw("posedge") {
                                Edge::Pos
                            } else if self.eat_kw("negedge") {
                                Edge::Neg
                            } else {
                                Edge::Any
                            };
                            events.push((edge, self.ident()?));
                            if !(self.eat_op(",") || self.eat_kw("or")) {
                                break;
                            }
                        }
                    }
                    self.op(")")?;
                }
                items.push(Item::Always {
                    events,
                    stmt: self.stmt()?,
                });
            }
            "generate" => {
                self.i += 1;
                while !self.eat_kw("endgenerate") {
                    self.item(items)?;
                }
            }
            "if" => {
                self.i += 1;
                self.op("(")?;
                let cond = self.expr()?;
                self.op(")")?;
                let then = self.gen_block()?;
                let otherwise = if self.eat_kw("else") {
                    self.gen_block()?
                } else {
                    Vec::new()
                };
                items.push(Item::GenIf {
                    cond,
                    then,
                    otherwise,
                });
            }
            "begin" => {
                items.extend(self.gen_block()?);
            }
            "specify" => {
                self.i += 1;
                while !self.eat_kw("endspecify") {
                    if self.eat_kw("specparam") {
                        loop {
                            let name = self.ident()?;
                            self.op("=")?;
                            items.push(Item::SpecParam {
                                name,
                                value: self.expr()?,
                            });
                            if !self.eat_op(",") {
                                break;
                            }
                        }
                        self.op(";")?;
                    } else {
                        items.push(Item::Path(self.path()?));
                    }
                }
            }
            k if PRIMITIVES.contains(&k) => {
                self.i += 1;
                let strength = self.strength()?;
                let delays = self.delays()?;
                loop {
                    if let Some(Tok::Ident(_)) = self.peek() {
                        self.i += 1; // instance name
                    }
                    self.op("(")?;
                    let mut args = vec![self.expr()?];
                    while self.eat_op(",") {
                        args.push(self.expr()?);
                    }
                    self.op(")")?;
                    items.push(Item::Prim {
                        kind: kw.clone(),
                        strength,
                        delays: delays.clone(),
                        args,
                    });
                    if !self.eat_op(",") {
                        break;
                    }
                }
                self.op(";")?;
            }
            "for" | "foreach" | "genvar" | "function" | "task" | "program" | "fork" => {
                return self.err("unsupported construct");
            }
            _ => {
                // Module instance.
                let module = self.ident()?;
                let mut params = Vec::new();
                if self.eat_op("#") {
                    self.op("(")?;
                    while !self.eat_op(")") {
                        self.op(".")?;
                        let p = self.ident()?;
                        self.op("(")?;
                        params.push((p, self.expr()?));
                        self.op(")")?;
                        self.eat_op(",");
                    }
                }
                let name = self.ident()?;
                self.op("(")?;
                let mut ports = Vec::new();
                while !self.eat_op(")") {
                    if !self.is_op(".") {
                        // Positional port.
                        ports.push((String::new(), Some(self.expr()?)));
                        self.eat_op(",");
                        continue;
                    }
                    self.op(".")?;
                    let p = self.ident()?;
                    self.op("(")?;
                    let actual = if self.is_op(")") {
                        None
                    } else {
                        Some(self.expr()?)
                    };
                    self.op(")")?;
                    ports.push((p, actual));
                    self.eat_op(",");
                }
                self.op(";")?;
                items.push(Item::Inst {
                    module,
                    name,
                    params,
                    ports,
                });
            }
        }
        Ok(())
    }

    /// `begin [: label] items end`, or a single item.
    fn gen_block(&mut self) -> Result<Vec<Item>> {
        let mut items = Vec::new();
        if self.eat_kw("begin") {
            self.label()?;
            while !self.eat_kw("end") {
                self.item(&mut items)?;
            }
        } else {
            self.item(&mut items)?;
        }
        Ok(items)
    }

    /// A module path: `(a, b *> y) = (rise, fall);`
    fn path(&mut self) -> Result<PathDecl> {
        self.op("(")?;
        let mut srcs = vec![self.ident()?];
        while self.eat_op(",") {
            srcs.push(self.ident()?);
        }
        if !self.eat_op("*>") {
            return self.err("only full module paths (*>) are supported");
        }
        let mut dsts = vec![self.ident()?];
        while self.eat_op(",") {
            dsts.push(self.ident()?);
        }
        self.op(")")?;
        self.op("=")?;
        let mut delays = Vec::new();
        if self.eat_op("(") {
            delays.push(self.expr()?);
            while self.eat_op(",") {
                delays.push(self.expr()?);
            }
            self.op(")")?;
        } else {
            delays.push(self.expr()?);
        }
        self.op(";")?;
        Ok(PathDecl { srcs, dsts, delays })
    }

    fn stmt(&mut self) -> Result<Stmt> {
        if self.eat_kw("begin") {
            self.label()?;
            let mut v = Vec::new();
            while !self.eat_kw("end") {
                v.push(self.stmt()?);
            }
            return Ok(Stmt::Block(v));
        }
        if self.eat_kw("if") {
            self.op("(")?;
            let c = self.expr()?;
            self.op(")")?;
            let t = self.stmt()?;
            let e = if self.eat_kw("else") {
                Some(Box::new(self.stmt()?))
            } else {
                None
            };
            return Ok(Stmt::If(c, Box::new(t), e));
        }
        if let Some(Tok::Ident(k)) = self.peek()
            && ["for", "foreach", "case", "while", "repeat", "forever"].contains(&k.as_str())
        {
            return self.err("unsupported statement");
        }
        let lhs = self.postfix()?;
        if !self.eat_op("=") && !self.eat_op("<=") {
            return self.err("assignment expected");
        }
        let rhs = self.expr()?;
        self.op(";")?;
        Ok(Stmt::Assign { lhs, rhs })
    }

    pub fn expr(&mut self) -> Result<Expr> {
        let c = self.binary(0)?;
        if self.eat_op("?") {
            let a = self.expr()?;
            self.op(":")?;
            let b = self.expr()?;
            return Ok(Expr::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }

    fn binary(&mut self, level: usize) -> Result<Expr> {
        if level == BINARY.len() {
            return self.unary();
        }
        let mut a = self.binary(level + 1)?;
        while let Some(Tok::Op(op)) = self.peek() {
            let Some(&op) = BINARY[level].iter().find(|o| *o == op) else {
                break;
            };
            self.i += 1;
            let b = self.binary(level + 1)?;
            a = Expr::Binary(op, Box::new(a), Box::new(b));
        }
        Ok(a)
    }

    fn unary(&mut self) -> Result<Expr> {
        for op in ["!", "~", "-", "+", "&", "|", "^"] {
            if self.eat_op(op) {
                return Ok(Expr::Unary(op, Box::new(self.unary()?)));
            }
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr> {
        let mut e = self.primary()?;
        while self.eat_op("[") {
            let idx = self.expr()?;
            self.op("]")?;
            e = Expr::Index(Box::new(e), Box::new(idx));
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.i += 1;
                Ok(Expr::Num(n))
            }
            Some(Tok::Ident(n)) => {
                self.i += 1;
                let n = if self.eat_op("::") {
                    let m = self.ident()?;
                    if !self.is_op("(") {
                        return Ok(Expr::Scoped(n, m));
                    }
                    format!("{n}::{m}")
                } else {
                    n
                };
                if self.eat_op("(") {
                    let mut args = Vec::new();
                    if !self.eat_op(")") {
                        loop {
                            if self.is_op(",") || self.is_op(")") {
                                args.push(None);
                            } else {
                                args.push(Some(self.expr()?));
                            }
                            if self.eat_op(")") {
                                break;
                            }
                            self.op(",")?;
                        }
                    }
                    return Ok(Expr::Call(n, args));
                }
                Ok(Expr::Ident(n))
            }
            Some(Tok::Op("(")) => {
                self.i += 1;
                let e = self.expr()?;
                self.op(")")?;
                Ok(e)
            }
            Some(Tok::Op("{")) => {
                self.i += 1;
                let mut v = vec![self.expr()?];
                while self.eat_op(",") {
                    v.push(self.expr()?);
                }
                self.op("}")?;
                Ok(Expr::Concat(v))
            }
            _ => Err(anyhow!(
                "line {}: expression expected, found {:?}",
                self.line(),
                self.peek()
            )),
        }
    }
}

pub enum Unit {
    Package(Package),
    Module(Module),
}
