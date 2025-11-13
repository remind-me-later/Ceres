//! SM83 CPU disassembler with structured instruction representation
//!
//! This module provides disassembly functionality for the Game Boy's SM83 CPU,
//! which is based on a mix of 8080 and Z80 instruction sets.
//!
//! The disassembler provides a structured representation of instructions
//! that can be formatted into human-readable assembly mnemonics.
//!
//! # Example
//!
//! ```
//! use ceres_core::disasm::{disassemble, Instruction};
//!
//! let bytes = [0x3E, 0xFF]; // LD A, $FF
//! let (instruction, length) = disassemble(&bytes).expect("Failed to disassemble");
//! assert_eq!(format!("{}", instruction), "LD A, $FF");
//! assert_eq!(length, 2);
//! ```

use core::fmt;

/// An operand in an SM83 instruction
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// Absolute 16-bit address
    Absolute(u16),
    /// Condition flag C (carry)
    ConditionC,
    /// Condition flag NC (not carry)
    ConditionNc,
    /// Condition flag NZ (not zero)
    ConditionNz,
    /// Condition flag Z (zero)
    ConditionZ,
    /// 16-bit immediate value
    Immediate16(u16),
    /// 8-bit immediate value
    Immediate8(u8),
    /// Indirect register (BC)
    IndirectBc,
    /// Indirect address (C)
    IndirectC,
    /// Indirect register (DE)
    IndirectDe,
    /// Indirect high address (C)
    IndirectHighC,
    /// Indirect high address with 8-bit immediate offset
    IndirectHighImm8(u8),
    /// Indirect register (HL)
    IndirectHl,
    /// Indirect register (HL-) - decrement after access
    IndirectHlMinus,
    /// Indirect register (HL+) - increment after access
    IndirectHlPlus,
    /// 16-bit register AF
    Register16Af,
    /// 16-bit register BC
    Register16Bc,
    /// 16-bit register DE
    Register16De,
    /// 16-bit register HL
    Register16Hl,
    /// 16-bit register SP
    Register16Sp,
    /// Register A
    RegisterA,
    /// Register B
    RegisterB,
    /// Register C
    RegisterC,
    /// Register D
    RegisterD,
    /// Register E
    RegisterE,
    /// Register H
    RegisterH,
    /// Register L
    RegisterL,
    /// Relative address offset
    Relative(i8),
    /// Reset vector (0x00, 0x08, 0x10, etc.)
    RstVector(u8),
}

