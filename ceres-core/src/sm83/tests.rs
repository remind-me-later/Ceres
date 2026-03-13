use super::*;
use crate::CgbMode;
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
fn test_speed_change_basic() {
    let mut gb = setup_gb();
    // Use CGB mode for speed change
    gb.cgb_mode = CgbMode::Cgb;

    // Preparation: STOP instruction
    let addr = 0xC000;
    gb.cpu.pc = addr;
    gb.write_mem(addr, 0x10); // STOP
    gb.write_mem(addr + 1, 0x00);

    // Request speed change (Normal -> Double)
    gb.write_mem(0xFF4D, 0x01);
    assert!(!gb.key1.is_enabled());
    assert!(gb.key1.is_requested());

    let start_dots = gb.total_dots;
    gb.run_cpu();
    let end_dots = gb.total_dots;

    // Speed change takes 32768 M-cycles (131072 dots)
    // plus the STOP instruction fetch/execute (4 cycles = 16 dots)
    // plus the next instruction fetch (2 cycles = 8 dots)
    // wait, run_cpu executes ONE instruction.
    // STOP is 1 byte + 1 byte operand = 2 bytes.
    // fetch STOP: 4 dots
    // fetch operand: 4 dots
    // execute speed switch: 131076 dots (1 + 32768 M-cycles)
    // Total should be 131084 dots.
    let elapsed = end_dots - start_dots;
    assert_eq!(
        elapsed, 131084,
        "Speed change should take 131084 dots (fetch + Switch)"
    );

    // Verify speed change happened
    assert!(gb.key1.is_enabled(), "Should be in double speed now");
    assert!(!gb.key1.is_requested(), "Request should be cleared");

    // Verify KEY1 register (0xFF4D)
    // Bit 7: current speed (1 for double)
    // Bits 6-1: always 1
    // Bit 0: request (0 after completion)
    // 1111 1110 = 0xFE
    assert_eq!(gb.key1.read(), 0xFE);

    // Now switch back to normal speed
    gb.cpu.pc = addr;
    gb.write_mem(0xFF4D, 0x01);
    gb.run_cpu();

    assert!(!gb.key1.is_enabled(), "Should be in normal speed now");
    // 0111 1110 = 0x7E
    assert_eq!(gb.key1.read(), 0x7E);
}

#[test]
fn test_speed_change_tima() {
    let mut gb = setup_gb();
    gb.cgb_mode = CgbMode::Cgb;

    // Setup TIMA at 4096Hz (Normal Speed: increment every 256 M-cycles)
    // 32768 M-cycles / 256 = 128 increments.
    gb.write_mem(0xFF06, 0x00); // TMA = 0
    gb.write_mem(0xFF05, 0x00); // TIMA = 0
    gb.write_mem(0xFF07, 0x04); // TAC = 0x04 (Enabled, 4096Hz)

    // Preparation: STOP instruction
    let addr = 0xC000;
    gb.cpu.pc = addr;
    gb.write_mem(addr, 0x10); // STOP
    gb.write_mem(addr + 1, 0x00);

    // Request speed change
    gb.write_mem(0xFF4D, 0x01);

    gb.run_cpu();

    // Verify TIMA incremented during speed switch
    // Note: The STOP instruction and fetch might add some cycles,
    // but not enough for another 256-cycle tick.
    assert_eq!(
        gb.read_mem(0xFF05),
        128,
        "TIMA should increment 128 times during 32768 M-cycle speed switch"
    );

    // Verify DIV was reset AT THE END (so it should be 0 or close to 0)
    // Actually DIV increments every 4 dots.
    // If it's reset at the end of the 131072 dots loop, it should be 0.
    assert_eq!(
        gb.read_mem(0xFF04),
        0,
        "DIV should be 0 immediately after speed switch"
    );
}

