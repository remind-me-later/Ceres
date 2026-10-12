//! Expressions, as in SameBoy's debugger: `$C000`, `hl + 2`, `[wCounter]`,
//! `$39:$4579`, `a == 5 && [hl] != 0`, `a = $10`.
//!
//! - Numbers are decimal, `$hex` (or `0xhex`) or `%binary`.
//! - Names are registers (`a f b c d e h l af bc de hl sp pc`), then
//!   symbols, and in watchpoint conditions `old` and `new` (the value the
//!   watched byte had and gets).
//! - `[x]` is the byte at x, `{x}` the little-endian word.
//! - `bank:addr` gives an address a bank: reading `[$3A:$4000]` reads ROM
//!   bank 3A whether it's mapped or not. A symbol has its bank.
//! - Operators bind as in C (SameBoy's own order is unusual), with `:`
//!   tightest: unary `- ! ~`, `* / %`, `+ -`, `<< >>`, `< <= > >=`,
//!   `== !=`, `&`, `^`, `|`, `&&`, `||`, then `=`, which assigns a register,
//!   `[x]` or `{x}`.
//!
//! Values are 16-bit and wrap.

use {
    super::{
        mapped_bank,
        symbols::{Bank, Symbols},
    },
    ceres_core::{AudioCallback, Gb},
};

/// A value, with the bank it's an address in, if it's known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value {
    pub bank: Option<Bank>,
    pub value: u16,
}

impl Value {
    const fn plain(value: u16) -> Self {
        Self { bank: None, value }
    }
}

/// The byte a watchpoint saw change, for `old` and `new` in its condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watched {
    pub old: u8,
    pub new: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    A,
    F,
    B,
    C,
    D,
    E,
    H,
    L,
    Af,
    Bc,
    De,
    Hl,
    Sp,
    Pc,
}

