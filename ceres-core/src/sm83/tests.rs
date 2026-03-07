use super::*;
use crate::test_util::setup_gb;

// Helper type alias
type Gb = crate::Gb<crate::test_util::DummyAudio>;

fn test_op_timing(gb: &mut Gb, opcode: u8, operands: &[u8], expected_m_cycles: u64) {
    let addr = 0xC000;
    gb.cpu.pc = addr;
    gb.write_mem(addr, opcode);
    for (i, &op) in operands.iter().enumerate() {
        gb.write_mem(addr + 1 + i as u16, op);
    }

    let start_dots = gb.total_dots;
    gb.run_cpu();
    let end_dots = gb.total_dots;
    let elapsed_dots = end_dots - start_dots;
    assert_eq!(
        elapsed_dots,
        expected_m_cycles * 4,
        "Opcode 0x{:02X} took {} dots, expected {}",
        opcode,
        elapsed_dots,
        expected_m_cycles * 4
    );
}

fn test_cb_timing(gb: &mut Gb, cb_opcode: u8, expected_m_cycles: u64) {
    let addr = 0xC000;
    gb.cpu.pc = addr;
    gb.write_mem(addr, 0xCB);
    gb.write_mem(addr + 1, cb_opcode);

    let start_dots = gb.total_dots;
    gb.run_cpu();
    let end_dots = gb.total_dots;
    let elapsed_dots = end_dots - start_dots;
    assert_eq!(
        elapsed_dots,
        expected_m_cycles * 4,
        "CB Opcode 0x{:02X} took {} dots, expected {}",
        cb_opcode,
        elapsed_dots,
        expected_m_cycles * 4
    );
}

#[test]
fn test_add() {
    let mut gb = setup_gb();
    // 0x12 + 0x34 = 0x46
    gb.cpu.af = 0x1200;
    gb.add(0x34);
    assert_eq!(gb.cpu.a(), 0x46);
    assert_eq!(gb.cpu.f(), 0);

    // 0xFF + 0x01 = 0x00 (Carry, Half-Carry, Zero)
    gb.cpu.af = 0xFF00;
    gb.add(0x01);
    assert_eq!(gb.cpu.a(), 0x00);
    assert_eq!(gb.cpu.f(), (ZF | HF | CF) as u8);

    // 0x0F + 0x01 = 0x10 (Half-Carry)
    gb.cpu.af = 0x0F00;
    gb.add(0x01);
    assert_eq!(gb.cpu.a(), 0x10);
    assert_eq!(gb.cpu.f(), HF as u8);
}

#[test]
fn test_adc() {
    let mut gb = setup_gb();
    // 0x12 + 0x34 + carry(0) = 0x46
    gb.cpu.af = 0x1200;
    gb.adc(0x34);
    assert_eq!(gb.cpu.a(), 0x46);
    assert_eq!(gb.cpu.f(), 0);

    // 0x12 + 0x34 + carry(1) = 0x47
    gb.cpu.af = 0x1200 | CF;
    gb.adc(0x34);
    assert_eq!(gb.cpu.a(), 0x47);
    assert_eq!(gb.cpu.f(), 0);

    // 0x0F + 0x00 + carry(1) = 0x10 (Half-Carry)
    gb.cpu.af = 0x0F00 | CF;
    gb.adc(0x00);
    assert_eq!(gb.cpu.a(), 0x10);
    assert_eq!(gb.cpu.f(), HF as u8);
}

#[test]
fn test_sub() {
    let mut gb = setup_gb();
    // 0x34 - 0x12 = 0x22
    gb.cpu.af = 0x3400;
    gb.sub(0x12);
    assert_eq!(gb.cpu.a(), 0x22);
    assert_eq!(gb.cpu.f(), NF as u8);

    // 0x00 - 0x01 = 0xFF (Carry, Half-Carry)
    gb.cpu.af = 0x0000;
    gb.sub(0x01);
    assert_eq!(gb.cpu.a(), 0xFF);
    assert_eq!(gb.cpu.f(), (NF | HF | CF) as u8);

    // 0x10 - 0x01 = 0x0F (Half-Carry)
    gb.cpu.af = 0x1000;
    gb.sub(0x01);
    assert_eq!(gb.cpu.a(), 0x0F);
    assert_eq!(gb.cpu.f(), (NF | HF) as u8);
}