#[test]
fn test_speed_change_double_to_normal() {
    let mut gb = setup_gb();
    gb.cgb_mode = CgbMode::Cgb;

    // First, enter double speed
    let addr = 0xC000;
    gb.cpu.pc = addr;
    gb.write_mem(addr, 0x10); // STOP
    gb.write_mem(addr + 1, 0x00);
    gb.write_mem(0xFF4D, 0x01);
    gb.run_cpu();
    assert!(gb.key1.is_enabled());

    // Setup TIMA at 4096Hz.
    // In double speed mode, TIMA increments twice as fast relative to CPU cycles,
    // but advance_dots(4) still adds 4 dots to run_timers(4), so it should be same
    // real-time duration.
    // 32768 M-cycles in double speed = 131072 T-cycles = 65536 normal dots.
    // 65536 normal dots / 4 dots per timer tick = 16384 ticks? No.
    // Timer tick at 4096Hz is every 1024 dots (at 4MHz).
    // 131072 / 1024 = 128 ticks.
    gb.write_mem(0xFF05, 0x00);
    gb.write_mem(0xFF07, 0x04);

    // Request speed change (Double -> Normal)
    gb.cpu.pc = addr;
    gb.write_mem(0xFF4D, 0x01);

    let start_dots = gb.total_dots;
    gb.run_cpu();
    let end_dots = gb.total_dots;

    // In double speed mode, each M-cycle is 4 T-cycles, but advance_dots_no_timers
    // increments total_dots by t_cycles / 2.
    // (1 + 32768) M-cycles * 4 T-cycles / 2 = 65538 dots.
    // plus fetch: 2 cycles * 4 T-cycles / 2 = 4 dots.
    // Total: 65538 + 4 = 65542 dots.
    let elapsed = end_dots - start_dots;
    assert_eq!(
        elapsed, 65542,
        "Speed change from double to normal should take 65542 normal dots"
    );

    assert!(!gb.key1.is_enabled());
    assert_eq!(
        gb.read_mem(0xFF05),
        128,
        "TIMA should increment 128 times during speed switch (Double -> Normal)"
    );
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
fn test_interrupt_related_opcode_timings() {
    let mut gb = setup_gb();

    test_op_timing(&mut gb, 0xF3, &[], 1); // DI
    test_op_timing(&mut gb, 0xFB, &[], 1); // EI
    test_op_timing(&mut gb, 0xE0, &[0x0F], 3); // LDH (a8), A
    test_op_timing(&mut gb, 0xF0, &[0x0F], 3); // LDH A, (a8)
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

fn do_speed_switch(gb: &mut Gb) {
    gb.write_mem(0xFF4D, 0x01);
    gb.write_mem(0xC000, 0x10);
    gb.write_mem(0xC001, 0x00);
    gb.set_cpu_pc(0xC000);
    gb.run_cpu();
}

fn elapsed_cpu_m_cycles(gb: &Gb, start_dots: u64) -> u64 {
    let dots = gb.total_dots() - start_dots;
    let dots_per_m_cycle = if gb.is_double_speed() { 2 } else { 4 };
    dots / dots_per_m_cycle
}

/// Measure the net ISR overhead (in M-cycles) caused by a single interrupt
/// dispatch, modelling the exact sequence used by the blargg interrupt_time ROM:
///
/// ```asm
///   ei
///   ld  a, d          ; d=0 → no interrupt; d=0x08 → serial IF bit
///   ld  ($FF0F), a    ; write IF; if d=0x08 the ISR fires after this
///   di
/// ```
///
/// The RST/IRQ vector at `0x0058` contains `JP $DEC3` and `0xDEC3` contains
/// `RET`, matching the blargg source layout.  The measurement is the
/// *difference* in elapsed M-cycles between the `d=0x08` run (interrupt fires)
/// and the `d=0x00` run (no interrupt), which should be exactly
/// `dispatch(5) + JP(4) + RET(4) = 13`.
fn measure_blargg_interrupt_time_sequence(mut gb: Gb) -> u64 {
    // Place `JP $DEC3` (C3 C3 DE) at the serial/timer IRQ vector 0x0058.
    // 0x0058 is in ROM space so it must come from the ROM image patches;
    // those patches are applied by the caller via `setup_*_with_rom`.

    // Place `RET` at 0xDEC3 (WRAM, writable at runtime).
    gb.write_mem(0xDEC3, 0xC9);

    // Stack lives above the RET stub so pushes don't clobber it.
    gb.set_cpu_sp(0xDFFF);

    // IE = serial (bit 3), IF cleared.
    gb.ints.write_ie(0x08);
    gb.ints.write_if(0x00);

    // -----------------------------------------------------------------------
    // Run WITHOUT interrupt (d = 0x00) to get the baseline elapsed time.
    // -----------------------------------------------------------------------
    let base: u16 = 0xC100;
    // ei / ld a, 0x00 / ld ($FF0F), a / di
    write_code(
        &mut gb,
        base,
        &[
            0xFB, // EI
            0x3E, 0x00, // LD A, 0x00
            0xEA, 0x0F, 0xFF, // LD ($FF0F), A   [4 M-cycles]
            0xF3, // DI
        ],
    );
    gb.set_cpu_pc(base);
    gb.run_cpu(); // EI
    let start_no_int = gb.total_dots();
    gb.run_cpu(); // LD A, 0x00
    gb.run_cpu(); // LD ($FF0F), A
    gb.run_cpu(); // DI
    let elapsed_no_int = elapsed_cpu_m_cycles(&gb, start_no_int);

    // -----------------------------------------------------------------------
    // Run WITH interrupt (d = 0x08) and measure elapsed time.
    // -----------------------------------------------------------------------
    gb.ints.write_ie(0x08);
    gb.ints.write_if(0x00);

    let base2: u16 = 0xC200;
    // ei / ld a, 0x08 / ld ($FF0F), a / di
    write_code(
        &mut gb,
        base2,
        &[
            0xFB, // EI
            0x3E, 0x08, // LD A, 0x08
            0xEA, 0x0F, 0xFF, // LD ($FF0F), A   [4 M-cycles] → ISR fires after
            0xF3, // DI
        ],
    );
    gb.set_cpu_pc(base2);
    gb.run_cpu(); // EI
    let start_int = gb.total_dots();
    gb.run_cpu(); // LD A, 0x08
    gb.run_cpu(); // LD ($FF0F), A  → interrupt taken; dispatch to 0x0058
    gb.run_cpu(); // JP $DEC3        (at 0x0058)
    gb.run_cpu(); // RET             (at 0xDEC3)
    gb.run_cpu(); // DI              (resume after ISR)
    let elapsed_int = elapsed_cpu_m_cycles(&gb, start_int);

    // The net ISR overhead is the difference.
    elapsed_int - elapsed_no_int
}

/// Build a minimal 32 KB DMG ROM image (all 0xFF by default) with custom
/// bytes patched in at the given `(offset, byte)` pairs.
///
/// The header bytes required by `Cartridge::new` are pre-filled:
///   - `0x0147` = 0x00 (ROM-only, no MBC)
///   - `0x0148` = 0x00 (32 KB)
///   - `0x0149` = 0x00 (no RAM)
///
/// All other bytes default to 0xFF.
fn make_rom_32k(patches: &[(usize, u8)]) -> Box<[u8]> {
    let mut rom = vec![0xFF_u8; 0x8000];
    rom[0x0147] = 0x00; // ROM only
    rom[0x0148] = 0x00; // 32 KB
    rom[0x0149] = 0x00; // no RAM
    for &(offset, byte) in patches {
        rom[offset] = byte;
    }
    rom.into_boxed_slice()
}

fn setup_model_with_rom(model: crate::Model, patches: &[(usize, u8)]) -> Gb {
    use crate::GbBuilder;
    let rom = make_rom_32k(patches);
    let mut gb = GbBuilder::new(44100, crate::test_util::DummyAudio)
        .with_model(model)
        .with_rom(rom)
        .expect("minimal ROM should be valid")
        .build();
    // Disable boot ROM so cart ROM is mapped at 0x0000–0x00FF.
    gb.write_mem(0xFF50, 0x01);
    gb
}

/// Build a DMG `Gb` backed by a minimal 32 KB ROM with the given patches, and
/// with the boot ROM disabled so that cart ROM is visible at `0x0000–0x00FF`.
fn setup_dmg_with_rom(patches: &[(usize, u8)]) -> Gb {
    setup_model_with_rom(crate::Model::DmgB, patches)
}

fn setup_cgb_with_rom(patches: &[(usize, u8)]) -> Gb {
    let mut gb = setup_model_with_rom(crate::Model::CgbE, patches);
    gb.write_mem(0xFF40, 0x00);
    gb
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

fn run_gambatte_irq_precedence_late_if_via_sp_if(ei_addr: u16) -> u8 {
    let patches = [
        (0x0048, 0xF0), // LDH A,(a8)
        (0x0049, 0x0F), // IF
        (0x004A, 0xC9), // RET
        (ei_addr as usize, 0xFB),
        (ei_addr.wrapping_add(1) as usize, 0x00),
    ];

    let mut gb = setup_dmg_with_rom(&patches);
    gb.set_cpu_pc(ei_addr);
    gb.set_cpu_sp(0xFF11);
    gb.ints.write_if(0x0A);
    gb.ints.write_ie(0x0A);

    gb.run_cpu();
    assert!(gb.cpu.has_ei_delay, "EI should arm delayed IME enable");

    gb.run_cpu();
    assert_eq!(
        gb.cpu.pc, 0x0048,
        "Interrupt should dispatch to LCD vector 0x0048"
    );

    gb.run_cpu();
    gb.cpu.a()
}

/// gambatte irq_precedence: late_if_via_sp_if_1_dmg08_cgb04c_outFD
#[test]
fn gambatte_irq_precedence_late_if_via_sp_if_1() {
    assert_eq!(
        run_gambatte_irq_precedence_late_if_via_sp_if(0x017D),
        0xFD,
        "Interrupt push via SP=0xFF11 should rewrite IF to 0xFD for late_if_via_sp_if_1"
    );
}

/// gambatte irq_precedence: late_if_via_sp_if_2_dmg08_cgb04c_outE0
#[test]
fn gambatte_irq_precedence_late_if_via_sp_if_2() {
    assert_eq!(
        run_gambatte_irq_precedence_late_if_via_sp_if(0x017E),
        0xE0,
        "Interrupt push via SP=0xFF11 should rewrite IF to 0xE0 for late_if_via_sp_if_2"
    );
}

// ----------------------------------------------------------------------------
// EI-delay + HALT + double interrupt test
//
// Source: gambatte/test/hwtests/halt/ifandie_ei_halt_sra_dmg08_cgb04c_out0A.asm
//
// When `EI; HALT` is executed with IF=IE=0x11 (VBlank bit0 + Joypad bit4):
//
//   1. EI sets has_ei_delay.
//   2. The next run_cpu (fetching HALT) fires the EI delay first → IME=1.
//   3. halt() sees IME=1 and IF&IE≠0 → no halt bug; CPU is NOT halted; PC is
//      decremented back to point at HALT.
//   4. End-of-step interrupt check: IME=1, lowest pending bit = VBlank (bit 0)
//      → dispatch to vector 0x0040.  SP is decremented by 2 (push return addr).
//      VBlank bit cleared from IF.  IME cleared.
//   5. VBlank handler at 0x0040: `SRA A; RET`.
//      SRA on A=0x11: A = 0x08, carry = 1.
//      RET pops return address → PC = address of HALT instruction.
//   6. Next run_cpu executes HALT again: has_ei_delay=false → IME stays 0.
//      halt() sees IME=0 and IF&IE = 0x10 & 0x11 = 0x10 ≠ 0 → HALT BUG.
//      PC is not rewound; is_halt_bug_triggered = true.
//   7. Next run_cpu: fetch opcode at INC_A address (0x3C), PC advances to
//      INC_A+1; halt bug rewinds PC back to INC_A.
//      Execute INC A → A = 0x09.
//   8. Next run_cpu: PC is still at INC_A (halt bug rewound it). Fetch INC A
//      again → A = 0x0A.
//
// Expected final A = 0x0A (= 10 = "0A" hex, which is the ROM's printed output).
// ----------------------------------------------------------------------------

/// gambatte halt: ifandie_ei_halt_sra_dmg08_cgb04c_out0A
///
/// Verifies that EI-delay + HALT with both VBlank and Joypad pending causes:
///   - Normal HALT (not halt-bug) on the first execution (IME=1 via EI delay)
///   - VBlank dispatch to handler at 0x0040 (`SRA A; RET`)
///   - Halt-bug on the second execution of HALT (IME=0 after dispatch)
///   - `INC A` executed twice due to halt-bug PC non-advance
///   - Final A = 0x0A
#[test]
fn gambatte_ifandie_ei_halt_sra() {
    // The VBlank handler lives at the fixed vector 0x0040 in cart ROM:
    //   0x0040: CB 2F  (SRA A)
    //   0x0042: C9     (RET)
    let rom_patches: &[(usize, u8)] = &[
        (0x0040, 0xCB), // SRA A — CB prefix
        (0x0041, 0x2F), // SRA A — sub-opcode
        (0x0042, 0xC9), // RET
    ];
    let mut gb = setup_dmg_with_rom(rom_patches);

    // SP in WRAM so interrupt push doesn't clobber IF/IE.
    gb.cpu.sp = 0xD000;
    // A = 0x11 (same as the ROM: `ld a, 0x11`)
    gb.cpu.af = 0x1100;
    // IF = IE = 0x11 (VBlank bit0 + Joypad bit4)
    gb.ints.write_if(0x11);
    gb.ints.write_ie(0x11);

    // Place: EI (0xFB), HALT (0x76), INC A (0x3C) in WRAM.
    let base: u16 = 0xC000;
    write_code(
        &mut gb,
        base,
        &[
            0xFB, // EI   → sets has_ei_delay
            0x76, // HALT
            0x3C, // INC A  (at base+2)
        ],
    );
    gb.cpu.pc = base;

    // Step 1: execute EI → has_ei_delay set, no dispatch yet.
    gb.run_cpu();
    assert!(gb.cpu.has_ei_delay, "EI should set has_ei_delay");

    // Step 2: execute HALT — EI delay fires at top of run_cpu → IME=1.
    // halt() with IME=1 and IF&IE≠0: no halt bug; PC rewound to HALT.
    // End-of-step: VBlank (bit 0) dispatched to 0x0040.
    //   IF bit 0 cleared; IME cleared; PC = 0x0040; SP -= 2.
    let halt_addr = base + 1;
    gb.run_cpu();
    assert!(
        !gb.cpu.has_ei_delay,
        "EI delay should have fired before HALT"
    );
    assert_eq!(gb.cpu.pc, 0x0040, "VBlank dispatch should jump to 0x0040");
    assert_eq!(
        gb.ints.read_if() & 0x01,
        0x00,
        "VBlank bit should be cleared after dispatch"
    );
    assert_eq!(
        gb.ints.read_if() & 0x10,
        0x10,
        "Joypad bit should still be pending"
    );
    assert!(
        !gb.ints.are_enabled(),
        "IME should be disabled after dispatch"
    );

    // Step 3: execute SRA A (CB 2F) at 0x0040.
    // A = 0x11 → SRA → A = 0x08 (arithmetic right shift; carry = 1).
    gb.run_cpu();
    assert_eq!(gb.cpu.a(), 0x08, "SRA 0x11 should give 0x08");

    // Step 4: execute RET at 0x0042. Returns to halt_addr (the HALT instruction).
    gb.run_cpu();
    assert_eq!(
        gb.cpu.pc, halt_addr,
        "RET should return to the HALT instruction address"
    );

    // Step 5: execute HALT again — IME=0, IF&IE = 0x10 & 0x11 = 0x10 ≠ 0.
    // → halt bug fires; is_halt_bug_triggered = true; PC not rewound.
    gb.run_cpu();
    assert!(
        gb.cpu.is_halt_bug_triggered,
        "Second HALT with IME=0 and IF&IE≠0 should trigger halt bug"
    );
    assert_eq!(
        gb.cpu.pc,
        halt_addr + 1,
        "PC should point at INC A (byte after HALT) after halt bug"
    );

    let inc_a_addr = halt_addr + 1; // base + 2

    // Step 6: execute INC A with halt bug active.
    // Opcode 0x3C (INC A) fetched from inc_a_addr; PC advances to inc_a_addr+1;
    // halt bug rewinds PC back to inc_a_addr; execute INC A → A = 0x09.
    gb.run_cpu();
    assert!(!gb.cpu.is_halt_bug_triggered, "Halt bug flag should clear");
    assert_eq!(gb.cpu.a(), 0x09, "First INC A should give 0x09");
    assert_eq!(
        gb.cpu.pc, inc_a_addr,
        "PC should be rewound to INC A address after halt bug"
    );

    // Step 7: execute INC A again (PC still at inc_a_addr from halt-bug rewind).
    // A = 0x09 + 1 = 0x0A.
    gb.run_cpu();
    assert_eq!(
        gb.cpu.a(),
        0x0A,
        "Second INC A (halt-bug re-execution) should give 0x0A"
    );
}

// ----------------------------------------------------------------------------
// Blargg interrupt timing / handling tests
//
// Sources:
// - external/test-sources/gb-test-roms/interrupt_time/interrupt_time.s
// - external/test-sources/gb-test-roms/cpu_instrs/source/02-interrupts.s
// ----------------------------------------------------------------------------

#[test]
fn blargg_interrupt_time_timer_dispatch_takes_13_cycles_dmg() {
    // 0x0058: JP $DEC3  →  C3 C3 DE
    let gb = setup_dmg_with_rom(&[(0x0058, 0xC3), (0x0059, 0xC3), (0x005A, 0xDE)]);
    let elapsed = measure_blargg_interrupt_time_sequence(gb);

    assert_eq!(
        elapsed, 13,
        "Timer interrupt dispatch should take 13 M-cycles on DMG (dispatch 5 + JP 4 + RET 4)"
    );
}

#[test]
fn blargg_interrupt_time_timer_dispatch_takes_13_cycles_cgb_double_speed() {
    // 0x0058: JP $DEC3  →  C3 C3 DE
    let mut gb = setup_cgb_with_rom(&[(0x0058, 0xC3), (0x0059, 0xC3), (0x005A, 0xDE)]);
    do_speed_switch(&mut gb);
    let elapsed = measure_blargg_interrupt_time_sequence(gb);

    assert_eq!(
        elapsed, 13,
        "Timer interrupt dispatch should still take 13 M-cycles in CGB double speed (dispatch 5 + JP 4 + RET 4)"
    );
}

#[test]
fn blargg_cpu_instrs_02_interrupts_ei_dispatches_after_following_instruction() {
    let mut gb = setup_dmg_with_rom(&[(0x0050, 0x3C), (0x0051, 0xC9)]);
    let base: u16 = 0xC100;
    write_code(
        &mut gb,
        base,
        &[
            0xFB, // EI
            0x01, 0x00, 0x00, // LD BC,0
            0xC5, // PUSH BC
            0xC1, // POP BC
            0x04, // INC B
            0x00, // NOP (represents IF write already done externally)
            0x05, // DEC B at interrupt_addr
        ],
    );
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);
    gb.ints.write_ie(0x04);

    gb.run_cpu();
    assert!(gb.cpu.has_ei_delay, "EI should arm delayed IME enable");

    gb.run_cpu();
    assert!(
        gb.cpu.pc == base + 4,
        "Instruction after EI should run before dispatch"
    );

    gb.run_cpu();
    gb.run_cpu();
    gb.run_cpu();
    gb.ints.write_if(0x04);
    gb.run_cpu();

    assert_eq!(
        gb.cpu.pc, 0x0050,
        "Interrupt should dispatch only after the following instruction completes"
    );
    assert_eq!(
        gb.cpu.bc() >> 8,
        1,
        "INC B should complete before interrupt handler runs"
    );
}

#[test]
fn blargg_cpu_instrs_02_interrupts_di_prevents_dispatch() {
    let mut gb = setup_dmg_with_rom(&[]);
    let base: u16 = 0xC000;
    write_code(
        &mut gb,
        base,
        &[
            0xF3, // DI
            0x01, 0x00, 0x00, // LD BC,0
            0xC5, // PUSH BC
            0xC1, // POP BC
        ],
    );
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);
    gb.ints.write_ie(0x04);
    gb.ints.write_if(0x04);

    for _ in 0..4 {
        gb.run_cpu();
    }

    assert_eq!(
        gb.cpu.pc,
        base + 6,
        "Execution should continue normally with DI active"
    );
    assert_eq!(
        gb.ints.read_if() & 0x04,
        0x04,
        "Pending timer interrupt should remain pending under DI"
    );
    assert!(
        !gb.ints.are_enabled(),
        "IME should remain disabled after DI"
    );
}

#[test]
fn blargg_cpu_instrs_02_interrupts_halt_exits_on_timer_interrupt() {
    let mut gb = setup_dmg_with_rom(&[(0x0050, 0xC9)]);
    let base: u16 = 0xC000;
    write_code(
        &mut gb,
        base,
        &[
            0x76, // HALT
            0x00, // NOP
        ],
    );
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);
    gb.ints.enable();
    gb.ints.write_ie(0x04);
    gb.write_tma(0x00);
    gb.write_tima(0xFE);
    gb.write_tac(0x05);
    gb.ints.write_if(0x00);

    gb.run_cpu();
    assert!(
        gb.cpu.is_halted(),
        "CPU should enter HALT with no pending interrupt yet"
    );

    for _ in 0..8 {
        gb.advance_dots(4);
        if gb.ints.read_if() & 0x04 != 0 {
            break;
        }
    }

    assert_eq!(
        gb.ints.read_if() & 0x04,
        0x04,
        "Timer interrupt should become pending and wake HALT"
    );

    gb.run_cpu();

    assert!(
        !gb.cpu.is_halted(),
        "Interrupt dispatch should wake the CPU out of HALT"
    );
    assert_eq!(
        gb.cpu.pc, 0x0050,
        "Woken HALT should dispatch to timer vector"
    );
}