impl Register {
    fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "a" => Self::A,
            "f" => Self::F,
            "b" => Self::B,
            "c" => Self::C,
            "d" => Self::D,
            "e" => Self::E,
            "h" => Self::H,
            "l" => Self::L,
            "af" => Self::Af,
            "bc" => Self::Bc,
            "de" => Self::De,
            "hl" => Self::Hl,
            "sp" => Self::Sp,
            "pc" => Self::Pc,
            _ => return None,
        })
    }

    fn read<A: AudioCallback>(self, gb: &Gb<A>) -> u16 {
        let high = |pair: u16| pair >> 8;
        let low = |pair: u16| pair & 0xFF;
        match self {
            Self::A => high(gb.cpu_af()),
            Self::F => low(gb.cpu_af()),
            Self::B => high(gb.cpu_bc()),
            Self::C => low(gb.cpu_bc()),
            Self::D => high(gb.cpu_de()),
            Self::E => low(gb.cpu_de()),
            Self::H => high(gb.cpu_hl()),
            Self::L => low(gb.cpu_hl()),
            Self::Af => gb.cpu_af(),
            Self::Bc => gb.cpu_bc(),
            Self::De => gb.cpu_de(),
            Self::Hl => gb.cpu_hl(),
            Self::Sp => gb.cpu_sp(),
            Self::Pc => gb.cpu_pc(),
        }
    }

    fn write<A: AudioCallback>(self, gb: &mut Gb<A>, value: u16) {
        // An 8-bit register is half of a pair.
        let high = |pair: u16| u16::from_le_bytes([pair.to_le_bytes()[0], value.to_le_bytes()[0]]);
        let low = |pair: u16| u16::from_le_bytes([value.to_le_bytes()[0], pair.to_le_bytes()[1]]);
        match self {
            Self::A => gb.set_cpu_af(high(gb.cpu_af())),
            Self::F => gb.set_cpu_af(low(gb.cpu_af())),
            Self::B => gb.set_cpu_bc(high(gb.cpu_bc())),
            Self::C => gb.set_cpu_bc(low(gb.cpu_bc())),
            Self::D => gb.set_cpu_de(high(gb.cpu_de())),
            Self::E => gb.set_cpu_de(low(gb.cpu_de())),
            Self::H => gb.set_cpu_hl(high(gb.cpu_hl())),
            Self::L => gb.set_cpu_hl(low(gb.cpu_hl())),
            Self::Af => gb.set_cpu_af(value),
            Self::Bc => gb.set_cpu_bc(value),
            Self::De => gb.set_cpu_de(value),
            Self::Hl => gb.set_cpu_hl(value),
            Self::Sp => gb.set_cpu_sp(value),
            Self::Pc => gb.set_cpu_pc(value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Xor,
    Or,
    LogicAnd,
    LogicOr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Complement,
}

/// A parsed expression; names are resolved when it's parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Number(u16),
    Register(Register),
    Symbol(Value),
    Old,
    New,
    Byte(Box<Self>),
    Word(Box<Self>),
    Unary(UnOp, Box<Self>),
    Binary(BinOp, Box<Self>, Box<Self>),
    Banked(Box<Self>, Box<Self>),
    Assign(Box<Self>, Box<Self>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Number(u16),
    Name(String),
    /// An operator or a bracket.
    Punct(&'static str),
}

/// The operators and brackets, longest first so `<=` isn't `<` then `=`.
const PUNCTUATION: [&str; 28] = [
    "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "+", "-", "*", "/", "%", "&", "|", "^", "~",
    "!", "<", ">", "=", ":", "(", ")", "[", "]", "{", "}",
];

fn lex(text: &str) -> Result<Vec<Token>, String> {
    if let Some(c) = text.chars().find(|c| !c.is_ascii()) {
        return Err(format!("unexpected `{c}`"));
    }
    // ASCII only from here: any byte index is a character boundary.
    let mut tokens = Vec::new();
    let mut rest = text.trim_start().as_bytes();
    while let Some(&first) = rest.first() {
        let binary_number =
            first == b'%' && !after_operand(&tokens) && matches!(rest.get(1), Some(b'0' | b'1'));
        let (token, len) = if first == b'$' || first.is_ascii_digit() || binary_number {
            number(rest)?
        } else if first.is_ascii_alphabetic() || first == b'_' || first == b'.' {
            let len = rest
                .iter()
                .position(|&byte| !(byte.is_ascii_alphanumeric() || b"_.@#$".contains(&byte)))
                .unwrap_or(rest.len());
            (Token::Name(ascii(&rest[..len])), len)
        } else if let Some(&punct) = PUNCTUATION
            .iter()
            .find(|punct| rest.starts_with(punct.as_bytes()))
        {
            (Token::Punct(punct), punct.len())
        } else {
            return Err(format!("unexpected `{}`", char::from(first)));
        };
        tokens.push(token);
        rest = rest[len..].trim_ascii_start();
    }
    Ok(tokens)
}

/// ASCII bytes as a string.
fn ascii(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| char::from(byte)).collect()
}

/// Whether the last token ends an operand (so `%` there is modulo).
fn after_operand(tokens: &[Token]) -> bool {
    matches!(
        tokens.last(),
        Some(Token::Number(_) | Token::Name(_) | Token::Punct(")" | "]" | "}"))
    )
}

/// A number at the start of `text`, and its length.
fn number(text: &[u8]) -> Result<(Token, usize), String> {
    let (radix, prefix) = match text {
        [b'$', ..] => (16, 1),
        [b'0', b'x' | b'X', ..] => (16, 2),
        [b'%', ..] => (2, 1),
        _ => (10, 0),
    };
    let digits = &text[prefix..];
    let len = digits
        .iter()
        .position(|&byte| !char::from(byte).is_digit(radix) && byte != b'_')
        .unwrap_or(digits.len());
    let literal: String = ascii(&digits[..len]).replace('_', "");
    let value = u32::from_str_radix(&literal, radix)
        .map_err(|_err| format!("bad number `{}`", ascii(&text[..prefix + len])))?;
    let value = u16::try_from(value).map_err(|_err| format!("{value} is over 16 bits"))?;
    Ok((Token::Number(value), prefix + len))
}

struct Parser<'a> {
    tokens: Vec<Token>,
    at: usize,
    symbols: Option<&'a Symbols>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).cloned();
        self.at += 1;
        token
    }

    fn expect(&mut self, punct: &str) -> Result<(), String> {
        match self.next() {
            Some(Token::Punct(found)) if found == punct => Ok(()),
            _ => Err(format!("expected `{punct}`")),
        }
    }

    /// An expression whose operators bind at least as tightly as `min`.
    fn expr(&mut self, min: u8) -> Result<Expr, String> {
        let mut left = self.operand()?;
        while let Some(&Token::Punct(punct)) = self.peek() {
            let Some((power, op)) = binary(punct) else {
                break;
            };
            if power < min {
                break;
            }
            self.at += 1;
            left = match op {
                Infix::Assign => {
                    // Right-associative: `a = b = 1`.
                    let right = self.expr(power)?;
                    Expr::Assign(Box::new(left), Box::new(right))
                }
                Infix::Bank => {
                    let right = self.expr(power + 1)?;
                    Expr::Banked(Box::new(left), Box::new(right))
                }
                Infix::Op(op) => {
                    let right = self.expr(power + 1)?;
                    Expr::Binary(op, Box::new(left), Box::new(right))
                }
            };
        }
        Ok(left)
    }

    fn operand(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Token::Number(value)) => Ok(Expr::Number(value)),
            Some(Token::Name(name)) => self.name(&name),
            Some(Token::Punct("(")) => {
                let inner = self.expr(0)?;
                self.expect(")")?;
                Ok(inner)
            }
            Some(Token::Punct("[")) => {
                let inner = self.expr(0)?;
                self.expect("]")?;
                Ok(Expr::Byte(Box::new(inner)))
            }
            Some(Token::Punct("{")) => {
                let inner = self.expr(0)?;
                self.expect("}")?;
                Ok(Expr::Word(Box::new(inner)))
            }
            Some(Token::Punct(punct @ ("-" | "!" | "~"))) => {
                let op = match punct {
                    "-" => UnOp::Neg,
                    "!" => UnOp::Not,
                    _ => UnOp::Complement,
                };
                Ok(Expr::Unary(op, Box::new(self.expr(UNARY)?)))
            }
            Some(Token::Punct(punct)) => Err(format!("unexpected `{punct}`")),
            None => Err("the expression ends too soon".to_owned()),
        }
    }

    fn name(&self, name: &str) -> Result<Expr, String> {
        if let Some(register) = Register::parse(name) {
            return Ok(Expr::Register(register));
        }
        match name {
            "old" => return Ok(Expr::Old),
            "new" => return Ok(Expr::New),
            _ => {}
        }
        let symbol = self
            .symbols
            .and_then(|symbols| symbols.find(name))
            .ok_or_else(|| format!("no register or symbol `{name}`"))?;
        Ok(Expr::Symbol(Value {
            bank: Some(symbol.bank),
            value: symbol.addr,
        }))
    }
}