#[test]
fn test_sbc() {
    let mut gb = setup_gb();
    // 0x34 - 0x12 - carry(0) = 0x22
    gb.cpu.af = 0x3400;
    gb.sbc(0x12);
    assert_eq!(gb.cpu.a(), 0x22);
    assert_eq!(gb.cpu.f(), NF as u8);

    // 0x34 - 0x12 - carry(1) = 0x21
    gb.cpu.af = 0x3400 | CF;
    gb.sbc(0x12);
    assert_eq!(gb.cpu.a(), 0x21);
    assert_eq!(gb.cpu.f(), NF as u8);

    // 0x10 - 0x00 - carry(1) = 0x0F (Half-Carry)
    gb.cpu.af = 0x1000 | CF;
    gb.sbc(0x00);
    assert_eq!(gb.cpu.a(), 0x0F);
    assert_eq!(gb.cpu.f(), (NF | HF) as u8);
}

#[test]
fn test_logical() {
    let mut gb = setup_gb();
    // AND
    gb.cpu.af = 0xFF00;
    gb.and(0x0F);
    assert_eq!(gb.cpu.a(), 0x0F);
    assert_eq!(gb.cpu.f(), HF as u8);

    // OR
    gb.cpu.af = 0xF000;
    gb.or(0x0F);
    assert_eq!(gb.cpu.a(), 0xFF);
    assert_eq!(gb.cpu.f(), 0);

    // XOR
    gb.cpu.af = 0xAA00;
    gb.xor(0xFF);
    assert_eq!(gb.cpu.a(), 0x55);
    assert_eq!(gb.cpu.f(), 0);
}

#[test]
fn test_daa() {
    let mut gb = setup_gb();
    // 0x45 + 0x38 = 0x7D -> DAA -> 0x83
    gb.cpu.af = 0x4500;
    gb.add(0x38);
    gb.daa();
    assert_eq!(gb.cpu.a(), 0x83);

    // 0x83 - 0x38 = 0x4B -> DAA -> 0x45
    gb.cpu.af = 0x8300;
    gb.sub(0x38);
    gb.daa();
    assert_eq!(gb.cpu.a(), 0x45);
}

#[test]
fn test_inc_dec() {
    let mut gb = setup_gb();
    // INC B (0x04)
    gb.cpu.bc = 0x0000;
    gb.inc_hr(0x04);
    assert_eq!(gb.cpu.bc >> 8, 1);
    assert_eq!(gb.cpu.f() & (NF as u8), 0);

    // DEC B (0x05)
    gb.dec_hr(0x05);
    assert_eq!(gb.cpu.bc >> 8, 0);
    assert_eq!(gb.cpu.f() & (ZF as u8), ZF as u8);
    assert_eq!(gb.cpu.f() & (NF as u8), NF as u8);

    // INC BC (0x03) - 16-bit, no flags
    gb.cpu.af = 0;
    gb.cpu.bc = 0xFFFF;
    gb.inc_rr(0x03);
    assert_eq!(gb.cpu.bc, 0x0000);
    assert_eq!(gb.cpu.f(), 0);
}

#[test]
fn test_rotates() {
    let mut gb = setup_gb();
    // RLCA
    gb.cpu.af = 0x8000;
    gb.rlca();
    assert_eq!(gb.cpu.a(), 0x01);
    assert_eq!(gb.cpu.f(), CF as u8);

    // RRCA
    gb.cpu.af = 0x0100;
    gb.rrca();
    assert_eq!(gb.cpu.a(), 0x80);
    assert_eq!(gb.cpu.f(), CF as u8);
}

