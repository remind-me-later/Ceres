//! SM83 CPU disassembler
//!
//! This module provides disassembly functionality for the Game Boy's SM83 CPU,
//! which is based on a mix of 8080 and Z80 instruction sets.
//!
//! The disassembler supports all 256 base opcodes and 256 CB-prefixed opcodes,
//! formatting output using RGBDS assembler syntax for compatibility with
//! Game Boy development tools.
//!
//! # Example
//!
//! ```
//! use ceres_core::disasm::disasm;
//!
//! let bytes = [0x3E, 0xFF]; // LD A, $FF
//! let result = disasm(&bytes, 0x0150);
//! assert_eq!(result.mnemonic.as_str(), "LD A, $FF");
//! assert_eq!(result.length, 2);
//! ```

use core::fmt::Write as _;
use heapless::String;

/// Result of disassembling an instruction
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisasmResult {
    /// The assembly mnemonic (e.g., "LD A, B", "JP $1234")
    pub mnemonic: String<32>,
    /// The length of the instruction in bytes (1-3)
    pub length: u8,
}

/// Disassemble an instruction from a byte slice
///
/// # Arguments
///
/// * `bytes` - The instruction bytes to disassemble
/// * `pc` - The program counter (used for calculating jump targets)
///
/// # Returns
///
/// A `DisasmResult` containing the mnemonic and instruction length
#[inline]
#[must_use]
pub fn disasm(bytes: &[u8], pc: u16) -> DisasmResult {
    if bytes.is_empty() {
        return DisasmResult {
            mnemonic: String::try_from("???").unwrap(),
            length: 1,
        };
    }

    let opcode = bytes[0];

    // CB-prefixed instructions
    if opcode == 0xCB {
        if bytes.len() < 2 {
            return DisasmResult {
                mnemonic: String::try_from("CB $??").unwrap(),
                length: 2,
            };
        }
        return disasm_cb(bytes[1]);
    }

    disasm_base(bytes, pc, opcode)
}

/// Disassemble a CB-prefixed instruction
#[inline]
#[must_use]
pub fn disasm_cb(opcode: u8) -> DisasmResult {
    let mut mnemonic = String::new();

    let bit = (opcode >> 3) & 0x07;
    let reg = opcode & 0x07;
    let reg_name = get_reg_name(reg);

    let instruction = match opcode {
        // RLC r8
        0x00..=0x07 => {
            let _ = write!(mnemonic, "RLC {reg_name}");
            mnemonic
        }
        // RRC r8
        0x08..=0x0F => {
            let _ = write!(mnemonic, "RRC {reg_name}");
            mnemonic
        }
        // RL r8
        0x10..=0x17 => {
            let _ = write!(mnemonic, "RL {reg_name}");
            mnemonic
        }
        // RR r8
        0x18..=0x1F => {
            let _ = write!(mnemonic, "RR {reg_name}");
            mnemonic
        }
        // SLA r8
        0x20..=0x27 => {
            let _ = write!(mnemonic, "SLA {reg_name}");
            mnemonic
        }
        // SRA r8
        0x28..=0x2F => {
            let _ = write!(mnemonic, "SRA {reg_name}");
            mnemonic
        }
        // SWAP r8
        0x30..=0x37 => {
            let _ = write!(mnemonic, "SWAP {reg_name}");
            mnemonic
        }
        // SRL r8
        0x38..=0x3F => {
            let _ = write!(mnemonic, "SRL {reg_name}");
            mnemonic
        }
        // BIT b, r8
        0x40..=0x7F => {
            let _ = write!(mnemonic, "BIT {bit}, {reg_name}");
            mnemonic
        }
        // RES b, r8
        0x80..=0xBF => {
            let _ = write!(mnemonic, "RES {bit}, {reg_name}");
            mnemonic
        }
        // SET b, r8
        0xC0..=0xFF => {
            let _ = write!(mnemonic, "SET {bit}, {reg_name}");
            mnemonic
        }
    };

    DisasmResult {
        mnemonic: instruction,
        length: 2,
    }
}

#[inline]
fn get_reg_name(reg: u8) -> &'static str {
    match reg {
        0 => "B",
        1 => "C",
        2 => "D",
        3 => "E",
        4 => "H",
        5 => "L",
        6 => "[HL]",
        7 => "A",
        _ => "?",
    }
}

