//! An SM83 disassembler, in RGBDS syntax.
//!
//! An instruction's text has a `{}` where its operand goes, so a debugger
//! can put a label there instead of a number (`Instruction::text`).

use {
    alloc::{format, string::String},
    core::fmt::{self, Display, Formatter},
};

/// What an instruction's operand is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Operand {
    None,
    /// An 8-bit value.
    Byte(u8),
    /// A 16-bit value.
    Word(u16),
    /// The address of data (`ld a, [$C000]`, `ldh [$FF44], a`).
    Data(u16),
    /// Where a jump, call or restart goes.
    Code(u16),
    /// A signed offset added to SP.
    Offset(i8),
}

/// A decoded instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Instruction {
    /// Its length in bytes, 1 to 3.
    pub len: u8,
    /// Its text, with `{}` where the operand goes.
    pub template: String,
    pub operand: Operand,
}

impl Instruction {
    fn new(len: u8, template: impl Into<String>, operand: Operand) -> Self {
        Self {
            len,
            template: template.into(),
            operand,
        }
    }

    /// The instruction's text, with `name` naming addresses where it can
    /// (a label), else `$`-prefixed hex.
    #[must_use]
    #[inline]
    pub fn text<F: FnOnce(u16) -> Option<String>>(&self, name: F) -> String {
        let operand = match self.operand {
            Operand::None => return self.template.clone(),
            Operand::Byte(value) => format!("${value:02X}"),
            Operand::Word(value) => format!("${value:04X}"),
            Operand::Data(addr) | Operand::Code(addr) => {
                name(addr).unwrap_or_else(|| format!("${addr:04X}"))
            }
            Operand::Offset(offset) => format!("{offset:+}"),
        };
        self.template.replacen("{}", &operand, 1)
    }
}

const R8: [&str; 8] = ["b", "c", "d", "e", "h", "l", "[hl]", "a"];
const R16: [&str; 4] = ["bc", "de", "hl", "sp"];
const R16_STACK: [&str; 4] = ["bc", "de", "hl", "af"];
const R16_MEMORY: [&str; 4] = ["[bc]", "[de]", "[hl+]", "[hl-]"];
const CONDITIONS: [&str; 4] = ["nz", "z", "nc", "c"];
/// The arithmetic and logic operations, by bits 3-5, with what comes before
/// their operand.
const ALU: [&str; 8] = [
    "add a, ", "adc a, ", "sub ", "sbc a, ", "and ", "xor ", "or ", "cp ",
];
const ROTATIONS: [&str; 8] = ["rlc", "rrc", "rl", "rr", "sla", "sra", "swap", "srl"];
const SINGLE: [&str; 8] = ["rlca", "rrca", "rla", "rra", "daa", "cpl", "scf", "ccf"];