#[test]
fn test_shifts() {
    let mut gb = setup_gb();
    // SLA A (0x27)
    gb.cpu.af = 0x8000;
    gb.sla_r(0x27);
    assert_eq!(gb.cpu.a(), 0x00);
    assert_eq!(gb.cpu.f(), (ZF | CF) as u8);

    // SRA A (0x2F)
    gb.cpu.af = 0x8100;
    gb.sra_r(0x2F);
    assert_eq!(gb.cpu.a(), 0xC0);
    assert_eq!(gb.cpu.f(), CF as u8);

    // SRL A (0x3F)
    gb.cpu.af = 0x8100;
    gb.srl_r(0x3F);
    assert_eq!(gb.cpu.a(), 0x40);
    assert_eq!(gb.cpu.f(), CF as u8);

    // SWAP A (0x37)
    gb.cpu.af = 0x1200;
    gb.swap_r(0x37);
    assert_eq!(gb.cpu.a(), 0x21);
    assert_eq!(gb.cpu.f(), 0);
}

#[test]
fn test_daa_edge_cases() {
    let mut gb = setup_gb();

    // Addition: 0x99 + 0x01 = 0x9A -> DAA -> 0x00 (Carry)
    gb.cpu.af = 0x9900;
    gb.add(0x01);
    gb.daa();
    assert_eq!(gb.cpu.a(), 0x00);
    assert_eq!(gb.cpu.f() & (CF as u8), CF as u8);
    assert_eq!(gb.cpu.f() & (ZF as u8), ZF as u8);

    // Subtraction: 0x00 - 0x01 = 0xFF (C, H, N) -> DAA -> 0x99 (Carry)
    gb.cpu.af = 0x0000;
    gb.sub(0x01);
    gb.daa();
    assert_eq!(gb.cpu.a(), 0x99);
    assert_eq!(gb.cpu.f() & (CF as u8), CF as u8);
}

#[test]
fn test_add_hl() {
    let mut gb = setup_gb();
    // 0x1234 + 0x1111 = 0x2345
    gb.cpu.af = NF | CF | HF; // These should be cleared
    gb.cpu.hl = 0x1234;
    gb.cpu.bc = 0x1111;
    gb.add_hl_rr(0x09); // ADD HL, BC (opcode 0x09)
    assert_eq!(gb.cpu.hl, 0x2345);
    assert_eq!(gb.cpu.f() & (NF | CF | HF) as u8, 0);

    // 0x0FFF + 0x0001 = 0x1000 (Half-Carry)
    gb.cpu.hl = 0x0FFF;
    gb.cpu.bc = 0x0001;
    gb.add_hl_rr(0x09);
    assert_eq!(gb.cpu.hl, 0x1000);
    assert_eq!(gb.cpu.f() & (HF as u8), HF as u8);

    // 0xFFFF + 0x0001 = 0x0000 (Carry, Half-Carry)
    gb.cpu.hl = 0xFFFF;
    gb.cpu.bc = 0x0001;
    gb.add_hl_rr(0x09);
    assert_eq!(gb.cpu.hl, 0x0000);
    assert_eq!(gb.cpu.f() & (CF | HF) as u8, (CF | HF) as u8);
}

#[test]
fn test_bit_ops() {
    let mut gb = setup_gb();
    // SET 7, A (0xFF) - using bit_r internal logic
    gb.cpu.af = 0x0000;
    gb.bit_r(0xFF); // SET 7, A
    assert_eq!(gb.cpu.a(), 0x80);

    // BIT 7, A (0x7F)
    gb.bit_r(0x7F);
    assert_eq!(gb.cpu.f() & (ZF as u8), 0);
    assert_eq!(gb.cpu.f() & (HF as u8), HF as u8);

    // BIT 6, A (0x77)
    gb.bit_r(0x40 | (6 << 3) | 7);
    assert_eq!(gb.cpu.f() & (ZF as u8), ZF as u8);

    // RES 7, A (0xBF)
    gb.bit_r(0xBF);
    assert_eq!(gb.cpu.a(), 0x00);
}