enum Infix {
    Op(BinOp),
    Bank,
    Assign,
}

/// How tightly unary operators bind.
const UNARY: u8 = 12;

/// An infix operator's binding power (higher binds tighter).
fn binary(punct: &str) -> Option<(u8, Infix)> {
    use BinOp::{
        Add, And, Div, Eq, Ge, Gt, Le, LogicAnd, LogicOr, Lt, Mul, Ne, Or, Rem, Shl, Shr, Sub, Xor,
    };
    Some(match punct {
        "=" => (1, Infix::Assign),
        "||" => (2, Infix::Op(LogicOr)),
        "&&" => (3, Infix::Op(LogicAnd)),
        "|" => (4, Infix::Op(Or)),
        "^" => (5, Infix::Op(Xor)),
        "&" => (6, Infix::Op(And)),
        "==" => (7, Infix::Op(Eq)),
        "!=" => (7, Infix::Op(Ne)),
        "<" => (8, Infix::Op(Lt)),
        "<=" => (8, Infix::Op(Le)),
        ">" => (8, Infix::Op(Gt)),
        ">=" => (8, Infix::Op(Ge)),
        "<<" => (9, Infix::Op(Shl)),
        ">>" => (9, Infix::Op(Shr)),
        "+" => (10, Infix::Op(Add)),
        "-" => (10, Infix::Op(Sub)),
        "*" => (11, Infix::Op(Mul)),
        "/" => (11, Infix::Op(Div)),
        "%" => (11, Infix::Op(Rem)),
        ":" => (13, Infix::Bank),
        _ => return None,
    })
}