/// An SM83 instruction with structured operands
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    /// ADC A, r8 - Add 8-bit register and carry flag to A
    Adc8(Operand),
    /// ADC A, imm8 - Add immediate 8-bit value and carry flag to A
    Adc8Imm8(u8),
    /// ADD HL, r16 - Add 16-bit register to HL
    Add16(Operand),
    /// ADD SP, imm8 - Add signed immediate 8-bit value to SP
    Add16SpImm8(i8),
    /// ADD A, r8 - Add 8-bit register to A
    Add8(Operand),
    /// ADD A, imm8 - Add immediate 8-bit value to A
    Add8Imm8(u8),
    /// AND A, r8 - Bitwise AND A with 8-bit register
    And8(Operand),
    /// AND A, imm8 - Bitwise AND A with immediate 8-bit value
    And8Imm8(u8),
    /// BIT b, r8 - Test bit
    Bit8(u8, Operand),
    /// CALL imm16 - Call subroutine
    Call(u16),
    /// CALL cc, imm16 - Conditional call subroutine
    CallCond(Operand, u16),
    /// CCF - Complement carry flag
    Ccf,
    /// CP A, r8 - Compare A with 8-bit register
    Cp8(Operand),
    /// CP A, imm8 - Compare A with immediate 8-bit value
    Cp8Imm8(u8),
    /// CPL - Complement A
    Cpl,
    /// DAA - Decimal adjust A
    Daa,
    /// DEC r16 - Decrement 16-bit register
    Dec16(Operand),
    /// DEC r8 - Decrement 8-bit register
    Dec8(Operand),
    /// DI - Disable interrupts
    Di,
    /// EI - Enable interrupts
    Ei,
    /// HALT - Halt CPU
    Halt,
    /// INC r16 - Increment 16-bit register
    Inc16(Operand),
    /// INC r8 - Increment 8-bit register
    Inc8(Operand),
    /// JP imm16 - Jump to address
    Jp(u16),
    /// JP cc, imm16 - Conditional jump to address
    JpCond(Operand, u16),
    /// JP HL - Jump to HL address
    JpHl,
    /// JR imm8 - Relative jump
    Jr(i8),
    /// JR cc, imm8 - Conditional relative jump
    JrCond(Operand, i8),
    /// LD HL, SP+imm8 - Load `SP+signed_imm8` into HL
    Ld16HlSpImm8(i8),
    /// LD [imm16], SP - Load SP register into memory address
    Ld16Imm16Sp(u16),
    /// LD r16, imm16 - Load immediate 16-bit value into 16-bit register
    Ld16RImm16(Operand, u16),
    /// LD SP, HL - Load HL register into SP
    Ld16SpHl,
    /// LD A, [r16] - Load memory address pointed to by 16-bit register into A register
    Ld88IndirectR(Operand, Operand),
    /// LD A, [C] - Load C memory address into A register
    Ld8AC,
    /// LD A, [HL-] - Load HL memory address into A register, then decrement HL
    Ld8AHlMinus,
    /// LD A, [HL+] - Load HL memory address into A register, then increment HL
    Ld8AHlPlus,
    /// LD A, [imm16] - Load memory address into A register
    Ld8AImm16(u16),
    /// LD A, [imm8] - Load high memory address into A register
    Ld8AImm8(u8),
    /// LD [C], A - Load A register into C memory address
    Ld8CA,
    /// LD [HL-], A - Load A register into HL memory address, then decrement HL
    Ld8HlMinusA,
    /// LD [HL+], A - Load A register into HL memory address, then increment HL
    Ld8HlPlusA,
    /// LD [imm16], A - Load A register into memory address
    Ld8Imm16A(u16),
    /// LD [imm8], A - Load A register into high memory address
    Ld8Imm8A(u8),
    /// LD [r16], A - Load A register into memory address pointed to by 16-bit register
    Ld8IndirectR8(Operand, Operand),
    /// LD r8, imm8 - Load immediate 8-bit value into 8-bit register
    Ld8RImm8(Operand, u8),
    /// LD r8, r8 - Load 8-bit register into 8-bit register
    Ld8RR(Operand, Operand),
    /// NOP - No operation
    Nop,
    /// OR A, r8 - Bitwise OR A with 8-bit register
    Or8(Operand),
    /// OR A, imm8 - Bitwise OR A with immediate 8-bit value
    Or8Imm8(u8),
    /// POP r16 - Pop 16-bit register from stack
    Pop(Operand),
    /// PUSH r16 - Push 16-bit register onto stack
    Push(Operand),
    /// RES b, r8 - Reset bit
    Res8(u8, Operand),
    /// RET - Return from subroutine
    Ret,
    /// RET cc - Conditional return from subroutine
    RetCond(Operand),
    /// RETI - Return and enable interrupts
    Reti,
    /// RL r8 - Rotate left
    Rl8(Operand),
    /// RLA - Rotate A left
    Rla,
    /// RLC r8 - Rotate left with carry
    Rlc8(Operand),
    /// RLCA - Rotate A left with carry
    Rlca,
    /// RR r8 - Rotate right
    Rr8(Operand),
    /// RRA - Rotate A right
    Rra,
    /// RRC r8 - Rotate right with carry
    Rrc8(Operand),
    /// RRCA - Rotate A right with carry
    Rrca,
    /// RST imm8 - Call restart handler
    Rst(u8),
    /// SBC A, r8 - Subtract 8-bit register and carry flag from A
    Sbc8(Operand),
    /// SBC A, imm8 - Subtract immediate 8-bit value and carry flag from A
    Sbc8Imm8(u8),
    /// SCF - Set carry flag
    Scf,
    /// SET b, r8 - Set bit
    Set8(u8, Operand),
    /// SLA r8 - Shift left arithmetic
    Sla8(Operand),
    /// SRA r8 - Shift right arithmetic
    Sra8(Operand),
    /// SRL r8 - Shift right logical
    Srl8(Operand),
    /// STOP - Stop CPU
    Stop,
    /// SUB A, r8 - Subtract 8-bit register from A
    Sub8(Operand),
    /// SUB A, imm8 - Subtract immediate 8-bit value from A
    Sub8Imm8(u8),
    /// SWAP r8 - Swap nibbles
    Swap8(Operand),
    /// XOR A, r8 - Bitwise XOR A with 8-bit register
    Xor8(Operand),
    /// XOR A, imm8 - Bitwise XOR A with immediate 8-bit value
    Xor8Imm8(u8),
}