#[test]
fn test_timing() {
    let mut gb = setup_gb();
    gb.cpu.sp = 0xD000; // Point stack to WRAM

    // Basic
    test_op_timing(&mut gb, 0x00, &[], 1); // NOP
    test_op_timing(&mut gb, 0x7F, &[], 1); // LD A, A
    test_op_timing(&mut gb, 0x06, &[0x42], 2); // LD B, d8
    test_op_timing(&mut gb, 0x46, &[], 2); // LD B, (HL)
    test_op_timing(&mut gb, 0x70, &[], 2); // LD (HL), B
    test_op_timing(&mut gb, 0x36, &[0x42], 3); // LD (HL), d8
    test_op_timing(&mut gb, 0x01, &[0x34, 0x12], 3); // LD BC, d16
    test_op_timing(&mut gb, 0x08, &[0x34, 0x12], 5); // LD (a16), SP

    // Arithmetic
    test_op_timing(&mut gb, 0x04, &[], 1); // INC B
    test_op_timing(&mut gb, 0x03, &[], 2); // INC BC
    test_op_timing(&mut gb, 0x34, &[], 3); // INC (HL)
    test_op_timing(&mut gb, 0x09, &[], 2); // ADD HL, BC
    test_op_timing(&mut gb, 0x80, &[], 1); // ADD A, B
    test_op_timing(&mut gb, 0xC6, &[0x42], 2); // ADD A, d8
    test_op_timing(&mut gb, 0x86, &[], 2); // ADD A, (HL)

    // Control flow
    test_op_timing(&mut gb, 0x18, &[0x02], 3); // JR e
    test_op_timing(&mut gb, 0xC3, &[0x00, 0xC1], 4); // JP a16
    test_op_timing(&mut gb, 0xE9, &[], 1); // JP (HL)

    // Stack
    test_op_timing(&mut gb, 0xC5, &[], 4); // PUSH BC
    test_op_timing(&mut gb, 0xC1, &[], 3); // POP BC

    // CB
    test_cb_timing(&mut gb, 0x00, 2); // RLC B
    test_cb_timing(&mut gb, 0x06, 4); // RLC (HL)
    test_cb_timing(&mut gb, 0x40, 2); // BIT 0, B
    test_cb_timing(&mut gb, 0x46, 3); // BIT 0, (HL)
}

#[test]
fn test_timing_complex() {
    let mut gb = setup_gb();
    gb.cpu.sp = 0xD000;

    // PUSH BC (0xC5) -> 4
    test_op_timing(&mut gb, 0xC5, &[], 4);
    assert_eq!(gb.cpu.sp, 0xCFFE); // SP - 2

    // POP BC (0xC1) -> 3
    test_op_timing(&mut gb, 0xC1, &[], 3);
    assert_eq!(gb.cpu.sp, 0xD000);

    // CALL a16 (0xCD) -> 6
    test_op_timing(&mut gb, 0xCD, &[0x00, 0xE0], 6);
    assert_eq!(gb.cpu.pc, 0xE000);
    assert_eq!(gb.cpu.sp, 0xCFFE);

    // RET (0xC9) -> 4
    // Note: CALL pushed 0xC003 (C000 + 3 bytes of instruction)
    test_op_timing(&mut gb, 0xC9, &[], 4);
    assert_eq!(gb.cpu.pc, 0xC003);
    assert_eq!(gb.cpu.sp, 0xD000);

    // RST 0x00 (0xC7) -> 4
    test_op_timing(&mut gb, 0xC7, &[], 4);
    assert_eq!(gb.cpu.pc, 0x0000);
}

#[test]
fn test_timing_conditional() {
    let mut gb = setup_gb();

    // JR NZ, r8 (0x20)
    // Not taken (ZF=1)
    gb.cpu.af = ZF;
    gb.cpu.pc = 0xC000;
    gb.write_mem(0xC000, 0x20);
    gb.write_mem(0xC001, 0x05);
    let start = gb.total_dots;
    gb.run_cpu();
    assert_eq!(gb.total_dots - start, 2 * 4);
    assert_eq!(gb.cpu.pc, 0xC002);

    // Taken (ZF=0)
    gb.cpu.af = 0;
    gb.cpu.pc = 0xC000;
    gb.write_mem(0xC000, 0x20);
    gb.write_mem(0xC001, 0x05);
    let start = gb.total_dots;
    gb.run_cpu();
    assert_eq!(gb.total_dots - start, 3 * 4);
    assert_eq!(gb.cpu.pc, 0xC007);
}