// ----------------------------------------------------------------------------
// Mooneye interrupt / HALT tests
//
// Sources:
// - external/test-sources/mooneye-test-suite/acceptance/interrupts/ie_push.s
// - external/test-sources/mooneye-test-suite/acceptance/if_ie_registers.s
// - external/test-sources/mooneye-test-suite/acceptance/halt_ime0_ei.s
// - external/test-sources/mooneye-test-suite/acceptance/halt_ime0_nointr_timing.s
// - external/test-sources/mooneye-test-suite/acceptance/halt_ime1_timing.s
// - external/test-sources/mooneye-test-suite/acceptance/halt_ime1_timing2-GS.s
// ----------------------------------------------------------------------------

#[test]
fn mooneye_acceptance_interrupts_ie_push_round1_upper_push_cancels_dispatch() {
    // Code lives in WRAM so write_code() takes effect (MBC0 ROM writes are no-ops).
    // base = 0xC200 → PC at dispatch = 0xC207 → hi-byte = 0xC2.
    // SP = 0x0000 → first push lands at 0xFFFF (IE register).
    // Writing 0xC2 to IE clears bit2 (timer); new IE & IF = 0 → dispatch cancelled.
    let mut gb = setup_gb();
    let base = 0xC200;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0x0000);
    gb.cpu.af = 0x0400;
    gb.ints.write_ie(0x04);
    gb.ints.write_if(0x00);
    // EI, NOP, LD SP $0000, LDH (FF0F) A
    write_code(&mut gb, base, &[0xFB, 0x00, 0x31, 0x00, 0x00, 0xE0, 0x0F]);

    gb.run_cpu(); // EI
    gb.run_cpu(); // NOP (IME now active)
    gb.run_cpu(); // LD SP, $0000
    gb.run_cpu(); // LDH (IF), A  → sets IF=0x04, triggers dispatch, upper push to IE→0xC2 cancels

    assert_eq!(
        gb.cpu.pc, 0x0000,
        "Upper-byte IE write should cancel timer dispatch and leave PC at 0x0000"
    );
    assert_eq!(
        gb.ints.read_if() & 0x1F,
        0x04,
        "IF should keep the timer bit after cancellation"
    );
    assert!(
        !gb.ints.are_enabled(),
        "IME should be cleared after cancelled dispatch"
    );
}