/// Decodes the instruction in `bytes` (the bytes at `pc` and after).
#[must_use]
#[inline]
pub fn disassemble(pc: u16, bytes: [u8; 3]) -> Instruction {
    let [op, lo, hi] = bytes;
    let word = u16::from_le_bytes([lo, hi]);
    // Where a relative jump goes, from after the instruction.
    let relative = pc
        .wrapping_add(2)
        .wrapping_add_signed(i16::from(lo.cast_signed()));
    let y = usize::from((op >> 3) & 7);
    let z = op & 7;
    let p = y >> 1;
    let odd = y & 1 == 1;
    let r8 = |i: u8| R8[usize::from(i)];

    match op >> 6 {
        0 => match z {
            0 => match y {
                0 => Instruction::new(1, "nop", Operand::None),
                1 => Instruction::new(3, "ld [{}], sp", Operand::Data(word)),
                2 => Instruction::new(2, "stop", Operand::None),
                3 => Instruction::new(2, "jr {}", Operand::Code(relative)),
                _ => Instruction::new(
                    2,
                    format!("jr {}, {{}}", CONDITIONS[y - 4]),
                    Operand::Code(relative),
                ),
            },
            1 if odd => Instruction::new(1, format!("add hl, {}", R16[p]), Operand::None),
            1 => Instruction::new(3, format!("ld {}, {{}}", R16[p]), Operand::Word(word)),
            2 if odd => Instruction::new(1, format!("ld a, {}", R16_MEMORY[p]), Operand::None),
            2 => Instruction::new(1, format!("ld {}, a", R16_MEMORY[p]), Operand::None),
            3 if odd => Instruction::new(1, format!("dec {}", R16[p]), Operand::None),
            3 => Instruction::new(1, format!("inc {}", R16[p]), Operand::None),
            4 => Instruction::new(1, format!("inc {}", R8[y]), Operand::None),
            5 => Instruction::new(1, format!("dec {}", R8[y]), Operand::None),
            6 => Instruction::new(2, format!("ld {}, {{}}", R8[y]), Operand::Byte(lo)),
            _ => Instruction::new(1, SINGLE[y], Operand::None),
        },
        1 if op == 0x76 => Instruction::new(1, "halt", Operand::None),
        1 => Instruction::new(1, format!("ld {}, {}", R8[y], r8(z)), Operand::None),
        2 => Instruction::new(1, format!("{}{}", ALU[y], r8(z)), Operand::None),
        _ => match z {
            0 => match y {
                0..=3 => Instruction::new(1, format!("ret {}", CONDITIONS[y]), Operand::None),
                4 => Instruction::new(2, "ldh [{}], a", Operand::Data(0xFF00 | u16::from(lo))),
                5 => Instruction::new(2, "add sp, {}", Operand::Offset(lo.cast_signed())),
                6 => Instruction::new(2, "ldh a, [{}]", Operand::Data(0xFF00 | u16::from(lo))),
                _ => Instruction::new(2, "ld hl, sp{}", Operand::Offset(lo.cast_signed())),
            },
            1 if !odd => Instruction::new(1, format!("pop {}", R16_STACK[p]), Operand::None),
            1 => Instruction::new(1, ["ret", "reti", "jp hl", "ld sp, hl"][p], Operand::None),
            2 => match y {
                0..=3 => Instruction::new(
                    3,
                    format!("jp {}, {{}}", CONDITIONS[y]),
                    Operand::Code(word),
                ),
                4 => Instruction::new(1, "ldh [c], a", Operand::None),
                5 => Instruction::new(3, "ld [{}], a", Operand::Data(word)),
                6 => Instruction::new(1, "ldh a, [c]", Operand::None),
                _ => Instruction::new(3, "ld a, [{}]", Operand::Data(word)),
            },
            3 => match y {
                0 => Instruction::new(3, "jp {}", Operand::Code(word)),
                1 => prefixed(lo),
                6 => Instruction::new(1, "di", Operand::None),
                7 => Instruction::new(1, "ei", Operand::None),
                _ => illegal(op),
            },
            4 => match y {
                0..=3 => Instruction::new(
                    3,
                    format!("call {}, {{}}", CONDITIONS[y]),
                    Operand::Code(word),
                ),
                _ => illegal(op),
            },
            5 if !odd => Instruction::new(1, format!("push {}", R16_STACK[p]), Operand::None),
            5 if p == 0 => Instruction::new(3, "call {}", Operand::Code(word)),
            5 => illegal(op),
            6 => Instruction::new(2, format!("{}{{}}", ALU[y]), Operand::Byte(lo)),
            _ => Instruction::new(1, "rst {}", Operand::Code(u16::from(op & 0x38))),
        },
    }
}

/// An instruction after the 0xCB prefix.
fn prefixed(op: u8) -> Instruction {
    let y = usize::from((op >> 3) & 7);
    let register = R8[usize::from(op & 7)];
    let text = match op >> 6 {
        0 => format!("{} {register}", ROTATIONS[y]),
        1 => format!("bit {y}, {register}"),
        2 => format!("res {y}, {register}"),
        _ => format!("set {y}, {register}"),
    };
    Instruction::new(2, text, Operand::None)
}

/// An opcode the SM83 doesn't have (it locks up), as a byte.
fn illegal(op: u8) -> Instruction {
    Instruction::new(1, format!("db ${op:02X}"), Operand::None)
}

impl Display for Instruction {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text(|_| None))
    }
}

#[cfg(test)]
mod tests {
    use super::{Operand, disassemble};
    use alloc::string::{String, ToString as _};

    /// The text with no labels.
    fn plain(pc: u16, bytes: [u8; 3]) -> String {
        disassemble(pc, bytes).to_string()
    }

    /// The documented lengths of the unprefixed opcodes, a row per high
    /// nibble (0 for an opcode that doesn't exist, which disassembles as one
    /// byte).
    const LENGTHS: [[u8; 16]; 16] = [
        [1, 3, 1, 1, 1, 1, 2, 1, 3, 1, 1, 1, 1, 1, 2, 1],
        [2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1],
        [2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1],
        [2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1],
        [1; 16],
        [1; 16],
        [1; 16],
        [1; 16],
        [1; 16],
        [1; 16],
        [1; 16],
        [1; 16],
        [1, 1, 3, 3, 3, 1, 2, 1, 1, 1, 3, 2, 3, 3, 2, 1],
        [1, 1, 3, 0, 3, 1, 2, 1, 1, 1, 3, 0, 3, 0, 2, 1],
        [2, 1, 1, 0, 0, 1, 2, 1, 2, 1, 3, 0, 0, 0, 2, 1],
        [2, 1, 1, 1, 0, 1, 2, 1, 2, 1, 3, 1, 0, 0, 2, 1],
    ];