// ============================================================================
// Gambatte-derived CPU tests
// ============================================================================

/// Helper: write a sequence of bytes to WRAM starting at `addr`.
fn write_code(gb: &mut Gb, addr: u16, bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        gb.write_mem(addr.wrapping_add(i as u16), b);
    }
}

// ----------------------------------------------------------------------------
// Undefined opcode tests
//
// Source: gambatte/test/hwtests/undef_ops/undef_op_XX_dmg08_cgb04c_out01.asm
//
// Behavior: Executing an undefined opcode locks the CPU (HALT-like infinite
// loop). The `illegal()` handler sets `cpu.is_halted = true` and calls
// `ints.illegal()` which zeroes IE. The CPU never advances past the opcode.
// ----------------------------------------------------------------------------

/// Helper: execute an undefined opcode and verify the CPU is locked up.
///
/// Writes the undefined opcode at 0xC000, runs one CPU step, and asserts
/// that `is_halted` is set and PC has not advanced beyond the opcode byte.
fn assert_undef_op_locks_cpu(opcode: u8) {
    let mut gb = setup_gb();
    let addr: u16 = 0xC000;
    gb.cpu.pc = addr;
    gb.write_mem(addr, opcode);
    // Sentinel: put a distinguishable byte after the opcode
    gb.write_mem(addr + 1, 0xAB);

    gb.run_cpu();

    assert!(
        gb.cpu.is_halted,
        "Undefined opcode 0x{opcode:02X}: CPU should be halted/locked after execution"
    );
    // PC must not have advanced past addr+1 (the opcode byte itself was
    // fetched and incremented PC to addr+1, but no further).
    assert_eq!(
        gb.cpu.pc,
        addr + 1,
        "Undefined opcode 0x{opcode:02X}: PC advanced too far after lock-up"
    );
}

/// gambatte undef_ops: opcode 0xD3 locks up the CPU.
#[test]
fn gambatte_undef_op_d3() {
    assert_undef_op_locks_cpu(0xD3);
}

/// gambatte undef_ops: opcode 0xDB locks up the CPU.
#[test]
fn gambatte_undef_op_db() {
    assert_undef_op_locks_cpu(0xDB);
}

/// gambatte undef_ops: opcode 0xE3 locks up the CPU.
#[test]
fn gambatte_undef_op_e3() {
    assert_undef_op_locks_cpu(0xE3);
}

/// gambatte undef_ops: opcode 0xE4 locks up the CPU.
#[test]
fn gambatte_undef_op_e4() {
    assert_undef_op_locks_cpu(0xE4);
}

/// gambatte undef_ops: opcode 0xEB locks up the CPU.
#[test]
fn gambatte_undef_op_eb() {
    assert_undef_op_locks_cpu(0xEB);
}

/// gambatte undef_ops: opcode 0xEC locks up the CPU.
#[test]
fn gambatte_undef_op_ec() {
    assert_undef_op_locks_cpu(0xEC);
}

/// gambatte undef_ops: opcode 0xED locks up the CPU.
#[test]
fn gambatte_undef_op_ed() {
    assert_undef_op_locks_cpu(0xED);
}

/// gambatte undef_ops: opcode 0xF4 locks up the CPU.
#[test]
fn gambatte_undef_op_f4() {
    assert_undef_op_locks_cpu(0xF4);
}

/// gambatte undef_ops: opcode 0xFC locks up the CPU.
#[test]
fn gambatte_undef_op_fc() {
    assert_undef_op_locks_cpu(0xFC);
}

/// gambatte undef_ops: opcode 0xFD locks up the CPU.
#[test]
fn gambatte_undef_op_fd() {
    assert_undef_op_locks_cpu(0xFD);
}