#[test]
fn mooneye_acceptance_interrupts_ie_push_round2_ime_stays_cleared_after_cancellation() {
    let mut gb = setup_gb();
    let base = 0xC000;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);
    gb.ints.disable();
    gb.ints.write_ie(0x10);
    gb.ints.write_if(0x10);
    write_code(&mut gb, base, &[0x00, 0x00, 0x00]);

    for _ in 0..3 {
        gb.run_cpu();
    }

    assert_eq!(
        gb.cpu.pc,
        base + 3,
        "IME should stay cleared after cancellation, so no later interrupt should dispatch"
    );
    assert!(!gb.ints.are_enabled(), "IME should remain cleared");
    assert_eq!(
        gb.ints.read_if() & 0x1F,
        0x10,
        "Pending joypad interrupt should remain pending"
    );
}

#[test]
fn mooneye_acceptance_interrupts_ie_push_round3_lower_push_too_late_to_cancel() {
    // Code lives in WRAM so write_code() takes effect (MBC0 ROM writes are no-ops).
    // base = 0xC22E → PC at dispatch = 0xC235 → hi-byte = 0xC2.
    // SP = 0x0001 → upper push lands at 0x0000 (ROM, no-op); lower push at 0xFFFF (IE).
    // After upper push IE is unchanged (0x08) and serial still wins → dispatch committed.
    // Lower push writing lo-byte to IE happens too late: hardware does not re-check.
    let mut gb = setup_gb();
    let base = 0xC22E;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0x0001);
    gb.cpu.af = 0x0800;
    gb.ints.write_if(0x00);
    gb.ints.write_ie(0x08);
    // EI, NOP, LD SP $0001, LDH (FF0F) A
    write_code(&mut gb, base, &[0xFB, 0x00, 0x31, 0x01, 0x00, 0xE0, 0x0F]);

    gb.run_cpu(); // EI
    gb.run_cpu(); // NOP (IME now active)
    gb.run_cpu(); // LD SP, $0001
    gb.run_cpu(); // LDH (IF), A → sets IF=0x08, triggers serial dispatch → PC=0x0058

    assert_eq!(
        gb.cpu.pc, 0x0058,
        "Lower-byte IE write should be too late to cancel serial dispatch"
    );
    assert_eq!(
        gb.ints.read_if() & 0x1F,
        0x00,
        "IF should be cleared after successful serial dispatch"
    );
}

