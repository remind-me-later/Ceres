//! Syntax tree of the SystemVerilog subset of dmg-sim's cells.

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Ident(String),
    /// `pkg::name`
    Scoped(String, String),
    Num(String),
    Unary(&'static str, Box<Self>),
    Binary(&'static str, Box<Self>, Box<Self>),
    Cond(Box<Self>, Box<Self>, Box<Self>),
    /// A function call; an empty argument (`tpd_z(, t)`) is `None`.
    Call(String, Vec<Option<Self>>),
    Index(Box<Self>, Box<Self>),
    Concat(Vec<Self>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortDir {
    Input,
    Output,
    Inout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetKind {
    Logic,
    Real,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub is_int: bool,
    pub default: Expr,
}

#[derive(Clone, Debug)]
pub struct PortDecl {
    pub name: String,
    pub dir: PortDir,
    pub kind: NetKind,
    pub vector: bool,
    /// Declared `tri`: undriven, it floats (z) instead of being unknown.
    pub tri: bool,
}

/// Drive strength of a gate output for a value of 1 or 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    Supply,
    Strong,
    Pull,
    Weak,
    HighZ,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Block(Vec<Self>),
    If(Expr, Box<Self>, Option<Box<Self>>),
    /// `lhs = rhs` or `lhs <= rhs`: both become VHDL signal assignments.
    Assign {
        lhs: Expr,
        rhs: Expr,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Pos,
    Neg,
    Any,
}

#[derive(Clone, Debug)]
pub struct PathDecl {
    pub srcs: Vec<String>,
    pub dsts: Vec<String>,
    pub delays: Vec<Expr>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Decl {
        kind: NetKind,
        vector: bool,
        names: Vec<String>,
    },
    LocalParam {
        name: String,
        value: Expr,
    },
    Assign {
        strength: Option<(Strength, Strength)>,
        delays: Vec<Expr>,
        lhs: Expr,
        rhs: Expr,
    },
    Initial(Stmt),
    AlwaysLatch(Stmt),
    /// `always_ff @(...)`, `always @(...)` and `always @*` (no events).
    Always {
        events: Vec<(Edge, String)>,
        stmt: Stmt,
    },
    /// Gate or switch primitive: `bufif1 (highz1, strong0) #(r, f, z) (y, a, ena);`
    Prim {
        kind: String,
        strength: Option<(Strength, Strength)>,
        delays: Vec<Expr>,
        args: Vec<Expr>,
    },
    Inst {
        module: String,
        name: String,
        params: Vec<(String, Expr)>,
        ports: Vec<(String, Option<Expr>)>,
    },
    GenIf {
        cond: Expr,
        then: Vec<Self>,
        otherwise: Vec<Self>,
    },
    SpecParam {
        name: String,
        value: Expr,
    },
    Path(PathDecl),
}

#[derive(Clone, Debug)]
pub struct Module {
    pub name: String,
    pub imports: Vec<String>,
    pub params: Vec<Param>,
    pub ports: Vec<PortDecl>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub imports: Vec<String>,
    pub params: Vec<(String, Expr)>,
}
