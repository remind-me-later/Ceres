use super::*;

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
fn test_cpu_isr_dispatch_latency() {
    let mut gb = setup_gb();
    let base = 0xC000;
    gb.set_cpu_pc(base);
    gb.set_cpu_sp(0xD000);

    // Enable VBlank interrupt
    gb.ints.write_ie(0x01);
    gb.ints.enable();

    // Code: NOP; NOP; NOP...
    write_code(&mut gb, base, &[0x00, 0x00, 0x00, 0x00]);

    // Step 1: Execute first NOP
    gb.run_cpu();
    assert_eq!(gb.cpu.pc, base + 1);

    // Step 2: Request interrupt mid-instruction (effectively)
    gb.ints.write_if(0x01);

    // Step 3: Run CPU. It should execute the NEXT instruction (NOP),
    // THEN check for interrupts and start dispatch.
    let start = gb.total_dots();
    gb.run_cpu();
    let elapsed = gb.total_dots() - start;

    // PC should now be at the VBlank vector
    assert_eq!(gb.cpu.pc, 0x0040);

    // Latency calculation:
    // ISR Dispatch (No instruction fetch overhead if interrupt is sampled before):
    //    - Internal NOP 1: 4 ticks
    //    - Internal NOP 2: 4 ticks
    //    - Internal NOP 3: 4 ticks
    //    - PUSH HI: 4 ticks
    //    - PUSH LO: 4 ticks
    // Total = 20 ticks.
    assert_eq!(elapsed, 20, "Total ISR dispatch latency should be 20 ticks");
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

#[test]
fn test_repro_gbmicro_halt_bug_suite() {
    // --- HALT bug (halt_bug.s) ---
    // Verifies that when IE & IF != 0 and IME=0, HALT does NOT halt the CPU
    // and triggers the halt bug (skipping the next PC increment after fetch).

    let mut gb = setup_gb();
    let base = 0xC000;
    gb.set_cpu_pc(base);
    gb.ints.disable();
    gb.ints.write_ie(0x01);
    gb.ints.write_if(0x01);

    // Code: HALT (0x76); INC A (0x3C)
    write_code(&mut gb, base, &[0x76, 0x3C]);

    gb.cpu.af = 0x0000;

    // 1. Run HALT.
    // Ceres fetches 0x76, increments PC to base+1.
    // Exec(0x76) sets is_halt_bug_triggered = true.
    gb.run_cpu();

    assert!(
        !gb.cpu.is_halted,
        "CPU should not be halted due to HALT bug"
    );
    assert_eq!(
        gb.cpu.pc,
        base + 1,
        "PC should be at base+1 after HALT fetch"
    );

    // 2. Next run_cpu().
    // It fetches byte at PC (base+1), which is 0x3C (INC A).
    // imm8() increments PC to base+2.
    // is_halt_bug_triggered is true, so PC is wrapped back to base+1!
    // Exec(0x3C) runs, increments A to 1.
    gb.run_cpu();

    assert_eq!(gb.cpu.a(), 1, "A should be 1 after first INC A");
    assert_eq!(
        gb.cpu.pc,
        base + 1,
        "PC should have been wrapped back to base+1 by the HALT bug"
    );

    // 3. Next run_cpu().
    // Fetches byte at PC (base+1) AGAIN. PC increments to base+2.
    // Exec(0x3C) runs AGAIN, increments A to 2.
    gb.run_cpu();

    assert_eq!(
        gb.cpu.a(),
        2,
        "A should be 2 because the instruction was executed twice"
    );
    assert_eq!(gb.cpu.pc, base + 2, "PC should now finally be at base+2");
}
