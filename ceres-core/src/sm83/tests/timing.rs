use super::*;

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
