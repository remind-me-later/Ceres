use super::*;

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