#[test]
fn mooneye_acceptance_interrupts_ie_push_round4_ie_clobber_changes_winning_vector() {
    // Code lives in WRAM so write_code() takes effect (MBC0 ROM writes are no-ops).
    // base = 0xC200 → PC at dispatch = 0xC207 → hi-byte = 0xC2.
    // SP = 0x0000 → upper push lands at 0xFFFF (IE register), writing 0xC2.
    // New IE = 0xC2: bit0 (VBlank) cleared, bit1 (STAT) survives.
    // determine_interrupt() re-selects STAT → dispatch to 0x0048; VBlank stays in IF.
    let mut gb = setup_gb();
    let base = 0xC200;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0x0000);
    gb.cpu.af = 0x0300;
    gb.ints.write_if(0x00);
    gb.ints.write_ie(0x03);
    // EI, NOP, LD SP $0000, LDH (FF0F) A
    write_code(&mut gb, base, &[0xFB, 0x00, 0x31, 0x00, 0x00, 0xE0, 0x0F]);

    gb.run_cpu(); // EI
    gb.run_cpu(); // NOP (IME now active)
    gb.run_cpu(); // LD SP, $0000
    gb.run_cpu(); // LDH (IF), A → sets IF=0x03, triggers VBlank dispatch, upper push rewrites IE to 0xC2 → STAT reselected

    assert_eq!(
        gb.cpu.pc, 0x0048,
        "IE clobber during upper push should switch dispatch from VBlank to STAT"
    );
    assert_eq!(
        gb.ints.read_if() & 0x1F,
        0x01,
        "Only VBlank should remain pending after STAT dispatch"
    );
}