impl fmt::Display for Operand {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absolute(val) | Self::Immediate16(val) => write!(f, "${val:04X}"),
            Self::ConditionC | Self::RegisterC => write!(f, "C"),
            Self::ConditionNc => write!(f, "NC"),
            Self::ConditionNz => write!(f, "NZ"),
            Self::ConditionZ => write!(f, "Z"),
            Self::Immediate8(val) | Self::RstVector(val) => write!(f, "${val:02X}"),
            Self::IndirectC => write!(f, "[C]"),
            Self::IndirectBc => write!(f, "[BC]"),
            Self::IndirectDe => write!(f, "[DE]"),
            Self::IndirectHl => write!(f, "[HL]"),
            Self::IndirectHlMinus => write!(f, "[HL-]"),
            Self::IndirectHlPlus => write!(f, "[HL+]"),
            Self::IndirectHighC => write!(f, "[$FF00+C]"),
            Self::IndirectHighImm8(val) => write!(f, "[$FF{val:02X}]"),
            Self::Register16Af => write!(f, "AF"),
            Self::Register16Bc => write!(f, "BC"),
            Self::Register16De => write!(f, "DE"),
            Self::Register16Hl => write!(f, "HL"),
            Self::Register16Sp => write!(f, "SP"),
            Self::RegisterA => write!(f, "A"),
            Self::RegisterB => write!(f, "B"),
            Self::RegisterD => write!(f, "D"),
            Self::RegisterE => write!(f, "E"),
            Self::RegisterH => write!(f, "H"),
            Self::RegisterL => write!(f, "L"),
            Self::Relative(val) => write!(f, "{val}"),
        }
    }
}

impl fmt::Display for Instruction {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nop => write!(f, "NOP"),
            Self::Ld8RImm8(dst, src) => {
                write!(f, "LD {dst}, {}", Operand::Immediate8(*src))
            }
            Self::Ld8RR(dst, src)
            | Self::Ld8IndirectR8(dst, src)
            | Self::Ld88IndirectR(dst, src) => write!(f, "LD {dst}, {src}"),
            Self::Ld16RImm16(dst, src) => {
                write!(f, "LD {dst}, {}", Operand::Immediate16(*src))
            }
            Self::Ld8Imm16A(addr) => write!(f, "LD [${addr:04X}], A"),
            Self::Ld8AImm16(addr) => write!(f, "LD A, [${addr:04X}]"),
            Self::Ld8Imm8A(addr) => write!(f, "LDH [${addr:02X}], A"),
            Self::Ld8AImm8(addr) => write!(f, "LDH A, [${addr:02X}]"),
            Self::Ld8CA => write!(f, "LD [$FF00+C], A"),
            Self::Ld8AC => write!(f, "LD A, [$FF00+C]"),
            Self::Ld8HlPlusA => write!(f, "LD [HL+], A"),
            Self::Ld8AHlPlus => write!(f, "LD A, [HL+]"),
            Self::Ld8HlMinusA => write!(f, "LD [HL-], A"),
            Self::Ld8AHlMinus => write!(f, "LD A, [HL-]"),
            Self::Ld16HlSpImm8(offset) => write!(f, "LD HL, SP+{offset}"),
            Self::Ld16SpHl => write!(f, "LD SP, HL"),
            Self::Ld16Imm16Sp(addr) => write!(f, "LD [${addr:04X}], SP"),
            Self::Push(reg) => write!(f, "PUSH {reg}"),
            Self::Pop(reg) => write!(f, "POP {reg}"),
            Self::Add8(src) => write!(f, "ADD A, {src}"),
            Self::Add8Imm8(imm) => write!(f, "ADD A, ${imm:02X}"),
            Self::Adc8(src) => write!(f, "ADC A, {src}"),
            Self::Adc8Imm8(imm) => write!(f, "ADC A, ${imm:02X}"),
            Self::Sub8(src) => write!(f, "SUB A, {src}"),
            Self::Sub8Imm8(imm) => write!(f, "SUB A, ${imm:02X}"),
            Self::Sbc8(src) => write!(f, "SBC A, {src}"),
            Self::Sbc8Imm8(imm) => write!(f, "SBC A, ${imm:02X}"),
            Self::And8(src) => write!(f, "AND A, {src}"),
            Self::And8Imm8(imm) => write!(f, "AND A, ${imm:02X}"),
            Self::Or8(src) => write!(f, "OR A, {src}"),
            Self::Or8Imm8(imm) => write!(f, "OR A, ${imm:02X}"),
            Self::Xor8(src) => write!(f, "XOR A, {src}"),
            Self::Xor8Imm8(imm) => write!(f, "XOR A, ${imm:02X}"),
            Self::Cp8(src) => write!(f, "CP A, {src}"),
            Self::Cp8Imm8(imm) => write!(f, "CP A, ${imm:02X}"),
            Self::Inc8(operand) | Self::Inc16(operand) => write!(f, "INC {operand}"),
            Self::Dec8(operand) | Self::Dec16(operand) => write!(f, "DEC {operand}"),
            Self::Add16(src) => write!(f, "ADD HL, {src}"),
            Self::Add16SpImm8(offset) => write!(f, "ADD SP, {offset}"),
            Self::Rlca => write!(f, "RLCA"),
            Self::Rla => write!(f, "RLA"),
            Self::Rrca => write!(f, "RRCA"),
            Self::Rra => write!(f, "RRA"),
            Self::Rlc8(operand) => write!(f, "RLC {operand}"),
            Self::Rl8(operand) => write!(f, "RL {operand}"),
            Self::Rrc8(operand) => write!(f, "RRC {operand}"),
            Self::Rr8(operand) => write!(f, "RR {operand}"),
            Self::Sla8(operand) => write!(f, "SLA {operand}"),
            Self::Sra8(operand) => write!(f, "SRA {operand}"),
            Self::Swap8(operand) => write!(f, "SWAP {operand}"),
            Self::Srl8(operand) => write!(f, "SRL {operand}"),
            Self::Bit8(bit, operand) => write!(f, "BIT {bit}, {operand}"),
            Self::Res8(bit, operand) => write!(f, "RES {bit}, {operand}"),
            Self::Set8(bit, operand) => write!(f, "SET {bit}, {operand}"),
            Self::Daa => write!(f, "DAA"),
            Self::Cpl => write!(f, "CPL"),
            Self::Scf => write!(f, "SCF"),
            Self::Ccf => write!(f, "CCF"),
            Self::Jp(addr) => write!(f, "JP ${addr:04X}"),
            Self::JpCond(cond, addr) => write!(f, "JP {cond}, ${addr:04X}"),
            #[expect(clippy::cast_sign_loss)]
            Self::Jr(offset) => write!(f, "JR ${:02X}", *offset as u8), // Show the byte value directly
            #[expect(clippy::cast_sign_loss)]
            Self::JrCond(cond, offset) => write!(f, "JR {cond}, ${:02X}", *offset as u8), // Show the byte value directly
            Self::JpHl => write!(f, "JP HL"),
            Self::Call(addr) => write!(f, "CALL ${addr:04X}"),
            Self::CallCond(cond, addr) => write!(f, "CALL {cond}, ${addr:04X}"),
            Self::Ret => write!(f, "RET"),
            Self::RetCond(cond) => write!(f, "RET {cond}"),
            Self::Reti => write!(f, "RETI"),
            Self::Rst(vector) => write!(f, "RST ${vector:02X}"),
            Self::Halt => write!(f, "HALT"),
            Self::Stop => write!(f, "STOP"),
            Self::Di => write!(f, "DI"),
            Self::Ei => write!(f, "EI"),
        }
    }
}