    #[test]
    fn lengths() {
        for op in 0..=0xFF_u8 {
            let expected = LENGTHS[usize::from(op >> 4)][usize::from(op & 0xF)];
            let instruction = disassemble(0, [op, 0, 0]);
            assert_eq!(
                instruction.len,
                expected.max(1),
                "{op:02X}: {}",
                instruction.template
            );
            assert_eq!(
                instruction.template.starts_with("db "),
                expected == 0,
                "{op:02X}: {}",
                instruction.template
            );
        }
        for op in 0..=0xFF_u8 {
            assert_eq!(disassemble(0, [0xCB, op, 0]).len, 2, "CB {op:02X}");
        }
    }

    #[test]
    fn text() {
        assert_eq!(plain(0, [0x00, 0, 0]), "nop");
        assert_eq!(plain(0, [0x3E, 0x42, 0]), "ld a, $42");
        assert_eq!(plain(0, [0x21, 0x00, 0xC0]), "ld hl, $C000");
        assert_eq!(plain(0, [0x2A, 0, 0]), "ld a, [hl+]");
        assert_eq!(plain(0, [0x70, 0, 0]), "ld [hl], b");
        assert_eq!(plain(0, [0x76, 0, 0]), "halt");
        assert_eq!(plain(0, [0x90, 0, 0]), "sub b");
        assert_eq!(plain(0, [0x8E, 0, 0]), "adc a, [hl]");
        assert_eq!(plain(0, [0xFE, 0x90, 0]), "cp $90");
        assert_eq!(plain(0, [0xEA, 0x00, 0xD0]), "ld [$D000], a");
        assert_eq!(plain(0, [0xF0, 0x44, 0]), "ldh a, [$FF44]");
        assert_eq!(plain(0, [0xE2, 0, 0]), "ldh [c], a");
        assert_eq!(plain(0, [0x08, 0x34, 0x12]), "ld [$1234], sp");
        assert_eq!(plain(0, [0xE8, 0xFB, 0]), "add sp, -5");
        assert_eq!(plain(0, [0xF8, 0x05, 0]), "ld hl, sp+5");
        assert_eq!(plain(0, [0xC1, 0, 0]), "pop bc");
        assert_eq!(plain(0, [0xF5, 0, 0]), "push af");
        assert_eq!(plain(0, [0xD9, 0, 0]), "reti");
        assert_eq!(plain(0, [0xE9, 0, 0]), "jp hl");
        assert_eq!(plain(0, [0xFF, 0, 0]), "rst $0038");
        assert_eq!(plain(0, [0xCD, 0x5A, 0x04]), "call $045A");
        assert_eq!(plain(0, [0xC4, 0x5A, 0x04]), "call nz, $045A");
        assert_eq!(plain(0, [0xDA, 0x00, 0x40]), "jp c, $4000");
        assert_eq!(plain(0, [0xD3, 0, 0]), "db $D3");
        assert_eq!(plain(0, [0xCB, 0x37, 0]), "swap a");
        assert_eq!(plain(0, [0xCB, 0x7E, 0]), "bit 7, [hl]");
        assert_eq!(plain(0, [0xCB, 0xC1, 0]), "set 0, c");
        assert_eq!(plain(0, [0x10, 0x00, 0]), "stop");
        assert_eq!(plain(0, [0xFB, 0, 0]), "ei");
    }

    #[test]
    fn relative_jumps() {
        // `jr` counts from after itself.
        assert_eq!(plain(0x0150, [0x18, 0xFE, 0]), "jr $0150");
        assert_eq!(plain(0x0150, [0x20, 0x10, 0]), "jr nz, $0162");
        assert_eq!(
            disassemble(0x0000, [0x38, 0xFC, 0]).operand,
            Operand::Code(0xFFFE)
        );
    }

    #[test]
    fn labels() {
        let call = disassemble(0, [0xCD, 0x5A, 0x04]);
        let named = call.text(|addr| (addr == 0x045A).then(|| "DelayFrame".into()));
        assert_eq!(named, "call DelayFrame");
        let unnamed = call.text(|_| None);
        assert_eq!(unnamed, "call $045A");
    }
}