#[inline]
fn get_r16_name(index: u8) -> &'static str {
    match index {
        0 => "BC",
        1 => "DE",
        2 => "HL",
        3 => "SP",
        _ => "??",
    }
}

#[inline]
fn get_r16_stk_name(index: u8) -> &'static str {
    match index {
        0 => "BC",
        1 => "DE",
        2 => "HL",
        3 => "AF",
        _ => "??",
    }
}

#[inline]
fn get_cond_name(cond: u8) -> &'static str {
    match cond {
        0 => "NZ",
        1 => "Z",
        2 => "NC",
        3 => "C",
        _ => "??",
    }
}

#[inline]
fn disasm_base(bytes: &[u8], pc: u16, opcode: u8) -> DisasmResult {
    let mut mnemonic = String::new();

    let y = (opcode >> 3) & 0x07;
    let z = opcode & 0x07;
    let p = (opcode >> 4) & 0x03;

    match opcode {
        0x00 => DisasmResult {
            mnemonic: String::try_from("NOP").unwrap(),
            length: 1,
        },
        0x08 => {
            let addr = read_u16(bytes, 1);
            let _ = write!(mnemonic, "LD [${addr:04X}], SP");
            DisasmResult {
                mnemonic,
                length: 3,
            }
        }
        0x10 => DisasmResult {
            mnemonic: String::try_from("STOP").unwrap(),
            length: 2,
        },
        0x18 => {
            let offset = read_i8(bytes, 1);
            let target = pc.wrapping_add(2).wrapping_add_signed(i16::from(offset));
            let _ = write!(mnemonic, "JR ${target:04X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0x20 | 0x28 | 0x30 | 0x38 => {
            let cond = get_cond_name((opcode >> 3) & 0x03);
            let offset = read_i8(bytes, 1);
            let target = pc.wrapping_add(2).wrapping_add_signed(i16::from(offset));
            let _ = write!(mnemonic, "JR {cond}, ${target:04X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0x01 | 0x11 | 0x21 | 0x31 => {
            let r16 = get_r16_name(p);
            let imm = read_u16(bytes, 1);
            let _ = write!(mnemonic, "LD {r16}, ${imm:04X}");
            DisasmResult {
                mnemonic,
                length: 3,
            }
        }
        0x09 | 0x19 | 0x29 | 0x39 => {
            let r16 = get_r16_name(p);
            let _ = write!(mnemonic, "ADD HL, {r16}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x02 => DisasmResult {
            mnemonic: String::try_from("LD [BC], A").unwrap(),
            length: 1,
        },
        0x12 => DisasmResult {
            mnemonic: String::try_from("LD [DE], A").unwrap(),
            length: 1,
        },
        0x22 => DisasmResult {
            mnemonic: String::try_from("LD [HL+], A").unwrap(),
            length: 1,
        },
        0x32 => DisasmResult {
            mnemonic: String::try_from("LD [HL-], A").unwrap(),
            length: 1,
        },
        0x0A => DisasmResult {
            mnemonic: String::try_from("LD A, [BC]").unwrap(),
            length: 1,
        },
        0x1A => DisasmResult {
            mnemonic: String::try_from("LD A, [DE]").unwrap(),
            length: 1,
        },
        0x2A => DisasmResult {
            mnemonic: String::try_from("LD A, [HL+]").unwrap(),
            length: 1,
        },
        0x3A => DisasmResult {
            mnemonic: String::try_from("LD A, [HL-]").unwrap(),
            length: 1,
        },
        0x03 | 0x13 | 0x23 | 0x33 => {
            let r16 = get_r16_name(p);
            let _ = write!(mnemonic, "INC {r16}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x0B | 0x1B | 0x2B | 0x3B => {
            let r16 = get_r16_name(p);
            let _ = write!(mnemonic, "DEC {r16}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
            let reg = get_reg_name(y);
            let _ = write!(mnemonic, "INC {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
            let reg = get_reg_name(y);
            let _ = write!(mnemonic, "DEC {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
            let reg = get_reg_name(y);
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "LD {reg}, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0x07 => DisasmResult {
            mnemonic: String::try_from("RLCA").unwrap(),
            length: 1,
        },
        0x0F => DisasmResult {
            mnemonic: String::try_from("RRCA").unwrap(),
            length: 1,
        },
        0x17 => DisasmResult {
            mnemonic: String::try_from("RLA").unwrap(),
            length: 1,
        },
        0x1F => DisasmResult {
            mnemonic: String::try_from("RRA").unwrap(),
            length: 1,
        },
        0x27 => DisasmResult {
            mnemonic: String::try_from("DAA").unwrap(),
            length: 1,
        },
        0x2F => DisasmResult {
            mnemonic: String::try_from("CPL").unwrap(),
            length: 1,
        },
        0x37 => DisasmResult {
            mnemonic: String::try_from("SCF").unwrap(),
            length: 1,
        },
        0x3F => DisasmResult {
            mnemonic: String::try_from("CCF").unwrap(),
            length: 1,
        },
        0x76 => DisasmResult {
            mnemonic: String::try_from("HALT").unwrap(),
            length: 1,
        },
        0x40..=0x75 | 0x77..=0x7F => {
            let dst = get_reg_name(y);
            let src = get_reg_name(z);
            let _ = write!(mnemonic, "LD {dst}, {src}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x80..=0x87 => {
            let reg = get_reg_name(z);
            let _ = write!(mnemonic, "ADD A, {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x88..=0x8F => {
            let reg = get_reg_name(z);
            let _ = write!(mnemonic, "ADC A, {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x90..=0x97 => {
            let reg = get_reg_name(z);
            let _ = write!(mnemonic, "SUB A, {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0x98..=0x9F => {
            let reg = get_reg_name(z);
            let _ = write!(mnemonic, "SBC A, {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xA0..=0xA7 => {
            let reg = get_reg_name(z);
            let _ = write!(mnemonic, "AND A, {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xA8..=0xAF => {
            let reg = get_reg_name(z);
            let _ = write!(mnemonic, "XOR A, {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xB0..=0xB7 => {
            let reg = get_reg_name(z);
            let _ = write!(mnemonic, "OR A, {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xB8..=0xBF => {
            let reg = get_reg_name(z);
            let _ = write!(mnemonic, "CP A, {reg}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xC0 | 0xC8 | 0xD0 | 0xD8 => {
            let cond = get_cond_name((opcode >> 3) & 0x03);
            let _ = write!(mnemonic, "RET {cond}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xC1 | 0xD1 | 0xE1 | 0xF1 => {
            let r16 = get_r16_stk_name(p);
            let _ = write!(mnemonic, "POP {r16}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xC2 | 0xCA | 0xD2 | 0xDA => {
            let cond = get_cond_name((opcode >> 3) & 0x03);
            let addr = read_u16(bytes, 1);
            let _ = write!(mnemonic, "JP {cond}, ${addr:04X}");
            DisasmResult {
                mnemonic,
                length: 3,
            }
        }
        0xC3 => {
            let addr = read_u16(bytes, 1);
            let _ = write!(mnemonic, "JP ${addr:04X}");
            DisasmResult {
                mnemonic,
                length: 3,
            }
        }
        0xC4 | 0xCC | 0xD4 | 0xDC => {
            let cond = get_cond_name((opcode >> 3) & 0x03);
            let addr = read_u16(bytes, 1);
            let _ = write!(mnemonic, "CALL {cond}, ${addr:04X}");
            DisasmResult {
                mnemonic,
                length: 3,
            }
        }
        0xC5 | 0xD5 | 0xE5 | 0xF5 => {
            let r16 = get_r16_stk_name(p);
            let _ = write!(mnemonic, "PUSH {r16}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xC6 => {
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "ADD A, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xCE => {
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "ADC A, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xD6 => {
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "SUB A, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xDE => {
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "SBC A, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xE6 => {
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "AND A, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xEE => {
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "XOR A, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xF6 => {
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "OR A, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xFE => {
            let imm = read_u8(bytes, 1);
            let _ = write!(mnemonic, "CP A, ${imm:02X}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
            let addr = (opcode & 0x38) as u16;
            let _ = write!(mnemonic, "RST ${addr:02X}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
        0xC9 => DisasmResult {
            mnemonic: String::try_from("RET").unwrap(),
            length: 1,
        },
        0xD9 => DisasmResult {
            mnemonic: String::try_from("RETI").unwrap(),
            length: 1,
        },
        0xE0 => {
            let offset = read_u8(bytes, 1);
            let _ = write!(mnemonic, "LDH [$FF{offset:02X}], A");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xE2 => DisasmResult {
            mnemonic: String::try_from("LD [$FF00+C], A").unwrap(),
            length: 1,
        },
        0xE8 => {
            let offset = read_i8(bytes, 1);
            let _ = write!(mnemonic, "ADD SP, {offset}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xE9 => DisasmResult {
            mnemonic: String::try_from("JP HL").unwrap(),
            length: 1,
        },
        0xEA => {
            let addr = read_u16(bytes, 1);
            let _ = write!(mnemonic, "LD [${addr:04X}], A");
            DisasmResult {
                mnemonic,
                length: 3,
            }
        }
        0xF0 => {
            let offset = read_u8(bytes, 1);
            let _ = write!(mnemonic, "LDH A, [$FF{offset:02X}]");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xF2 => DisasmResult {
            mnemonic: String::try_from("LD A, [$FF00+C]").unwrap(),
            length: 1,
        },
        0xF3 => DisasmResult {
            mnemonic: String::try_from("DI").unwrap(),
            length: 1,
        },
        0xF8 => {
            let offset = read_i8(bytes, 1);
            let _ = write!(mnemonic, "LD HL, SP+{offset}");
            DisasmResult {
                mnemonic,
                length: 2,
            }
        }
        0xF9 => DisasmResult {
            mnemonic: String::try_from("LD SP, HL").unwrap(),
            length: 1,
        },
        0xFA => {
            let addr = read_u16(bytes, 1);
            let _ = write!(mnemonic, "LD A, [${addr:04X}]");
            DisasmResult {
                mnemonic,
                length: 3,
            }
        }
        0xFB => DisasmResult {
            mnemonic: String::try_from("EI").unwrap(),
            length: 1,
        },
        0xCD => {
            let addr = read_u16(bytes, 1);
            let _ = write!(mnemonic, "CALL ${addr:04X}");
            DisasmResult {
                mnemonic,
                length: 3,
            }
        }
        _ => {
            let _ = write!(mnemonic, "ILLEGAL ${opcode:02X}");
            DisasmResult {
                mnemonic,
                length: 1,
            }
        }
    }
}

#[inline]
fn read_u8(bytes: &[u8], offset: usize) -> u8 {
    bytes.get(offset).copied().unwrap_or(0)
}

#[inline]
fn read_i8(bytes: &[u8], offset: usize) -> i8 {
    bytes.get(offset).copied().unwrap_or(0) as i8
}

#[inline]
fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    let lo = bytes.get(offset).copied().unwrap_or(0);
    let hi = bytes.get(offset + 1).copied().unwrap_or(0);
    u16::from_le_bytes([lo, hi])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nop() {
        let result = disasm(&[0x00], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "NOP");
        assert_eq!(result.length, 1);
    }

    #[test]
    fn test_ld_a_immediate() {
        let result = disasm(&[0x3E, 0xFF], 0x0150);
        assert_eq!(result.mnemonic.as_str(), "LD A, $FF");
        assert_eq!(result.length, 2);
    }

    #[test]
    fn test_ld_a_b() {
        let result = disasm(&[0x78], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "LD A, B");
        assert_eq!(result.length, 1);
    }

    #[test]
    fn test_ld_hl_indirect() {
        let result = disasm(&[0x36, 0x42], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "LD [HL], $42");
        assert_eq!(result.length, 2);
    }

    #[test]
    fn test_jp_absolute() {
        let result = disasm(&[0xC3, 0x50, 0x01], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "JP $0150");
        assert_eq!(result.length, 3);
    }

    #[test]
    fn test_jp_conditional() {
        let result = disasm(&[0xC2, 0x50, 0x01], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "JP NZ, $0150");
        assert_eq!(result.length, 3);
    }

    #[test]
    fn test_jr_relative() {
        let result = disasm(&[0x18, 0x10], 0x0100);
        assert_eq!(result.mnemonic.as_str(), "JR $0112");
        assert_eq!(result.length, 2);
    }

    #[test]
    fn test_jr_negative_offset() {
        let result = disasm(&[0x18, 0xFE], 0x0100);
        assert_eq!(result.mnemonic.as_str(), "JR $0100");
        assert_eq!(result.length, 2);
    }

    #[test]
    fn test_call() {
        let result = disasm(&[0xCD, 0x50, 0x01], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "CALL $0150");
        assert_eq!(result.length, 3);
    }

    #[test]
    fn test_call_conditional() {
        let result = disasm(&[0xC4, 0x50, 0x01], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "CALL NZ, $0150");
        assert_eq!(result.length, 3);
    }

    #[test]
    fn test_ret() {
        let result = disasm(&[0xC9], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RET");
        assert_eq!(result.length, 1);
    }

    #[test]
    fn test_ret_conditional() {
        let result = disasm(&[0xC0], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RET NZ");
        assert_eq!(result.length, 1);
    }

    #[test]
    fn test_push_pop() {
        let result = disasm(&[0xC5], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "PUSH BC");
        assert_eq!(result.length, 1);

        let result = disasm(&[0xC1], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "POP BC");
        assert_eq!(result.length, 1);
    }

    #[test]
    fn test_arithmetic() {
        let result = disasm(&[0x80], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "ADD A, B");

        let result = disasm(&[0x90], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "SUB A, B");

        let result = disasm(&[0xC6, 0x10], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "ADD A, $10");
    }

    #[test]
    fn test_logical() {
        let result = disasm(&[0xA0], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "AND A, B");

        let result = disasm(&[0xB0], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "OR A, B");

        let result = disasm(&[0xA8], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "XOR A, B");
    }

    #[test]
    fn test_inc_dec() {
        let result = disasm(&[0x04], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "INC B");

        let result = disasm(&[0x05], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "DEC B");

        let result = disasm(&[0x03], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "INC BC");
    }

    #[test]
    fn test_ldh() {
        let result = disasm(&[0xE0, 0x44], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "LDH [$FF44], A");

        let result = disasm(&[0xF0, 0x44], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "LDH A, [$FF44]");
    }

    #[test]
    fn test_ld_indirect_c() {
        let result = disasm(&[0xE2], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "LD [$FF00+C], A");

        let result = disasm(&[0xF2], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "LD A, [$FF00+C]");
    }

    #[test]
    fn test_cb_bit() {
        let result = disasm(&[0xCB, 0x7E], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "BIT 7, [HL]");
        assert_eq!(result.length, 2);

        let result = disasm(&[0xCB, 0x47], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "BIT 0, A");
        assert_eq!(result.length, 2);
    }

    #[test]
    fn test_cb_set_res() {
        let result = disasm(&[0xCB, 0xC7], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "SET 0, A");

        let result = disasm(&[0xCB, 0x87], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RES 0, A");
    }

    #[test]
    fn test_cb_rotates() {
        let result = disasm(&[0xCB, 0x00], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RLC B");

        let result = disasm(&[0xCB, 0x08], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RRC B");

        let result = disasm(&[0xCB, 0x10], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RL B");

        let result = disasm(&[0xCB, 0x18], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RR B");
    }

    #[test]
    fn test_cb_shifts() {
        let result = disasm(&[0xCB, 0x20], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "SLA B");

        let result = disasm(&[0xCB, 0x28], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "SRA B");

        let result = disasm(&[0xCB, 0x38], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "SRL B");
    }

    #[test]
    fn test_cb_swap() {
        let result = disasm(&[0xCB, 0x30], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "SWAP B");
    }

    #[test]
    fn test_rst() {
        let result = disasm(&[0xC7], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RST $00");

        let result = disasm(&[0xFF], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "RST $38");
    }

    #[test]
    fn test_halt() {
        let result = disasm(&[0x76], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "HALT");
    }

    #[test]
    fn test_di_ei() {
        let result = disasm(&[0xF3], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "DI");

        let result = disasm(&[0xFB], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "EI");
    }

    #[test]
    fn test_insufficient_bytes() {
        let result = disasm(&[0xCD], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "CALL $0000");
        assert_eq!(result.length, 3);
    }

    #[test]
    fn test_little_endian_16bit() {
        let result = disasm(&[0xC3, 0x50, 0x01], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "JP $0150");
        assert!(result.mnemonic.as_str().contains("0150"));
        assert!(!result.mnemonic.as_str().contains("5001"));
    }

    #[test]
    fn test_add_sp() {
        let result = disasm(&[0xE8, 0x10], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "ADD SP, 16");

        let result = disasm(&[0xE8, 0xF0], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "ADD SP, -16");
    }

    #[test]
    fn test_ld_hl_sp_offset() {
        let result = disasm(&[0xF8, 0x10], 0x0000);
        assert_eq!(result.mnemonic.as_str(), "LD HL, SP+16");
    }
}