/// Disassemble an instruction from a byte slice
///
/// # Arguments
///
/// * `bytes` - The instruction bytes to disassemble
///
/// # Panics
///
/// Panics if the input byte slice is empty.
#[must_use]
#[inline]
#[expect(clippy::missing_asserts_for_indexing, reason = "False positive")]
pub fn disassemble(bytes: &[u8]) -> Option<(Instruction, u8)> {
    assert!(!bytes.is_empty(), "Input byte slice must not be empty");
    let opcode = bytes[0];

    // CB-prefixed instructions
    if opcode == 0xCB {
        if !bytes.len() > 1 {
            return None; // Invalid instruction
        }

        assert!(
            bytes.len() > 1,
            "Input byte slice for CB instruction must have at least 2 bytes"
        );

        return Some(disassemble_cb(bytes[1]));
    }

    disassemble_base(bytes, opcode)
}

/// Disassemble a CB-prefixed instruction
const fn disassemble_cb(opcode: u8) -> (Instruction, u8) {
    let bit = (opcode >> 3) & 0x07;
    let reg = opcode & 0x07;
    let reg_operand = match reg {
        0 => Operand::RegisterB,
        1 => Operand::RegisterC,
        2 => Operand::RegisterD,
        3 => Operand::RegisterE,
        4 => Operand::RegisterH,
        5 => Operand::RegisterL,
        6 => Operand::IndirectHl,
        _ => Operand::RegisterA, // Default case (shouldn't happen)
    };
    let instruction = match opcode {
        // RLC r8
        0x00..=0x07 => Instruction::Rlc8(reg_operand),
        // RRC r8
        0x08..=0x0F => Instruction::Rrc8(reg_operand),
        // RL r8
        0x10..=0x17 => Instruction::Rl8(reg_operand),
        // RR r8
        0x18..=0x1F => Instruction::Rr8(reg_operand),
        // SLA r8
        0x20..=0x27 => Instruction::Sla8(reg_operand),
        // SRA r8
        0x28..=0x2F => Instruction::Sra8(reg_operand),
        // SWAP r8
        0x30..=0x37 => Instruction::Swap8(reg_operand),
        // SRL r8
        0x38..=0x3F => Instruction::Srl8(reg_operand),
        // BIT b, r8
        0x40..=0x7F => Instruction::Bit8(bit, reg_operand),
        // RES b, r8
        0x80..=0xBF => Instruction::Res8(bit, reg_operand),
        // SET b, r8
        0xC0..=0xFF => Instruction::Set8(bit, reg_operand),
    };

    (instruction, 2)
}

/// Get the register name for a given register index
const fn get_reg_name(reg: u8) -> Operand {
    match reg {
        0 => Operand::RegisterB,
        1 => Operand::RegisterC,
        2 => Operand::RegisterD,
        3 => Operand::RegisterE,
        4 => Operand::RegisterH,
        5 => Operand::RegisterL,
        6 => Operand::IndirectHl,
        _ => Operand::RegisterA, // Default case (shouldn't happen)
    }
}

