//! Tokens of the SystemVerilog subset that dmg-sim's cells are written in.

use std::collections::HashSet;

use anyhow::{Result, bail};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    /// A number as written: `54`, `0.14e-15`, `1'b0`, `'1`, `5ps`.
    Num(String),
    Str(String),
    /// Punctuation and operators.
    Op(&'static str),
}

/// Operators, longest first.
const OPS: &[&str] = &[
    "===", "!==", "<<<", ">>>", "*>", "=>", "<=", ">=", "==", "!=", "&&", "||", "~^", "^~", "<<",
    ">>", "::", "(", ")", "[", "]", "{", "}", ",", ";", ":", ".", "#", "@", "=", "!", "~", "&",
    "|", "^", "?", "+", "-", "*", "/", "%", "<", ">",
];

const TIME_UNITS: &[&str] = &["s", "ms", "us", "ns", "ps", "fs"];

/// Splits a source file into tokens. `` `ifdef `` blocks are kept or dropped
/// according to `defines`; the other directives are ignored.
#[expect(clippy::too_many_lines, reason = "a single scanner loop")]
pub fn lex(src: &str, defines: &HashSet<&str>) -> Result<Vec<(Tok, usize)>> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    // Stack of `ifdef states: whether the current branch is kept.
    let mut keep: Vec<bool> = Vec::new();
    let active = |k: &Vec<bool>| k.iter().all(|&x| x);
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            i += 2;
            while i < b.len() && !b[i..].starts_with(b"*/") {
                if b[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == b'`' {
            let start = i + 1;
            i = start;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let dir = &src[start..i];
            let arg_start = i;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            let arg = src[arg_start..i].trim();
            match dir {
                "ifdef" => keep.push(defines.contains(arg)),
                "ifndef" => keep.push(!defines.contains(arg)),
                "else" => {
                    let Some(k) = keep.last_mut() else {
                        bail!("line {line}: `else without `ifdef")
                    };
                    *k = !*k;
                }
                "endif" => {
                    if keep.pop().is_none() {
                        bail!("line {line}: `endif without `ifdef");
                    }
                }
                "default_nettype" | "timescale" => {}
                _ => bail!("line {line}: unsupported directive `{dir}"),
            }
            continue;
        }
        let tok_start = i;
        let tok = if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            Tok::Ident(src[tok_start..i].to_owned())
        } else if c == b'\\' {
            // Escaped identifier, up to white space.
            while i < b.len() && !b[i].is_ascii_whitespace() {
                i += 1;
            }
            Tok::Ident(src[tok_start + 1..i].to_owned())
        } else if c.is_ascii_digit()
            || (c == b'\'' && i + 1 < b.len() && b[i + 1].is_ascii_alphanumeric())
        {
            if c.is_ascii_digit() {
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
                    i += 1;
                }
                if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if i < b.len()
                    && (b[i] == b'e' || b[i] == b'E')
                    && i + 1 < b.len()
                    && (b[i + 1].is_ascii_digit() || b[i + 1] == b'-' || b[i + 1] == b'+')
                {
                    i += 2;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            if i < b.len() && b[i] == b'\'' {
                // Based number: 1'b0, 'z, '1, 8'hFF
                i += 1;
                if i < b.len() && matches!(b[i], b's' | b'S') {
                    i += 1;
                }
                if i < b.len()
                    && matches!(b[i], b'b' | b'B' | b'h' | b'H' | b'd' | b'D' | b'o' | b'O')
                {
                    i += 1;
                }
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
            } else {
                // Time literal: 5ps, 1s
                let mut j = i;
                while j < b.len() && b[j].is_ascii_alphabetic() {
                    j += 1;
                }
                if TIME_UNITS.contains(&&src[i..j]) {
                    i = j;
                }
            }
            Tok::Num(src[tok_start..i].to_owned())
        } else if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            Tok::Str(src[tok_start + 1..i - 1].to_owned())
        } else if let Some(op) = OPS.iter().find(|op| b[i..].starts_with(op.as_bytes())) {
            i += op.len();
            Tok::Op(op)
        } else {
            bail!("line {line}: unexpected character {:?}", c as char);
        };
        if active(&keep) {
            out.push((tok, line));
        }
    }
    if !keep.is_empty() {
        bail!("unterminated `ifdef");
    }
    Ok(out)
}