impl Expr {
    /// Parses `text`, looking names up in `symbols`.
    ///
    /// # Errors
    ///
    /// Returns what's wrong with the expression.
    #[inline]
    pub fn parse(text: &str, symbols: Option<&Symbols>) -> Result<Self, String> {
        let mut parser = Parser {
            tokens: lex(text)?,
            at: 0,
            symbols,
        };
        let expr = parser.expr(0)?;
        match parser.peek() {
            None => Ok(expr),
            Some(Token::Punct(punct)) => Err(format!("unexpected `{punct}`")),
            Some(Token::Number(value)) => Err(format!("unexpected {value}")),
            Some(Token::Name(name)) => Err(format!("unexpected `{name}`")),
        }
    }

    /// Evaluates the expression (and does its assignments). `watched` gives
    /// `old` and `new` their values.
    ///
    /// # Errors
    ///
    /// Returns why it can't be evaluated: a division by zero, `old` outside
    /// a watchpoint, a bank that isn't mapped.
    #[inline]
    pub fn eval<A: AudioCallback>(
        &self,
        gb: &mut Gb<A>,
        watched: Option<Watched>,
    ) -> Result<Value, String> {
        Ok(match self {
            Self::Number(value) => Value::plain(*value),
            Self::Register(register) => Value::plain(register.read(gb)),
            Self::Symbol(value) => *value,
            Self::Old | Self::New => {
                let watched = watched.ok_or("`old` and `new` are for watchpoint conditions")?;
                Value::plain(u16::from(if matches!(self, Self::Old) {
                    watched.old
                } else {
                    watched.new
                }))
            }
            Self::Byte(addr) => {
                let addr = addr.eval(gb, watched)?;
                Value::plain(read(gb, addr)?.into())
            }
            Self::Word(addr) => {
                let addr = addr.eval(gb, watched)?;
                let next = Value {
                    value: addr.value.wrapping_add(1),
                    ..addr
                };
                Value::plain(u16::from_le_bytes([read(gb, addr)?, read(gb, next)?]))
            }
            Self::Unary(op, operand) => {
                let value = operand.eval(gb, watched)?.value;
                Value::plain(match op {
                    UnOp::Neg => value.wrapping_neg(),
                    UnOp::Not => u16::from(value == 0),
                    UnOp::Complement => !value,
                })
            }
            Self::Binary(op, left, right) => {
                let left = left.eval(gb, watched)?;
                let right = right.eval(gb, watched)?;
                binary_value(*op, left, right)?
            }
            Self::Banked(bank, addr) => {
                let bank = bank.eval(gb, watched)?.value;
                let addr = addr.eval(gb, watched)?.value;
                Value {
                    bank: Some(Bank::Number(bank.into())),
                    value: addr,
                }
            }
            Self::Assign(target, value) => {
                let value = value.eval(gb, watched)?;
                assign(gb, target, value, watched)?;
                value
            }
        })
    }
}