/// Get the 16-bit register name for a given index
const fn get_r16_name(index: u8) -> Operand {
    match index {
        1 => Operand::Register16De,
        2 => Operand::Register16Hl,
        3 => Operand::Register16Sp,
        _ => Operand::Register16Bc, // Default case (shouldn't happen)
    }
}

/// Get the 16-bit register name for stack operations
const fn get_r16_stk_name(index: u8) -> Operand {
    match index {
        1 => Operand::Register16De,
        2 => Operand::Register16Hl,
        3 => Operand::Register16Af,
        _ => Operand::Register16Bc, // Default case (shouldn't happen)
    }
}

/// Get the condition name for a given condition index
const fn get_cond_name(cond: u8) -> Operand {
    match cond {
        1 => Operand::ConditionZ,
        2 => Operand::ConditionNc,
        3 => Operand::ConditionC,
        _ => Operand::ConditionNz, // Default case (shouldn't happen)
    }
}

/// Read an 8-bit value from bytes slice at offset
fn read_u8(bytes: &[u8], offset: usize) -> u8 {
    bytes.get(offset).copied().unwrap_or(0)
}

/// Read an 8-bit signed value from bytes slice at offset
#[expect(clippy::cast_possible_wrap)]
fn read_i8(bytes: &[u8], offset: usize) -> i8 {
    bytes.get(offset).copied().unwrap_or(0) as i8
}

/// Read a 16-bit value from bytes slice at offset (little endian)
fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    let lo = bytes.get(offset).copied().unwrap_or(0);
    let hi = bytes.get(offset + 1).copied().unwrap_or(0);
    u16::from_le_bytes([lo, hi])
}