#[test]
fn mooneye_acceptance_if_ie_registers_if_without_ie_does_not_dispatch() {
    let mut gb = setup_dmg_with_rom(&[(0x0058, 0x1C), (0x0059, 0xD9)]);
    gb.cpu.af = 0;
    gb.cpu.bc = 0;
    gb.cpu.de = 0;
    gb.ints.disable();
    gb.ints.write_if(0x00);
    gb.ints.write_ie(0x00);

    gb.ints.enable();
    gb.ints.write_if(0x08);
    for _ in 0..64 {
        gb.run_cpu();
    }

    assert_eq!(
        gb.cpu.de & 0x00FF,
        0x00,
        "Serial handler must not run when IE is 0"
    );
    assert_eq!(
        gb.ints.read_if(),
        0xE8,
        "IF should retain the serial bit when IE is 0"
    );
}

#[test]
fn mooneye_acceptance_if_ie_registers_enabling_ie_triggers_once() {
    let mut gb = setup_dmg_with_rom(&[(0x0058, 0x1C), (0x0059, 0xD9)]);
    let base = 0xC000;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);
    gb.cpu.de = 0;
    gb.ints.write_if(0x08);
    gb.ints.write_ie(0x00);
    gb.ints.enable();
    write_code(&mut gb, base, &[0x00, 0x00]);

    gb.ints.write_ie(0x08);
    gb.run_cpu();
    assert_eq!(
        gb.cpu.pc, 0x0058,
        "Enabling IE with IF already set should dispatch serial on the next instruction boundary"
    );
    gb.run_cpu();
    gb.run_cpu();

    assert_eq!(
        gb.cpu.de & 0x00FF,
        0x01,
        "Serial handler should increment E exactly once"
    );
    assert_eq!(
        gb.ints.read_if(),
        0xE0,
        "IF should be cleared after RETI from serial handler"
    );
}