// ----------------------------------------------------------------------------
// HALT bug tests
//
// Source: gambatte/test/hwtests/halt/noime_ifandie_halt_*.asm
//
// When IME=0 and (IF & IE) != 0, executing HALT triggers the halt bug:
// the byte immediately following HALT is used as both the opcode and its
// first operand (i.e., the opcode-fetch PC is not incremented before
// fetching the first immediate byte).
// ----------------------------------------------------------------------------

/// gambatte halt: noime_ifandie_halt_lda_3c_dmg08_cgb04c_out3F
///
/// Setup: IME=0, IF=IE=0x11 (VBlank+Joypad). Code after HALT is LD A,0x3C
/// (bytes 0x3E 0x3C). Due to halt bug the operand re-reads the opcode byte
/// (0x3E), so A = 0x3E. There is NO interrupt dispatch (IME=0).
///
/// Expected result: A = 0x3E (halt bug causes LD A to load its own opcode
/// byte rather than 0x3C).
///
/// Note: the Gambatte reference ROM output of "3F" is produced after an
/// additional `inc a` (0x3C) that the display routine executes; the pure
/// CPU state just after the halt-bugged LD A is A=0x3E.
///
/// We verify the halt bug fires and A = 0x3E (operand = re-read opcode).
#[test]
fn gambatte_halt_bug_noime_lda_3c() {
    let mut gb = setup_gb();

    // Place code at WRAM: HALT (0x76), LD A,0x3C (0x3E 0x3C), NOP
    let base: u16 = 0xC000;
    write_code(
        &mut gb,
        base,
        &[
            0x76, // HALT
            0x3E, 0x3C, // LD A, 0x3C  (opcode=0x3E, operand=0x3C)
            0x00, // NOP
        ],
    );
    gb.cpu.pc = base;

    // IME=0 (default), IF=IE=0x11
    gb.ints.write_if(0x11);
    gb.ints.write_ie(0x11);
    // IME stays disabled (don't call gb.ints.enable())

    // Step 1: execute HALT → halt bug fires (IME=0, IF&IE != 0)
    gb.run_cpu();
    assert!(
        gb.cpu.is_halt_bug_triggered,
        "HALT with IME=0 and IF&IE!=0 should trigger halt bug"
    );
    assert!(
        !gb.cpu.is_halted,
        "CPU should not be fully halted with halt bug"
    );
    assert_eq!(gb.cpu.pc, base + 1, "PC should be at byte after HALT");

    // Step 2: execute next instruction WITH halt bug active
    // Opcode at base+1 = 0x3E (LD A, d8); halt bug re-reads base+1 = 0x3E as operand
    gb.run_cpu();
    assert!(
        !gb.cpu.is_halt_bug_triggered,
        "Halt bug flag should be cleared after use"
    );
    // A = 0x3E because the operand byte was re-read as the opcode itself
    assert_eq!(
        gb.cpu.a(),
        0x3E,
        "Halt bug: LD A,d8 should load opcode byte 0x3E as operand (not 0x3C)"
    );
    // PC ends up at base+2 (advanced normally after the bugged operand read)
    assert_eq!(gb.cpu.pc, base + 2, "PC after halted LD A should be base+2");
}

