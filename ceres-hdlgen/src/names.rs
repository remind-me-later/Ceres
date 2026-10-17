//! VHDL identifiers for the names of the netlist and of the cell library.

/// Reserved words of VHDL-2008, which cannot be identifiers.
const RESERVED: &[&str] = &[
    "abs",
    "access",
    "after",
    "alias",
    "all",
    "and",
    "architecture",
    "array",
    "assert",
    "assume",
    "assume_guarantee",
    "attribute",
    "begin",
    "block",
    "body",
    "buffer",
    "bus",
    "case",
    "component",
    "configuration",
    "constant",
    "context",
    "cover",
    "default",
    "disconnect",
    "downto",
    "else",
    "elsif",
    "end",
    "entity",
    "exit",
    "fairness",
    "file",
    "for",
    "force",
    "function",
    "generate",
    "generic",
    "group",
    "guarded",
    "if",
    "impure",
    "in",
    "inertial",
    "inout",
    "is",
    "label",
    "library",
    "linkage",
    "literal",
    "loop",
    "map",
    "mod",
    "nand",
    "new",
    "next",
    "nor",
    "not",
    "null",
    "of",
    "on",
    "open",
    "or",
    "others",
    "out",
    "package",
    "parameter",
    "port",
    "postponed",
    "procedure",
    "process",
    "property",
    "protected",
    "pure",
    "range",
    "record",
    "register",
    "reject",
    "release",
    "rem",
    "report",
    "restrict",
    "restrict_guarantee",
    "return",
    "rol",
    "ror",
    "select",
    "sequence",
    "severity",
    "shared",
    "signal",
    "sla",
    "sll",
    "sra",
    "srl",
    "strong",
    "subtype",
    "then",
    "to",
    "transport",
    "type",
    "unaffected",
    "units",
    "until",
    "use",
    "variable",
    "vmode",
    "vprop",
    "vunit",
    "wait",
    "when",
    "while",
    "with",
    "xnor",
    "xor",
];

fn is_basic(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.contains("__")
        && !s.ends_with('_')
}

/// The VHDL identifier for a SystemVerilog identifier. Reserved words get a
/// `_p` suffix (the cell port `in` is `in_p`); a name that is not a basic
/// VHDL identifier becomes an extended one (`\16384hz\`).
pub fn vhdl_id(sv: &str) -> String {
    let sv = sv.strip_prefix('\\').map_or(sv, str::trim_end);
    if RESERVED.contains(&sv.to_ascii_lowercase().as_str()) {
        format!("{sv}_p")
    } else if is_basic(sv) {
        sv.to_owned()
    } else {
        format!("\\{}\\", sv.replace('\\', "\\\\"))
    }
}

/// A VHDL identifier made of a prefix and a name (`n_` for the signal of a
/// wire, `u_` for the label of an instance), so that it cannot clash with a
/// reserved word or with an identifier of the other kinds.
pub fn prefixed(prefix: &str, sv: &str) -> String {
    vhdl_id(&format!(
        "{prefix}{}",
        sv.strip_prefix('\\').map_or(sv, str::trim_end)
    ))
}

/// A real literal for VHDL: `54` is `54.0`, and a negative number is put in
/// parentheses, since VHDL does not allow `a + -1.0`.
pub fn real_literal(s: &str) -> String {
    let mut s = s.to_owned();
    if !s.contains(['.', 'e', 'E']) || s.starts_with(['e', 'E']) {
        s.push_str(".0");
    } else if !s.contains('.') {
        // 1e-15 is not a real literal in VHDL
        let (m, e) = s.split_at(s.find(['e', 'E']).unwrap_or(s.len()));
        s = format!("{m}.0{e}");
    }
    if s.starts_with('-') {
        format!("({s})")
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers() {
        assert_eq!(vhdl_id("in"), "in_p");
        assert_eq!(vhdl_id("ena_n"), "ena_n");
        assert_eq!(vhdl_id("\\16384hz "), "\\16384hz\\");
        assert_eq!(prefixed("n_", "\\16384hz "), "n_16384hz");
        assert_eq!(real_literal("54"), "54.0");
        assert_eq!(real_literal("2292.875"), "2292.875");
        assert_eq!(real_literal("-3"), "(-3.0)");
        assert_eq!(real_literal("1e-15"), "1.0e-15");
    }
}