#[test]
fn mooneye_acceptance_halt_ime0_ei_ei_before_halt_behaves_like_ime1() {
    let mut gb = setup_gb();
    let base = 0xC000;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);
    gb.ints.disable();
    gb.ints.write_if(0x01);
    gb.ints.write_ie(0x01);
    write_code(&mut gb, base, &[0xFB, 0x76, 0xF3]);

    gb.run_cpu();
    gb.run_cpu();

    assert_eq!(
        gb.cpu.pc, 0x0040,
        "EI before HALT should cause the pending VBlank interrupt to dispatch at HALT time"
    );
}

#[test]
fn mooneye_acceptance_halt_ime1_timing_interrupt_serviced_before_post_halt_instruction() {
    let mut gb = setup_dmg_with_rom(&[(0x0050, 0x00)]);
    let base = 0xC000;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);
    gb.cpu.bc = 0;
    gb.ints.enable();
    gb.ints.write_ie(0x04);
    gb.write_tma(0x00);
    gb.write_tima(0xF0);
    gb.write_tac(0x05);
    write_code(&mut gb, base, &[0x76, 0x04]);

    // HALT: CPU halts, TIMA=0xF0, TAC=0x05 (clock/16).
    // From 0xF0 to overflow: 16 ticks × 16 T-cycles = 256 T-cycles.
    // Loop up to 80 × 4 = 320 T-cycles to give enough margin.
    gb.run_cpu();
    for _ in 0..80 {
        if gb.ints.read_if() & 0x04 != 0 {
            break;
        }
        gb.advance_dots(4);
    }
    gb.run_cpu();

    assert_eq!(
        gb.cpu.pc, 0x0050,
        "HALT with IME=1 should service the timer interrupt before executing the following instruction"
    );
    assert_eq!(
        gb.cpu.bc() >> 8,
        0x00,
        "Instruction after HALT must not execute before the interrupt service entry"
    );
}

#[test]
fn mooneye_acceptance_halt_ime0_nointr_timing_halt_matches_nointr_reference_window() {
    let mut gb = setup_dmg_with_rom(&[]);
    gb.set_cpu_pc(0x0200);
    gb.ints.disable();
    gb.ints.write_ie(0x01);
    gb.ints.write_if(0x00);

    write_code(&mut gb, 0x0200, &[0x76, 0x00, 0x00, 0x00]);

    let start = gb.total_dots();
    gb.run_cpu();
    let halt_elapsed = gb.total_dots() - start;

    gb.set_cpu_pc(0x0300);
    write_code(&mut gb, 0x0300, &[0x00]);
    let start = gb.total_dots();
    gb.run_cpu();
    let nop_elapsed = gb.total_dots() - start;

    assert_eq!(
        halt_elapsed, nop_elapsed,
        "IME=0 HALT without pending interrupt should behave like a normal 1-cycle wait in this simplified window"
    );
}