/// Disassemble a base instruction (non-CB-prefixed)
#[expect(clippy::too_many_lines)]
fn disassemble_base(bytes: &[u8], opcode: u8) -> Option<(Instruction, u8)> {
    let y = (opcode >> 3) & 0x07;
    let z = opcode & 0x07;
    let p = (opcode >> 4) & 0x03;

    match opcode {
        0x00 => Some((Instruction::Nop, 1)),
        0x08 => {
            let addr = read_u16(bytes, 1);
            Some((Instruction::Ld16Imm16Sp(addr), 3))
        }
        0x10 => Some((Instruction::Stop, 2)),
        0x18 => {
            let offset = read_i8(bytes, 1);
            Some((Instruction::Jr(offset), 2))
        }
        0x20 | 0x28 | 0x30 | 0x38 => {
            let cond = get_cond_name((opcode >> 3) & 0x03);
            let offset = read_i8(bytes, 1);
            Some((Instruction::JrCond(cond, offset), 2))
        }
        0x01 | 0x11 | 0x21 | 0x31 => {
            let r16 = get_r16_name(p);
            let imm = read_u16(bytes, 1);
            Some((Instruction::Ld16RImm16(r16, imm), 3))
        }
        0x09 | 0x19 | 0x29 | 0x39 => {
            let r16 = get_r16_name(p);
            Some((Instruction::Add16(r16), 1))
        }
        0x02 => Some((
            Instruction::Ld8IndirectR8(Operand::IndirectBc, Operand::RegisterA),
            1,
        )),
        0x12 => Some((
            Instruction::Ld8IndirectR8(Operand::IndirectDe, Operand::RegisterA),
            1,
        )),
        0x22 => Some((Instruction::Ld8HlPlusA, 1)),
        0x32 => Some((Instruction::Ld8HlMinusA, 1)),
        0x0A => Some((
            Instruction::Ld88IndirectR(Operand::RegisterA, Operand::IndirectBc),
            1,
        )),
        0x1A => Some((
            Instruction::Ld88IndirectR(Operand::RegisterA, Operand::IndirectDe),
            1,
        )),
        0x2A => Some((Instruction::Ld8AHlPlus, 1)),
        0x3A => Some((Instruction::Ld8AHlMinus, 1)),
        0x03 | 0x13 | 0x23 | 0x33 => {
            let r16 = get_r16_name(p);
            Some((Instruction::Inc16(r16), 1))
        }
        0x0B | 0x1B | 0x2B | 0x3B => {
            let r16 = get_r16_name(p);
            Some((Instruction::Dec16(r16), 1))
        }
        0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
            let reg = get_reg_name(y);
            Some((Instruction::Inc8(reg), 1))
        }
        0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
            let reg = get_reg_name(y);
            Some((Instruction::Dec8(reg), 1))
        }
        0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
            let reg = get_reg_name(y);
            let imm = read_u8(bytes, 1);
            Some((Instruction::Ld8RImm8(reg, imm), 2))
        }
        0x07 => Some((Instruction::Rlca, 1)),
        0x0F => Some((Instruction::Rrca, 1)),
        0x17 => Some((Instruction::Rla, 1)),
        0x1F => Some((Instruction::Rra, 1)),
        0x27 => Some((Instruction::Daa, 1)),
        0x2F => Some((Instruction::Cpl, 1)),
        0x37 => Some((Instruction::Scf, 1)),
        0x3F => Some((Instruction::Ccf, 1)),
        0x76 => Some((Instruction::Halt, 1)),
        0x40..=0x75 | 0x77..=0x7F => {
            let dst = get_reg_name(y);
            let src = get_reg_name(z);
            Some((Instruction::Ld8RR(dst, src), 1))
        }
        0x80..=0x87 => {
            let reg = get_reg_name(z);
            Some((Instruction::Add8(reg), 1))
        }
        0x88..=0x8F => {
            let reg = get_reg_name(z);
            Some((Instruction::Adc8(reg), 1))
        }
        0x90..=0x97 => {
            let reg = get_reg_name(z);
            Some((Instruction::Sub8(reg), 1))
        }
        0x98..=0x9F => {
            let reg = get_reg_name(z);
            Some((Instruction::Sbc8(reg), 1))
        }
        0xA0..=0xA7 => {
            let reg = get_reg_name(z);
            Some((Instruction::And8(reg), 1))
        }
        0xA8..=0xAF => {
            let reg = get_reg_name(z);
            Some((Instruction::Xor8(reg), 1))
        }
        0xB0..=0xB7 => {
            let reg = get_reg_name(z);
            Some((Instruction::Or8(reg), 1))
        }
        0xB8..=0xBF => {
            let reg = get_reg_name(z);
            Some((Instruction::Cp8(reg), 1))
        }
        0xC0 | 0xC8 | 0xD0 | 0xD8 => {
            let cond = get_cond_name((opcode >> 3) & 0x03);
            Some((Instruction::RetCond(cond), 1))
        }
        0xC1 | 0xD1 | 0xE1 | 0xF1 => {
            let r16 = get_r16_stk_name(p);
            Some((Instruction::Pop(r16), 1))
        }
        0xC2 | 0xCA | 0xD2 | 0xDA => {
            let cond = get_cond_name((opcode >> 3) & 0x03);
            let addr = read_u16(bytes, 1);
            Some((Instruction::JpCond(cond, addr), 3))
        }
        0xC3 => {
            let addr = read_u16(bytes, 1);
            Some((Instruction::Jp(addr), 3))
        }
        0xC4 | 0xCC | 0xD4 | 0xDC => {
            let cond = get_cond_name((opcode >> 3) & 0x03);
            let addr = read_u16(bytes, 1);
            Some((Instruction::CallCond(cond, addr), 3))
        }
        0xC5 | 0xD5 | 0xE5 | 0xF5 => {
            let r16 = get_r16_stk_name(p);
            Some((Instruction::Push(r16), 1))
        }
        0xC6 => {
            let imm = read_u8(bytes, 1);
            Some((Instruction::Add8Imm8(imm), 2))
        }
        0xCE => {
            let imm = read_u8(bytes, 1);
            Some((Instruction::Adc8Imm8(imm), 2))
        }
        0xD6 => {
            let imm = read_u8(bytes, 1);
            Some((Instruction::Sub8Imm8(imm), 2))
        }
        0xDE => {
            let imm = read_u8(bytes, 1);
            Some((Instruction::Sbc8Imm8(imm), 2))
        }
        0xE6 => {
            let imm = read_u8(bytes, 1);
            Some((Instruction::And8Imm8(imm), 2))
        }
        0xEE => {
            let imm = read_u8(bytes, 1);
            Some((Instruction::Xor8Imm8(imm), 2))
        }
        0xF6 => {
            let imm = read_u8(bytes, 1);
            Some((Instruction::Or8Imm8(imm), 2))
        }
        0xFE => {
            let imm = read_u8(bytes, 1);
            Some((Instruction::Cp8Imm8(imm), 2))
        }
        0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
            let addr = opcode & 0x38;
            Some((Instruction::Rst(addr), 1))
        }
        0xC9 => Some((Instruction::Ret, 1)),
        0xD9 => Some((Instruction::Reti, 1)),
        0xE0 => {
            let offset = read_u8(bytes, 1);
            Some((Instruction::Ld8Imm8A(offset), 2))
        }
        0xE2 => Some((Instruction::Ld8CA, 1)),
        0xE8 => {
            let offset = read_i8(bytes, 1);
            Some((Instruction::Add16SpImm8(offset), 2))
        }
        0xE9 => Some((Instruction::JpHl, 1)),
        0xEA => {
            let addr = read_u16(bytes, 1);
            Some((Instruction::Ld8Imm16A(addr), 3))
        }
        0xF0 => {
            let offset = read_u8(bytes, 1);
            Some((Instruction::Ld8AImm8(offset), 2))
        }
        0xF2 => Some((Instruction::Ld8AC, 1)),
        0xF3 => Some((Instruction::Di, 1)),
        0xF8 => {
            let offset = read_i8(bytes, 1);
            Some((Instruction::Ld16HlSpImm8(offset), 2))
        }
        0xF9 => Some((Instruction::Ld16SpHl, 1)),
        0xFA => {
            let addr = read_u16(bytes, 1);
            Some((Instruction::Ld8AImm16(addr), 3))
        }
        0xFB => Some((Instruction::Ei, 1)),
        0xCD => {
            let addr = read_u16(bytes, 1);
            Some((Instruction::Call(addr), 3))
        }
        _ => None, // Invalid opcode, return None
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Test code")]
#[expect(clippy::shadow_unrelated, reason = "Test code")]
mod tests {
    use super::*;