fn binary_value(op: BinOp, left: Value, right: Value) -> Result<Value, String> {
    let (a, b) = (left.value, right.value);
    let value = match op {
        BinOp::Mul => a.wrapping_mul(b),
        BinOp::Div => a.checked_div(b).ok_or("division by zero")?,
        BinOp::Rem => a.checked_rem(b).ok_or("division by zero")?,
        // An address plus or minus an offset stays in its bank.
        BinOp::Add => {
            return Ok(Value {
                bank: left.bank.or(right.bank),
                value: a.wrapping_add(b),
            });
        }
        BinOp::Sub => {
            return Ok(Value {
                bank: left.bank,
                value: a.wrapping_sub(b),
            });
        }
        BinOp::Shl => a.checked_shl(b.into()).unwrap_or(0),
        BinOp::Shr => a.checked_shr(b.into()).unwrap_or(0),
        BinOp::Lt => u16::from(a < b),
        BinOp::Le => u16::from(a <= b),
        BinOp::Gt => u16::from(a > b),
        BinOp::Ge => u16::from(a >= b),
        BinOp::Eq => u16::from(a == b),
        BinOp::Ne => u16::from(a != b),
        BinOp::And => a & b,
        BinOp::Xor => a ^ b,
        BinOp::Or => a | b,
        BinOp::LogicAnd => u16::from(a != 0 && b != 0),
        BinOp::LogicOr => u16::from(a != 0 || b != 0),
    };
    Ok(Value::plain(value))
}

/// The byte at `addr`. A ROM address in a bank reads that bank, mapped or
/// not; other banked memory can only be read in the bank mapped.
fn read<A: AudioCallback>(gb: &Gb<A>, addr: Value) -> Result<u8, String> {
    match addr.bank {
        Some(bank @ Bank::Number(number)) if addr.value < 0x8000 => {
            if mapped_bank(gb, addr.value) == bank {
                Ok(gb.peek_mem(addr.value))
            } else {
                Ok(gb.peek_rom(number, addr.value))
            }
        }
        Some(bank) if mapped_bank(gb, addr.value) != bank => Err(format!(
            "{bank}:{:04X} isn't mapped (bank {} is)",
            addr.value,
            mapped_bank(gb, addr.value)
        )),
        _ => Ok(gb.peek_mem(addr.value)),
    }
}

fn assign<A: AudioCallback>(
    gb: &mut Gb<A>,
    target: &Expr,
    value: Value,
    watched: Option<Watched>,
) -> Result<(), String> {
    let [low, high] = value.value.to_le_bytes();
    match target {
        Expr::Register(register) => register.write(gb, value.value),
        Expr::Byte(addr) => {
            let addr = addr.eval(gb, watched)?;
            let addr = writable(gb, addr)?;
            gb.poke_mem(addr, low);
        }
        Expr::Word(addr) => {
            let addr = addr.eval(gb, watched)?;
            let addr = writable(gb, addr)?;
            gb.poke_mem(addr, low);
            gb.poke_mem(addr.wrapping_add(1), high);
        }
        _ => return Err("only a register or memory (a byte or a word) can be assigned".to_owned()),
    }
    Ok(())
}