/// gambatte halt: noime_ifandie_halt_sra_dmg08_cgb04c_outF1
///
/// Setup: IME=0, IF=IE=0x11, E=1. Code after HALT is `SRA A` (CB 2F) then
/// `ADD A, E`. Due to halt bug, the byte after HALT (0xCB) is re-fetched as
/// the second byte of the CB-prefix dispatch. This means the CB-prefix opcode
/// is 0xCB (not 0x2F), which maps to CB CB = `SET 1, E`.
///
/// We verify the halt bug flag fires and then confirm CPU state after the
/// bugged instruction is consistent with halt-bug behavior.
#[test]
fn gambatte_halt_bug_noime_sra() {
    let mut gb = setup_gb();

    // Place code: HALT (0x76), SRA A via CB prefix (0xCB 0x2F), ADD A,E (0x83)
    let base: u16 = 0xC000;
    write_code(
        &mut gb,
        base,
        &[
            0x76, // HALT
            0xCB, 0x2F, // SRA A  (CB-prefix, then 0x2F)
            0x83, // ADD A, E
        ],
    );
    gb.cpu.pc = base;
    gb.cpu.af = 0x1100; // A=0x11 (arbitrary initial A)
    gb.cpu.de = 0x0001; // E=0x01

    // IME=0, IF=IE=0x11
    gb.ints.write_if(0x11);
    gb.ints.write_ie(0x11);

    // Step 1: HALT → halt bug
    gb.run_cpu();
    assert!(
        gb.cpu.is_halt_bug_triggered,
        "HALT halt bug should be triggered"
    );

    // Step 2: execute with halt bug — opcode = 0xCB (at base+1),
    // PC is then reset to base+1 before operand fetch, so CB dispatch
    // reads 0xCB again as the sub-opcode (CB CB = SET 1, E per CB table).
    gb.run_cpu();
    assert!(!gb.cpu.is_halt_bug_triggered, "Halt bug flag cleared");

    // The important assertion: E register should have bit 1 SET
    // (CB CB = SET 1, E: sets bit 1 of E)
    let e = (gb.cpu.de & 0xFF) as u8;
    assert_eq!(
        e & 0x02,
        0x02,
        "Halt bug with CB prefix: CB CB = SET 1, E should set bit 1 of E"
    );
}

// ----------------------------------------------------------------------------
// IRQ dispatch tests
//
// Source: gambatte/test/hwtests/irq_precedence/if_and_ie_0_*.asm
//
// These tests verify interrupt dispatch behavior: IF bit clearing, vector
// address selection, and the IE-clobber edge case when SP=$0000.
// ----------------------------------------------------------------------------

/// gambatte irq_precedence: if_and_ie_0_if_1_dmg08_cgb04c_outE4
///
/// Setup: SP=0x0000, IF=IE=0x04 (Timer), EI delay active.
/// After the EI instruction takes effect and the timer interrupt is
/// dispatched, the interrupt push with SP=0x0000 writes the high byte of
/// PC to 0xFFFF (IE register). If the high PC byte clears the timer bit
/// in IE, the interrupt is cancelled; IF is NOT cleared and remains 0x04
/// (read as 0xE4 with upper bits forced high).
///
/// We verify: after dispatch with SP=0x0000, IF still has the timer bit set
/// (= 0xE4) and PC jumped to 0x0000 (no valid interrupt was dispatched).
#[test]
fn gambatte_irq_precedence_if_and_ie_0_if_1() {
    let mut gb = setup_gb();

    // Place EI + NOP at 0xC100 so high byte of PC = 0xC1 when EI completes.
    // EI takes effect at the START of the next instruction (has_ei_delay=true).
    // When run_cpu() processes the NOP after EI, it first enables IME, then
    // executes NOP, and then checks for interrupts. At that point SP=0x0000,
    // so the push of PC's high byte (0xC1) goes to 0xFFFF (IE register).
    // 0xC1 & 0x04 = 0x00, so IE no longer has timer bit → dispatch cancelled.
    let base: u16 = 0xC100;
    write_code(
        &mut gb,
        base,
        &[
            0xFB, // EI
            0x00, // NOP  <- PC here = 0xC102 when interrupt check fires
        ],
    );
    gb.cpu.pc = base;
    gb.cpu.sp = 0x0000;

    // IF = IE = 0x04 (Timer)
    gb.ints.write_if(0x04);
    gb.ints.write_ie(0x04);

    // Step 1: execute EI — sets has_ei_delay, no dispatch yet
    gb.run_cpu();
    assert!(gb.cpu.has_ei_delay, "After EI, has_ei_delay should be set");

    // Step 2: execute NOP — at start of run_cpu IME is enabled, then NOP
    // executes, then interrupt check fires: SP=0x0000, high byte of PC
    // (0xC1) overwrites IE, timer bit is lost, dispatch cancelled.
    gb.run_cpu();

    // IF should still have the timer bit set (not acknowledged)
    assert_eq!(
        gb.ints.read_if(),
        0xE4,
        "Timer interrupt should NOT be acknowledged when IE is clobbered via SP=0x0000"
    );

    // IME should be disabled (interrupt dispatch always clears IME)
    assert!(
        !gb.ints.are_enabled(),
        "IME should be disabled after interrupt dispatch attempt"
    );

    // PC should be 0x0000 (the cancelled dispatch jumped to vector 0x0000)
    assert_eq!(
        gb.cpu.pc, 0x0000,
        "Cancelled interrupt dispatch should jump PC to 0x0000"
    );
}