    #[test]
    fn test_nop() {
        let (instr, len) = disassemble(&[0x00]).unwrap();
        assert_eq!(instr, Instruction::Nop);
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "NOP");
    }

    #[test]
    fn test_ld_a_immediate() {
        let (instr, len) = disassemble(&[0x3E, 0xFF]).unwrap();
        assert_eq!(instr, Instruction::Ld8RImm8(Operand::RegisterA, 0xFF));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "LD A, $FF");
    }

    #[test]
    fn test_ld_a_b() {
        let (instr, len) = disassemble(&[0x78]).unwrap();
        assert_eq!(
            instr,
            Instruction::Ld8RR(Operand::RegisterA, Operand::RegisterB)
        );
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "LD A, B");
    }

    #[test]
    fn test_ld_hl_indirect() {
        let (instr, len) = disassemble(&[0x36, 0x42]).unwrap();
        assert_eq!(instr, Instruction::Ld8RImm8(Operand::IndirectHl, 0x42));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "LD [HL], $42");
    }

    #[test]
    fn test_jp_absolute() {
        let (instr, len) = disassemble(&[0xC3, 0x50, 0x01]).unwrap();
        assert_eq!(instr, Instruction::Jp(0x0150));
        assert_eq!(len, 3);
        assert_eq!(instr.to_string(), "JP $0150");
    }

    #[test]
    fn test_jp_conditional() {
        let (instr, len) = disassemble(&[0xC2, 0x50, 0x01]).unwrap();
        assert_eq!(instr, Instruction::JpCond(Operand::ConditionNz, 0x0150));
        assert_eq!(len, 3);
        assert_eq!(instr.to_string(), "JP NZ, $0150");
    }

    #[test]
    fn test_jr_relative() {
        let (instr, len) = disassemble(&[0x18, 0x10]).unwrap();
        assert_eq!(instr, Instruction::Jr(0x10));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "JR $10");
    }

    #[test]
    fn test_call() {
        let (instr, len) = disassemble(&[0xCD, 0x50, 0x01]).unwrap();
        assert_eq!(instr, Instruction::Call(0x0150));
        assert_eq!(len, 3);
        assert_eq!(instr.to_string(), "CALL $0150");
    }

    #[test]
    fn test_call_conditional() {
        let (instr, len) = disassemble(&[0xC4, 0x50, 0x01]).unwrap();
        assert_eq!(instr, Instruction::CallCond(Operand::ConditionNz, 0x0150));
        assert_eq!(len, 3);
        assert_eq!(instr.to_string(), "CALL NZ, $0150");
    }

    #[test]
    fn test_ret() {
        let (instr, len) = disassemble(&[0xC9]).unwrap();
        assert_eq!(instr, Instruction::Ret);
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "RET");
    }

    #[test]
    fn test_ret_conditional() {
        let (instr, len) = disassemble(&[0xC0]).unwrap();
        assert_eq!(instr, Instruction::RetCond(Operand::ConditionNz));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "RET NZ");
    }

    #[test]
    fn test_push_pop() {
        let (instr, len) = disassemble(&[0xC5]).unwrap();
        assert_eq!(instr, Instruction::Push(Operand::Register16Bc));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "PUSH BC");

        let (instr, len) = disassemble(&[0xC1]).unwrap();
        assert_eq!(instr, Instruction::Pop(Operand::Register16Bc));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "POP BC");
    }

    #[test]
    fn test_arithmetic() {
        let (instr, len) = disassemble(&[0x80]).unwrap();
        assert_eq!(instr, Instruction::Add8(Operand::RegisterB));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "ADD A, B");

        let (instr, len) = disassemble(&[0x90]).unwrap();
        assert_eq!(instr, Instruction::Sub8(Operand::RegisterB));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "SUB A, B");

        let (instr, len) = disassemble(&[0xC6, 0x10]).unwrap();
        assert_eq!(instr, Instruction::Add8Imm8(0x10));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "ADD A, $10");
    }

    #[test]
    fn test_logical() {
        let (instr, len) = disassemble(&[0xA0]).unwrap();
        assert_eq!(instr, Instruction::And8(Operand::RegisterB));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "AND A, B");

        let (instr, len) = disassemble(&[0xB0]).unwrap();
        assert_eq!(instr, Instruction::Or8(Operand::RegisterB));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "OR A, B");

        let (instr, len) = disassemble(&[0xA8]).unwrap();
        assert_eq!(instr, Instruction::Xor8(Operand::RegisterB));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "XOR A, B");
    }

    #[test]
    fn test_inc_dec() {
        let (instr, len) = disassemble(&[0x04]).unwrap();
        assert_eq!(instr, Instruction::Inc8(Operand::RegisterB));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "INC B");

        let (instr, len) = disassemble(&[0x05]).unwrap();
        assert_eq!(instr, Instruction::Dec8(Operand::RegisterB));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "DEC B");

        let (instr, len) = disassemble(&[0x03]).unwrap();
        assert_eq!(instr, Instruction::Inc16(Operand::Register16Bc));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "INC BC");
    }

    #[test]
    fn test_ldh() {
        let (instr, len) = disassemble(&[0xE0, 0x44]).unwrap();
        assert_eq!(instr, Instruction::Ld8Imm8A(0x44));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "LDH [$44], A");

        let (instr, len) = disassemble(&[0xF0, 0x44]).unwrap();
        assert_eq!(instr, Instruction::Ld8AImm8(0x44));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "LDH A, [$44]");
    }

    #[test]
    fn test_cb_bit() {
        let (instr, len) = disassemble(&[0xCB, 0x7E]).unwrap();
        assert_eq!(instr, Instruction::Bit8(7, Operand::IndirectHl));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "BIT 7, [HL]");

        let (instr, len) = disassemble(&[0xCB, 0x47]).unwrap();
        assert_eq!(instr, Instruction::Bit8(0, Operand::RegisterA));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "BIT 0, A");
    }

    #[test]
    fn test_cb_set_res() {
        let (instr, len) = disassemble(&[0xCB, 0xC7]).unwrap();
        assert_eq!(instr, Instruction::Set8(0, Operand::RegisterA));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "SET 0, A");

        let (instr, len) = disassemble(&[0xCB, 0x87]).unwrap();
        assert_eq!(instr, Instruction::Res8(0, Operand::RegisterA));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "RES 0, A");
    }

    #[test]
    fn test_cb_rotates() {
        let (instr, len) = disassemble(&[0xCB, 0x00]).unwrap();
        assert_eq!(instr, Instruction::Rlc8(Operand::RegisterB));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "RLC B");

        let (instr, len) = disassemble(&[0xCB, 0x08]).unwrap();
        assert_eq!(instr, Instruction::Rrc8(Operand::RegisterB));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "RRC B");

        let (instr, len) = disassemble(&[0xCB, 0x10]).unwrap();
        assert_eq!(instr, Instruction::Rl8(Operand::RegisterB));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "RL B");

        let (instr, len) = disassemble(&[0xCB, 0x18]).unwrap();
        assert_eq!(instr, Instruction::Rr8(Operand::RegisterB));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "RR B");
    }

    #[test]
    fn test_rst() {
        let (instr, len) = disassemble(&[0xC7]).unwrap();
        assert_eq!(instr, Instruction::Rst(0x00));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "RST $00");

        let (instr, len) = disassemble(&[0xFF]).unwrap();
        assert_eq!(instr, Instruction::Rst(0x38));
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "RST $38");
    }

    #[test]
    fn test_halt() {
        let (instr, len) = disassemble(&[0x76]).unwrap();
        assert_eq!(instr, Instruction::Halt);
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "HALT");
    }

    #[test]
    fn test_di_ei() {
        let (instr, len) = disassemble(&[0xF3]).unwrap();
        assert_eq!(instr, Instruction::Di);
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "DI");

        let (instr, len) = disassemble(&[0xFB]).unwrap();
        assert_eq!(instr, Instruction::Ei);
        assert_eq!(len, 1);
        assert_eq!(instr.to_string(), "EI");
    }

    #[test]
    fn test_add_sp() {
        let (instr, len) = disassemble(&[0xE8, 0x10]).unwrap();
        assert_eq!(instr, Instruction::Add16SpImm8(0x10));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "ADD SP, 16");

        let (instr, len) = disassemble(&[0xE8, 0xF0]).unwrap();
        assert_eq!(instr, Instruction::Add16SpImm8(-16));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "ADD SP, -16");
    }

    #[test]
    fn test_ld_hl_sp_offset() {
        let (instr, len) = disassemble(&[0xF8, 0x10]).unwrap();
        assert_eq!(instr, Instruction::Ld16HlSpImm8(0x10));
        assert_eq!(len, 2);
        assert_eq!(instr.to_string(), "LD HL, SP+16");
    }
}