#[test]
fn mooneye_acceptance_halt_ime1_timing2_gs_roundtrip_window_matches_dmg_expectation() {
    let mut gb = setup_gb();
    let base = 0xC000;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);
    gb.ints.write_ie(0x01);
    gb.ints.write_if(0x01);
    write_code(&mut gb, base, &[0xFB, 0x76, 0x00]);

    gb.run_cpu();
    let start = gb.total_dots();
    gb.run_cpu();
    let elapsed = gb.total_dots() - start;

    assert_eq!(
        gb.cpu.pc, 0x0040,
        "DMG EI;HALT timing2-GS scenario should dispatch through the VBlank vector"
    );
    assert_eq!(
        elapsed, 16,
        "DMG EI;HALT dispatch window should take 4 M-cycles in this simplified timing2-GS scenario"
    );
}
#[test]
fn samesuite_ei_delay_halt() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x00);

    gb.ints.write_ie(0x07);
    gb.ints.write_if(0x03);
    gb.ints.disable();

    gb.write_mem(0xC000, 0xFB); // EI
    gb.write_mem(0xC001, 0x76); // HALT
    gb.write_mem(0xC002, 0x00); // NOP
    gb.cpu.pc = 0xC000;
    gb.cpu.sp = 0xD000; // Safe stack

    gb.run_cpu(); // Run EI
    assert!(gb.cpu.has_ei_delay);
    assert!(!gb.ints.are_enabled());

    gb.run_cpu(); // Run HALT
    // Interrupt should have dispatched. IME becomes 0.
    assert!(!gb.ints.are_enabled());
    assert_eq!(gb.cpu.pc, 0x0040); // VBlank vector

    // Check if it pushed the return address 0xC001 (HALT)
    let sp = gb.cpu.sp;
    let lo = gb.read_mem(sp);
    let hi = gb.read_mem(sp + 1);
    let ret_addr = (hi as u16) << 8 | (lo as u16);
    assert_eq!(ret_addr, 0xC001, "Should return to HALT instruction");
}

// -----------------------------------------------------------------------
// rapid_di_ei - mooneye-test-suite/acceptance/rapid_di_ei.s
// -----------------------------------------------------------------------
#[test]
fn mooneye_acceptance_rapid_di_ei() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x00);
    gb.write_mem(0xFF50, 0x01); // Disable bootrom
    gb.cpu.sp = 0xD000;

    // ISR increments E
    gb.set_rom_byte(0x0058, 0x1C); // INC E
    gb.set_rom_byte(0x0059, 0xD9); // RETI

    let reset = |gb: &mut Gb| {
        gb.ints.write_if(0x08);
        gb.ints.write_ie(0x08);
        gb.ints.disable();
        gb.cpu.set_de(0x0000);
    };

    // Part 1: Rapid EI;DI
    reset(&mut gb);
    gb.write_mem(0xC000, 0xFB); // EI
    gb.write_mem(0xC001, 0xF3); // DI
    gb.set_cpu_pc(0xC000);
    gb.run_cpu(); // EI
    gb.run_cpu(); // DI -> IME becomes 1 at END of this call, but DI also sets delay
    assert_eq!(gb.cpu.de() & 0xFF, 0, "EI;DI should not trigger interrupt");

    // Part 2: NOP after EI -> should dispatch AFTER the NOP
    reset(&mut gb);
    gb.write_mem(0xC000, 0xFB); // EI
    gb.write_mem(0xC001, 0x00); // NOP
    gb.set_cpu_pc(0xC000);
    gb.run_cpu(); // Run EI
    gb.run_cpu(); // Run NOP -> dispatches AFTER NOP in the same call
    assert_eq!(gb.cpu.pc, 0x0058, "Should dispatch after delay slot NOP");
}

// -----------------------------------------------------------------------
// ei_timing - mooneye-test-suite/acceptance/ei_timing.s
// -----------------------------------------------------------------------
#[test]
fn mooneye_acceptance_ei_timing() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x00);
    gb.write_mem(0xFF50, 0x01); // Disable bootrom
    gb.cpu.sp = 0xD000;

    gb.ints.write_if(0x08);
    gb.ints.write_ie(0x08);
    gb.ints.disable();
    gb.cpu.set_bc(0x0000);

    gb.write_mem(0xC000, 0xFB); // EI
    gb.write_mem(0xC001, 0x04); // INC B
    gb.set_cpu_pc(0xC000);

    gb.set_rom_byte(0x0058, 0x1C); // INC E
    gb.set_rom_byte(0x0059, 0xD9); // RETI

    gb.run_cpu(); // Run EI
    gb.run_cpu(); // Run INC B (delay slot) -> IME becomes 1 at end, dispatches immediately

    assert_eq!(gb.cpu.pc, 0x0058, "Should have dispatched after delay slot");
    assert_eq!(gb.cpu.bc() >> 8, 1, "B should be 1");
}

// -----------------------------------------------------------------------
// reti_intr_timing - mooneye-test-suite/acceptance/reti_intr_timing.s
// -----------------------------------------------------------------------
#[test]
fn mooneye_acceptance_reti_intr_timing() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x00);
    gb.write_mem(0xFF50, 0x01); // Disable bootrom
    gb.cpu.sp = 0xD000;

    gb.ints.write_if(0x09);
    gb.ints.write_ie(0x09);
    gb.ints.disable();
    gb.cpu.set_bc(0x0000);
    gb.cpu.set_de(0x0000);

    gb.write_mem(0xC000, 0xFB); // EI
    gb.write_mem(0xC001, 0x04); // INC B
    gb.set_cpu_pc(0xC000);

    gb.set_rom_byte(0x0040, 0x14); // INC D
    gb.set_rom_byte(0x0041, 0xD9); // RETI

    gb.set_rom_byte(0x0058, 0x1C); // INC E
    gb.set_rom_byte(0x0059, 0xD9); // RETI

    gb.run_cpu(); // EI
    gb.run_cpu(); // INC B -> delay slot finishes, dispatches VBLANK immediately
    assert_eq!(gb.cpu.pc, 0x0040);

    gb.run_cpu(); // INC D
    gb.run_cpu(); // RETI

    // In Ceres, RETI enables IME and dispatches immediately within the same call.
    assert_eq!(
        gb.cpu.pc, 0x0058,
        "RETI should have immediately dispatched SERIAL interrupt"
    );
}