/// gambatte irq_precedence: if_and_ie_0_vector_2_dmg08_cgb04c_out50
///
/// Setup: SP=0xD000 (WRAM, no collision with IF/IE), IF=IE=0x04 (Timer), EI.
/// Normal dispatch to timer vector 0x0050. IF timer bit is cleared, PC = 0x0050.
///
/// Note: SP must NOT be 0xFF10 here. With base=0xC000 the return PC=0xC002,
/// so SP-1=0xFF0F would write 0xC0 into IF (clearing it) and cancel dispatch.
/// Using SP=0xD000 ensures the push lands in WRAM with no collision.
#[test]
fn gambatte_irq_precedence_if_and_ie_0_vector_normal_timer() {
    let mut gb = setup_gb();

    // Place EI + NOP at 0xC000; return PC = 0xC002
    let base: u16 = 0xC000;
    write_code(
        &mut gb,
        base,
        &[
            0xFB, // EI
            0x00, // NOP
        ],
    );
    gb.cpu.pc = base;
    // SP in WRAM: push of high byte 0xC0 goes to 0xCFFF (not IF/IE)
    gb.cpu.sp = 0xD000;

    gb.ints.write_if(0x04);
    gb.ints.write_ie(0x04);

    // Step 1: EI
    gb.run_cpu();
    // Step 2: NOP → interrupt fires
    gb.run_cpu();

    // Timer bit (0x04) in IF should be cleared after normal dispatch
    assert_eq!(
        gb.ints.read_if() & 0x04,
        0x00,
        "Timer interrupt bit should be cleared after normal dispatch"
    );

    // PC should be at timer vector 0x0050
    assert_eq!(
        gb.cpu.pc, 0x0050,
        "After timer interrupt dispatch, PC should be 0x0050"
    );

    // IME should be disabled
    assert!(
        !gb.ints.are_enabled(),
        "IME should be disabled after interrupt dispatch"
    );

    // SP should have decremented by 2
    assert_eq!(
        gb.cpu.sp, 0xCFFE,
        "SP should have decremented by 2 after push"
    );
}

/// gambatte irq_precedence: if_and_ie_0_if_2_dmg08_cgb04c_outE1
///
/// Setup: SP=0x0001, IF=IE=0x04 (Timer), A=0xFD (pre-loaded), EI.
/// With SP=0x0001: the LOWER byte (lo) of PC goes to 0x0000, but that's
/// too late to cancel; the UPPER byte (hi) goes to 0x0001 (normal WRAM),
/// IE is NOT clobbered. Dispatch proceeds normally.
///
/// Expected: timer bit IS cleared (0xE0 in IF), PC = 0x0050.
#[test]
fn gambatte_irq_precedence_if_and_ie_0_if_2() {
    let mut gb = setup_gb();

    let base: u16 = 0xC000;
    write_code(
        &mut gb,
        base,
        &[
            0xFB, // EI
            0x00, // NOP
        ],
    );
    gb.cpu.pc = base;
    gb.cpu.sp = 0x0001;

    gb.ints.write_if(0x04);
    gb.ints.write_ie(0x04);

    // Step 1: EI
    gb.run_cpu();
    // Step 2: NOP → interrupt
    gb.run_cpu();

    // With SP=0x0001, upper byte of PC is written to 0x0000 (WRAM), not IE.
    // IE is not clobbered, so dispatch completes normally.
    assert_eq!(
        gb.ints.read_if() & 0x04,
        0x00,
        "Timer interrupt should be acknowledged when SP=0x0001 (IE not clobbered)"
    );
    assert_eq!(gb.cpu.pc, 0x0050, "PC should be at timer vector 0x0050");
}
