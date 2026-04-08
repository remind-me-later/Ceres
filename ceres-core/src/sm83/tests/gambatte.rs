use super::*;

#[test]
fn gambatte_undef_op_d3() {
    assert_undef_op_locks_cpu(0xD3);
}

#[test]
fn gambatte_undef_op_db() {
    assert_undef_op_locks_cpu(0xDB);
}

#[test]
fn gambatte_undef_op_e3() {
    assert_undef_op_locks_cpu(0xE3);
}

#[test]
fn gambatte_undef_op_e4() {
    assert_undef_op_locks_cpu(0xE4);
}

#[test]
fn gambatte_undef_op_eb() {
    assert_undef_op_locks_cpu(0xEB);
}

#[test]
fn gambatte_undef_op_ec() {
    assert_undef_op_locks_cpu(0xEC);
}

#[test]
fn gambatte_undef_op_ed() {
    assert_undef_op_locks_cpu(0xED);
}

#[test]
fn gambatte_undef_op_f4() {
    assert_undef_op_locks_cpu(0xF4);
}

#[test]
fn gambatte_undef_op_fc() {
    assert_undef_op_locks_cpu(0xFC);
}

#[test]
fn gambatte_undef_op_fd() {
    assert_undef_op_locks_cpu(0xFD);
}

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

#[test]
fn gambatte_irq_precedence_late_if_via_sp_if_1() {
    assert_eq!(
        run_gambatte_irq_precedence_late_if_via_sp_if(0x017D),
        0xFD,
        "Interrupt push via SP=0xFF11 should rewrite IF to 0xFD for late_if_via_sp_if_1"
    );
}

#[test]
fn gambatte_irq_precedence_late_if_via_sp_if_2() {
    assert_eq!(
        run_gambatte_irq_precedence_late_if_via_sp_if(0x017E),
        0xE0,
        "Interrupt push via SP=0xFF11 should rewrite IF to 0xE0 for late_if_via_sp_if_2"
    );
}

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