/// An address that can be written: in its bank if it has one.
fn writable<A: AudioCallback>(gb: &Gb<A>, addr: Value) -> Result<u16, String> {
    match addr.bank {
        Some(bank) if mapped_bank(gb, addr.value) != bank => Err(format!(
            "{bank}:{:04X} isn't mapped (bank {} is)",
            addr.value,
            mapped_bank(gb, addr.value)
        )),
        _ => Ok(addr.value),
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests")]
mod tests {
    use super::{Bank, Expr, Value, Watched};
    use crate::debugger::symbols::Symbols;
    use ceres_core::{AudioCallback, Gb, GbBuilder, Model, Sample};

    struct Silent;

    impl AudioCallback for Silent {
        fn audio_sample(&self, _: Sample, _: Sample) {}
    }

    /// A 64 KiB MBC1 ROM whose bank 2 starts with $22, after a boot.
    fn gb() -> Gb<Silent> {
        let mut rom = vec![0; 0x1_0000];
        rom[0x147] = 0x01;
        rom[0x148] = 0x01;
        rom[0x8000] = 0x22;
        GbBuilder::new(48000, Silent)
            .with_model(Model::DmgB)
            .with_run_bootrom(false)
            .with_rom(rom.into_boxed_slice())
            .unwrap()
            .build()
    }

    const SYM: &str = "00:0150 Main\n02:4000 BankTwo\n00:c000 wCounter\n";

    fn eval(text: &str, gb: &mut Gb<Silent>) -> Result<u16, String> {
        let symbols = Symbols::parse(SYM);
        Expr::parse(text, Some(&symbols))?
            .eval(gb, None)
            .map(|value| value.value)
    }

    #[test]
    fn numbers_and_precedence() {
        let mut gb = gb();
        assert_eq!(eval("10", &mut gb), Ok(10));
        assert_eq!(eval("$1F", &mut gb), Ok(0x1F));
        assert_eq!(eval("0x1F", &mut gb), Ok(0x1F));
        assert_eq!(eval("%1010", &mut gb), Ok(10));
        assert_eq!(eval("7 % 4", &mut gb), Ok(3));
        assert_eq!(eval("1 + 2 * 3", &mut gb), Ok(7));
        assert_eq!(eval("(1 + 2) * 3", &mut gb), Ok(9));
        assert_eq!(eval("1 << 4 | 1", &mut gb), Ok(17));
        assert_eq!(eval("1 + 1 == 2 && 3 > 2", &mut gb), Ok(1));
        assert_eq!(eval("-1", &mut gb), Ok(0xFFFF));
        assert_eq!(eval("!0 + ~0", &mut gb), Ok(0));
        assert_eq!(eval("1 / 0", &mut gb), Err("division by zero".to_owned()));
        assert_eq!(
            eval("1 +", &mut gb),
            Err("the expression ends too soon".to_owned())
        );
        assert_eq!(
            eval("nope", &mut gb),
            Err("no register or symbol `nope`".to_owned())
        );
        assert_eq!(
            eval("$10000", &mut gb),
            Err("65536 is over 16 bits".to_owned())
        );
    }

    #[test]
    fn registers_and_memory() {
        let mut gb = gb();
        assert_eq!(eval("a = $12", &mut gb), Ok(0x12));
        assert_eq!(gb.cpu_af() >> 8, 0x12);
        assert_eq!(eval("hl = wCounter", &mut gb), Ok(0xC000));
        assert_eq!(eval("[hl] = 5", &mut gb), Ok(5));
        assert_eq!(eval("[wCounter]", &mut gb), Ok(5));
        assert_eq!(eval("{$C000} = $1234", &mut gb), Ok(0x1234));
        assert_eq!(eval("[$C001]", &mut gb), Ok(0x12));
        assert_eq!(eval("l", &mut gb), Ok(0));
        assert_eq!(
            eval("1 = 2", &mut gb),
            Err("only a register or memory (a byte or a word) can be assigned".to_owned())
        );
    }

    #[test]
    fn banks() {
        let mut gb = gb();
        let symbols = Symbols::parse(SYM);
        let bank_two = Expr::parse("BankTwo + 1", Some(&symbols)).unwrap();
        assert_eq!(
            bank_two.eval(&mut gb, None),
            Ok(Value {
                bank: Some(Bank::Number(2)),
                value: 0x4001
            })
        );
        // Bank 2 isn't mapped (bank 1 is), but ROM can be read anyway.
        assert_eq!(eval("[BankTwo]", &mut gb), Ok(0x22));
        assert_eq!(eval("[2:$4000]", &mut gb), Ok(0x22));
        assert_eq!(eval("[$4000]", &mut gb), Ok(0));
        // Other banked memory can't be.
        assert_eq!(
            eval("[3:$D000]", &mut gb),
            Err("03:D000 isn't mapped (bank 01 is)".to_owned())
        );
    }

    #[test]
    fn watch_values() {
        let mut gb = gb();
        let condition = Expr::parse("new == old + 1", None).unwrap();
        let hit = Watched { old: 4, new: 5 };
        assert_eq!(condition.eval(&mut gb, Some(hit)).map(|v| v.value), Ok(1));
        assert_eq!(
            condition.eval(&mut gb, None),
            Err("`old` and `new` are for watchpoint conditions".to_owned())
        );
    }
}
