use super::*;

#[test]
fn gambatte_lycint_ly_1_out5() {
    let mut gb = setup_gb();

    // Turn LCD on
    gb.write_mem(0xFF40, 0x80);

    // Wait until LY=3, mirroring the ROM's "wait for LY=3" preamble
    advance_to_ly(&mut gb, 3);

    // Set LYC=5 and enable LYC coincidence interrupt
    gb.write_mem(0xFF45, 5); // LYC = 5
    gb.write_mem(0xFF41, 0x40); // STAT: enable LYC=LY interrupt
    gb.ints.write_if(0); // clear IF

    // Advance until the LCD STAT interrupt fires (IF bit 1 set)
    let mut interrupt_tick = None;
    for t in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            interrupt_tick = Some(t);
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        interrupt_tick.is_some(),
        "LYC interrupt never fired (LYC=5)"
    );

    // At this point the interrupt just fired.  "Few nops" corresponds to reading
    // LY ~10 ticks after the interrupt fires — LY should still read 5.
    let ly_early = gb.ppu.read_ly();
    assert_eq!(
        ly_early, 5,
        "gambatte lycint_ly_1: LY read immediately after LYC=5 IRQ should be 5 (got {ly_early})"
    );
}

#[test]
fn gambatte_lycint_ly_2_out6() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);

    gb.write_mem(0xFF45, 5);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    // Wait for LYC interrupt
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance well past the LY update point — LY should now be 6.
    // On DMG the LY increment for line N+1 happens at T-cycle ~4 of OAM scan.
    // One scanline is 912 T-cycles; we only need to advance a little to cross
    // the boundary between LY=5 and LY=6.
    for _ in 0..1000 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
        if gb.ppu.read_ly() == 6 {
            break;
        }
    }

    let ly_late = gb.ppu.read_ly();
    assert_eq!(
        ly_late, 6,
        "gambatte lycint_ly_2: LY after advancing past LY=5→6 boundary should be 6 (got {ly_late})"
    );
}

#[test]
fn gambatte_lycint_lycirq_1_out1() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);

    gb.write_mem(0xFF45, 5); // LYC = 5
    gb.write_mem(0xFF41, 0x40); // enable LYC int
    gb.ints.write_if(0);

    // Wait for interrupt on LY=5
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert_eq!(
        gb.ppu.read_ly(),
        5,
        "precondition: interrupt should fire while LY is 5"
    );

    // Immediately after the interrupt (LY=5 still), change LYC to 6.
    // LY≠LYC=6, so the LYC flag should drop and no new interrupt fires.
    gb.write_mem(0xFF45, 6);
    gb.ints.write_if(0); // clear the just-fired interrupt

    // The STAT interrupt line should not re-trigger because LYC≠LY.
    // Read IF — should be 0x00 (no pending STAT IRQ), i.e. IF & 0x02 == 0.
    let if_val = gb.ints.read_if() & 0x02;
    assert_eq!(
        if_val,
        0,
        "gambatte lycint_lycirq_1: no re-trigger expected when LYC set to non-matching value (IF={:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_lycint_lycirq_2_out3() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);

    gb.write_mem(0xFF45, 5);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    // Advance until the LYC interrupt fires
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance until LY becomes 6
    for _ in 0..2000 {
        if gb.ppu.read_ly() == 6 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert_eq!(gb.ppu.read_ly(), 6, "precondition: must reach LY=6");

    // Now set LYC=6 while LY=6 → should immediately re-trigger the LYC interrupt.
    gb.ints.write_if(0);
    gb.write_mem(0xFF45, 6);

    // A new STAT interrupt should be pending immediately.
    let if_val = gb.ints.read_if() & 0x02;
    assert_eq!(
        if_val,
        0x02,
        "gambatte lycint_lycirq_2: writing LYC=6 while LY=6 should trigger STAT IRQ (IF={:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_lycint_lycflag_1_out0() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);
    gb.write_mem(0xFF45, 5);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert_eq!(gb.ppu.read_ly(), 5, "precondition: IRQ fires during LY=5");

    // Set LYC to non-matching value — LYC flag should clear.
    gb.write_mem(0xFF45, 6);

    let stat = gb.ppu.read_stat() & 0x07;
    // LYC flag (bit 2) should be 0, mode should be 2 (OAM scan) or 0 (HBlank).
    // Key invariant: LYC coincidence bit (bit 2) = 0 because LYC≠LY.
    assert_eq!(
        stat & 0x04,
        0,
        "gambatte lycint_lycflag_1: LYC flag must be clear when LYC≠LY (STAT&7={stat:#04X})"
    );
}

#[test]
fn gambatte_lycint_lycflag_3_out4() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);
    gb.write_mem(0xFF45, 5);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    // Wait for LYC interrupt
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance into HBlank of line 5
    advance_to_mode(&mut gb, 0);
    let ly_in_hblank = gb.ppu.read_ly();

    // Set LYC to match current LY while in HBlank → LYC flag should be set.
    gb.write_mem(0xFF45, ly_in_hblank);

    let stat = gb.ppu.read_stat() & 0x07;
    // Should see LYC flag (bit 2 = 0x04) set; mode should be 0 (HBlank).
    assert_eq!(
        stat & 0x04,
        0x04,
        "gambatte lycint_lycflag_3: LYC flag must be set in HBlank when LYC=LY (STAT&7={stat:#04X})"
    );
    assert_eq!(
        stat & 0x03,
        0,
        "gambatte lycint_lycflag_3: mode must be HBlank (0) (STAT&7={stat:#04X})"
    );
}

#[test]
fn gambatte_m2int_m2irq_1_out0() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);

    // Wait for Mode 3 to appear (steady-state line)
    advance_to_mode(&mut gb, 3);

    // Enable Mode 2 STAT interrupt and clear IF
    gb.write_mem(0xFF41, 0x20);
    gb.ints.write_if(0);

    // Wait for the STAT interrupt to fire
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "Mode 2 STAT interrupt should have fired"
    );

    // Simulate ISR dispatch: clear IF bit 1 (CPU acknowledges the interrupt)
    gb.ints.acknowledge_interrupt(0x02);

    // Immediately read IF — should be 0 (no new interrupt yet)
    let if_val = gb.ints.read_if() & 0x02;
    assert_eq!(
        if_val,
        0,
        "gambatte m2int_m2irq_1: immediately after dispatch IF&2 should be 0 (got {:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_m2int_m2irq_2_out2() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 3);

    gb.write_mem(0xFF41, 0x20);
    gb.ints.write_if(0);

    // Wait for first Mode 2 interrupt
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Clear IF (ISR dispatch)
    gb.ints.acknowledge_interrupt(0x02);

    // Advance past the rest of this line and into the next line's Mode 2.
    // One full scanline is 912 ticks, so waiting >912 ticks guarantees the
    // next Mode 2 interrupt fires.
    for _ in 0..1200 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let if_val = gb.ints.read_if() & 0x02;
    assert_eq!(
        if_val,
        0x02,
        "gambatte m2int_m2irq_2: a new Mode 2 IRQ should fire within the next scanline (IF={:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_m2int_m2irq_ifw_1_out2() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 3);

    gb.write_mem(0xFF41, 0x20);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "Mode 2 STAT interrupt should have fired"
    );

    gb.ints.write_if(0);

    for _ in 0..1200 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    assert_eq!(
        gb.ints.read_if() & 0x02,
        0x02,
        "gambatte m2int_m2irq_ifw_1: clearing IF in the handler should still allow the next Mode 2 IRQ to become pending"
    );
}

#[test]
fn gambatte_m2int_m2irq_ifw_2_out0() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 3);

    gb.write_mem(0xFF41, 0x20);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "Mode 2 STAT interrupt should have fired"
    );

    gb.ints.write_if(0);

    // Read before the next line's Mode 2 boundary.
    for _ in 0..400 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    assert_eq!(
        gb.ints.read_if() & 0x02,
        0x00,
        "gambatte m2int_m2irq_ifw_2: IF should still be clear before the next Mode 2 IRQ boundary"
    );
}

#[test]
fn gambatte_m2int_m2stat_1_out2() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);

    // Advance to a steady-state scanline (line 1+)
    advance_to_mode(&mut gb, 3);

    // Enable Mode 2 STAT interrupt
    gb.write_mem(0xFF41, 0x20);
    gb.ints.write_if(0);

    // Wait for interrupt
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "Mode 2 interrupt should have fired"
    );

    // Simulate ISR dispatch (approx 20 T-cycles) so we are past the
    // 4-tick early IRQ window and into Mode 2.
    for _ in 0..20 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // ISR reads STAT: should be Mode 2 (OAM scan)
    let stat_mode = gb.ppu.read_stat() & 0x03;
    assert_eq!(
        stat_mode, 2,
        "gambatte m2int_m2stat_1: STAT mode should be 2 after Mode 2 IRQ dispatch (got {stat_mode})"
    );
}

#[test]
fn gambatte_m2int_m2stat_2_out3() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 3);

    gb.write_mem(0xFF41, 0x20);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance until Mode 3 (Drawing) is visible — simulates "many nops" in ISR
    for _ in 0..1000 {
        if gb.ppu.read_stat() & 0x03 == 3 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let stat_mode = gb.ppu.read_stat() & 0x03;
    assert_eq!(
        stat_mode, 3,
        "gambatte m2int_m2stat_2: STAT mode should be 3 after advancing past OAM scan (got {stat_mode})"
    );
}

#[test]
fn gambatte_lycint_m0stat_1_out0() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);

    gb.write_mem(0xFF45, 5);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(gb.ints.read_if() & 0x02 != 0, "LYC interrupt should fire");

    // Advance into HBlank (Mode 0) of line 5
    advance_to_mode(&mut gb, 0);

    let stat_mode = gb.ppu.read_stat() & 0x03;
    assert_eq!(
        stat_mode, 0,
        "gambatte lycint_m0stat_1: STAT mode should be 0 (HBlank) on line 5 (got {stat_mode})"
    );
}

#[test]
fn gambatte_lycint_m0stat_2_out2() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);

    gb.write_mem(0xFF45, 5);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Wait until LY=6 and Mode 2 (OAM scan of line 6)
    for _ in 0..2000 {
        if gb.ppu.read_ly() == 6 && gb.ppu.read_stat() & 0x03 == 2 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let stat_mode = gb.ppu.read_stat() & 0x03;
    assert_eq!(
        stat_mode, 2,
        "gambatte lycint_m0stat_2: STAT mode should be 2 (OAM scan) for line 6 (got {stat_mode})"
    );
    assert_eq!(
        gb.ppu.read_ly(),
        6,
        "gambatte lycint_m0stat_2: LY should be 6 (got {})",
        gb.ppu.read_ly()
    );
}

#[test]
fn gambatte_lycint152_ly153_1_out98() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);

    // Advance to LY=150
    advance_to_ly(&mut gb, 150);

    // Set LYC=152 and enable LYC interrupt
    gb.write_mem(0xFF45, 152);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    // Wait for the interrupt
    for _ in 0..10_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "LYC=152 interrupt should fire"
    );

    // Immediately read LY — should be 152
    let ly = gb.ppu.read_ly();
    assert_eq!(
        ly, 152,
        "gambatte lycint152_ly153_1: fast read after LYC=152 IRQ should give LY=152 (got {ly})"
    );
}

#[test]
fn gambatte_lycint152_ly153_2_out99() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 150);

    gb.write_mem(0xFF45, 152);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    for _ in 0..10_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance until LY becomes 153
    for _ in 0..2000 {
        if gb.ppu.read_ly() == 153 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let ly = gb.ppu.read_ly();
    assert_eq!(
        ly, 153,
        "gambatte lycint152_ly153_2: after advancing LY should be 153 (got {ly})"
    );
}

#[test]
fn gambatte_lycint152_ly153_3_out00() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 150);

    gb.write_mem(0xFF45, 152);
    gb.write_mem(0xFF41, 0x40);
    gb.ints.write_if(0);

    for _ in 0..10_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Advance past line 153 wrap — LY must reach 0 again.
    // Line 153 is only ~912 ticks; afterwards LY resets to 0.
    for _ in 0..2000 {
        if gb.ppu.read_ly() == 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let ly = gb.ppu.read_ly();
    assert_eq!(
        ly, 0,
        "gambatte lycint152_ly153_3: after line 153 wrap LY should be 0 (got {ly})"
    );
}

#[test]
fn gambatte_m0int_m0irq_1_out0() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);

    // Wait for Mode 2 (OAM scan)
    advance_to_mode(&mut gb, 2);

    // Enable Mode 0 (HBlank) STAT interrupt
    gb.write_mem(0xFF41, 0x08);
    gb.ints.write_if(0);

    // Wait for the STAT interrupt
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "HBlank STAT interrupt should fire"
    );

    // ISR acknowledges interrupt
    gb.ints.acknowledge_interrupt(0x02);

    let if_val = gb.ints.read_if() & 0x02;
    assert_eq!(
        if_val,
        0,
        "gambatte m0int_m0irq_1: IF&2 should be 0 immediately after HBlank IRQ dispatch (got {:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_m0int_m0irq_2_out2() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 2);

    gb.write_mem(0xFF41, 0x08);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Acknowledge first HBlank interrupt
    gb.ints.acknowledge_interrupt(0x02);

    // Advance more than one scanline to guarantee next HBlank fires
    for _ in 0..1200 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let if_val = gb.ints.read_if() & 0x02;
    assert_eq!(
        if_val,
        0x02,
        "gambatte m0int_m0irq_2: next HBlank IRQ should fire within ~1 scanline (IF={:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_late_m0irq_retrigger_1_oute2() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 2);

    gb.write_mem(0xFF41, 0x08);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "HBlank STAT interrupt should fire"
    );

    gb.write_mem(0xFF41, 0x08);
    gb.ints.write_if(0);

    for _ in 0..1200 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    assert_eq!(
        gb.ints.read_if(),
        0xE2,
        "gambatte late_m0irq_retrigger_1: IF should show a retriggered LCD interrupt (got {:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_late_m0irq_retrigger_2_oute0() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 2);

    gb.write_mem(0xFF41, 0x08);
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "HBlank STAT interrupt should fire"
    );

    gb.write_mem(0xFF41, 0x08);
    gb.ints.write_if(0);

    for _ in 0..400 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    assert_eq!(
        gb.ints.read_if(),
        0xE0,
        "gambatte late_m0irq_retrigger_2: IF should remain clear at the later sampling point (got {:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_ff40_disable_1_out0() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 91);

    gb.write_mem(0xFF45, 93); // LYC = 93
    gb.write_mem(0xFF41, 0x40); // enable LYC int
    gb.ints.write_if(0);

    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    assert!(
        gb.ints.read_if() & 0x02 != 0,
        "LYC=93 interrupt should fire"
    );

    // Change LYC to 94 (non-matching while LY=93) and disable LCD
    gb.write_mem(0xFF45, 94);
    gb.ints.acknowledge_interrupt(0x02);
    gb.write_mem(0xFF40, 0x00); // LCD off

    // Advance a full scanline; no new STAT IRQ should fire
    for _ in 0..1200 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    let if_val = gb.ints.read_if() & 0x02;
    assert_eq!(
        if_val,
        0,
        "gambatte ff40_disable_1: no STAT IRQ after LCD disable (IF={:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_oam_preread_blocking_starts_at_tick4_dmg() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Advance to tick 3 of OamScan on line 1
    advance_to_oam_scan_tick(&mut gb, 1, 3, crate::CgbMode::Dmg, false);

    // Before tick-3 logic runs: blocking must be off
    assert!(
        !gb.ppu.oam_read_blocked,
        "oam_read_blocked should be false before tick-3 logic runs"
    );

    // Execute tick 3 — hardware does NOT block here yet
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);

    // After tick-3 logic: blocking must STILL be false (hardware blocks at tick 4)
    assert!(
        !gb.ppu.oam_read_blocked,
        "oam_read_blocked must remain false after tick 3 (hardware blocks at tick 4, not tick 3)"
    );

    // Execute tick 4 — NOW hardware blocks
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);

    assert!(
        gb.ppu.oam_read_blocked,
        "oam_read_blocked must be true after tick 4"
    );
}

#[test]
fn gambatte_oam_preread_blocking_starts_at_tick4_cgb() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    advance_to_oam_scan_tick(&mut gb, 1, 3, crate::CgbMode::Cgb, false);

    assert!(
        !gb.ppu.oam_read_blocked,
        "oam_read_blocked should be false before tick-3 logic runs (CGB)"
    );

    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);

    assert!(
        !gb.ppu.oam_read_blocked,
        "oam_read_blocked must remain false after tick 3 on CGB (hardware blocks at tick 4)"
    );

    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);

    assert!(
        gb.ppu.oam_read_blocked,
        "oam_read_blocked must be true after tick 4 on CGB"
    );
}

#[test]
fn gambatte_oam_preread_blocking_boundary_cgb_double_speed() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // In double-speed, tick 3 must NOT set oam_read_blocked (it's gated on !double_speed)
    advance_to_oam_scan_tick(&mut gb, 1, 3, crate::CgbMode::Cgb, true);

    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, true); // process tick 3

    assert!(
        !gb.ppu.oam_read_blocked,
        "oam_read_blocked must stay false at tick 3 in double-speed mode"
    );

    // Advance to tick 9 (just before tick 10 unconditional block)
    advance_to_oam_scan_tick(&mut gb, 1, 9, crate::CgbMode::Cgb, true);

    assert!(
        !gb.ppu.oam_read_blocked,
        "oam_read_blocked must still be false at tick 9 in double-speed mode"
    );

    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, true); // process tick 9 → tick 10

    // After tick-10 logic: oam_read_blocked must be true
    assert!(
        gb.ppu.oam_read_blocked,
        "oam_read_blocked must be true after tick 10 in double-speed mode (gambatte preread_ds_2)"
    );
}

#[test]
fn gambatte_oam_prewrite_blocking_boundary_cgb() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // Pre-load OAM[0] with 0x00 via DMA write (bypasses blocking)
    gb.ppu.write_oam_by_dma(0xFE00, 0x00);

    advance_to_oam_scan_tick(&mut gb, 1, 3, crate::CgbMode::Cgb, false);

    // At tick 3, write blocking must be off — a write to OAM[0] must succeed
    assert!(
        !gb.ppu.oam_write_blocked,
        "oam_write_blocked must be false at tick 3 (write should succeed)"
    );

    // Perform the write via the normal (blocking-aware) path
    gb.ppu.write_oam(0xFE00, 0x01);

    // Verify the value was written (read bypassing blocking)
    let raw = gb.ppu.oam().read(0);
    assert_eq!(
        raw, 0x01,
        "OAM[0] write at tick 3 should succeed (expected 0x01, got {:#04x})",
        raw
    );

    // Execute tick 3 → tick 4
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);

    // After tick 4 blocking kicks in — writes should be silently dropped
    assert!(
        gb.ppu.oam_write_blocked,
        "oam_write_blocked must be true after tick 4"
    );
}

#[test]
fn gambatte_lycint_lycflag_1_failure() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);
    gb.write_mem(0xFF45, 5);
    gb.write_mem(0xFF41, 0x40); // enable LYC int
    gb.ints.write_if(0);

    // Wait for the LYC interrupt
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // ROM sets LYC=6 immediately after IRQ
    gb.write_mem(0xFF45, 6);

    let stat = gb.read_mem(0xFF41) & 0x07;
    // Expected 0x00 (Mode 0, LYC flag clear) or at least LYC flag clear.
    // Integration test says Ceres fails this.
    assert_eq!(
        stat & 0x04,
        0,
        "LYC flag should be clear after setting LYC to non-matching value (got STAT&7={:#04X})",
        stat
    );
}

#[test]
fn gambatte_m0int_m0irq_1_failure() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 2);
    gb.write_mem(0xFF41, 0x08); // Mode 0 STAT int
    gb.ints.write_if(0);

    // Wait for IRQ
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Dispatch ISR (clears IF bit 1)
    gb.ints.acknowledge_interrupt(0x02);

    let if_reg = gb.ints.read_if() & 0x02;
    // Expected 0.
    assert_eq!(
        if_reg,
        0,
        "IF STAT bit should be clear immediately after dispatch (got {:#04X})",
        gb.ints.read_if()
    );
}

#[test]
fn gambatte_m0int_m3stat_1_failure() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 2);
    gb.write_mem(0xFF41, 0x08); // Mode 0 STAT int
    gb.ints.write_if(0);

    // Wait for IRQ
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // ROM reads STAT shortly after IRQ. Expected 0x03 (Mode 3).
    // This sounds weird for a Mode 0 interrupt, but maybe it's checking re-triggering?
    // Let's check what STAT is.
    let stat = gb.read_mem(0xFF41) & 0x03;
    assert_eq!(
        stat, 3,
        "ROM expects Mode 3 shortly after Mode 0 IRQ (got Mode {})",
        stat
    );
}

#[test]
fn gambatte_m2int_m0stat_1_failure() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);
    advance_to_mode(&mut gb, 3);
    gb.write_mem(0xFF41, 0x20); // Mode 2 STAT int
    gb.ints.write_if(0);

    // Wait for IRQ
    for _ in 0..100_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // ROM reads STAT shortly after IRQ. Expected 0x00 (Mode 0).
    let stat = gb.read_mem(0xFF41) & 0x03;
    assert_eq!(
        stat, 0,
        "ROM expects Mode 0 shortly after Mode 2 IRQ (got Mode {})",
        stat
    );
}

#[test]
fn gambatte_lcdoffset1_oam_read_blocking_boundary_cgb() {
    let mut gb = setup_gb();

    // Warm up: get past the initial startup to steady-state normal lines.
    gb.write_mem(0xFF40, 0x80); // LCD ON
    advance_to_ly(&mut gb, 3);

    // Turn LCD OFF (simulates lprint_a VRAM-copy phase).
    gb.write_mem(0xFF40, 0x00);
    assert!(
        !gb.ppu.oam_read_blocked,
        "OAM must be unblocked when LCD is off"
    );

    // Turn LCD back ON — starts the startup sequence (lcdoffset1 condition).
    gb.write_mem(0xFF40, 0x80);

    // Advance to line 1, OamScan Running{tick:3}.
    // In normal timing (no lcdoffset1 fix), tick:3 on line 1 is still unblocked.
    // After the lcdoffset1 fix, the 16-tick shorter line 0 shifts line 1's
    // OamScan start 16 half-clocks earlier; tick:3 remains unblocked (the boundary
    // is still at tick:4).
    advance_to_oam_scan_tick(&mut gb, 1, 3, crate::CgbMode::Cgb, false);

    // At tick 3: OAM must NOT yet be read-blocked (preread_lcdoffset1_1 expects
    // the read to return the real OAM value = accessible = 0x00 result).
    assert!(
        !gb.ppu.oam_read_blocked,
        "oam_read_blocked must be false at OamScan tick 3 after LCD-off→on \
         (lcdoffset1 preread boundary: read must be unblocked)"
    );

    // Execute one more tick → execute tick 3's body, advance phase to tick 4.
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);

    // After tick 3 runs: still unblocked (blocking only set at tick 4).
    assert!(
        !gb.ppu.oam_read_blocked,
        "oam_read_blocked must still be false after tick 3 body runs after LCD-off→on"
    );

    // Execute tick 4 → sets oam_read_blocked = true.
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);

    // At tick 4: blocking must be engaged (same boundary as normal preread_2).
    assert!(
        gb.ppu.oam_read_blocked,
        "oam_read_blocked must be true at OamScan tick 4 after LCD-off→on"
    );
}

#[test]
fn gambatte_lcdoffset1_oam_write_blocking_boundary_cgb() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);

    // Simulate LCD-off → LCD-on (lcdoffset1 condition).
    gb.write_mem(0xFF40, 0x00);
    gb.write_mem(0xFF40, 0x80);

    // Pre-load OAM[0] so we can detect whether the write took effect.
    gb.ppu.write_oam_by_dma(0xFE00, 0x00);

    advance_to_oam_scan_tick(&mut gb, 1, 3, crate::CgbMode::Cgb, false);

    assert!(
        !gb.ppu.oam_write_blocked,
        "oam_write_blocked must be false at tick 3 after LCD-off→on (prewrite_lcdoffset1_1)"
    );

    // Write via normal (blocking-aware) path — must succeed at tick 3.
    gb.ppu.write_oam(0xFE00, 0x01);

    let raw = gb.ppu.oam().read(0);
    assert_eq!(
        raw, 0x01,
        "OAM[0] write at tick 3 after LCD-off→on must succeed (expected 0x01, got {:#04x})",
        raw
    );

    // Tick into tick 4 — write-blocking must now engage.
    gb.ppu.tick(&mut gb.ints, crate::CgbMode::Cgb, false);
    assert!(
        gb.ppu.oam_write_blocked,
        "oam_write_blocked must be true at tick 4 after LCD-off→on"
    );
}

#[test]
fn gambatte_lcdoffset1_oam_write_blocking_boundary_cgb_double_speed() {
    let mut gb = setup_gb();

    gb.write_mem(0xFF40, 0x80);
    advance_to_ly(&mut gb, 3);

    // Simulate LCD-off → LCD-on (lcdoffset1 condition).
    gb.write_mem(0xFF40, 0x00);
    gb.write_mem(0xFF40, 0x80);

    gb.ppu.write_oam_by_dma(0xFE00, 0x00);

    // Double-speed: advance to tick 3 on line 1.
    advance_to_oam_scan_tick(&mut gb, 1, 3, crate::CgbMode::Cgb, true);

    assert!(
        !gb.ppu.oam_write_blocked,
        "oam_write_blocked must be false at tick 3 after LCD-off→on (prewrite_ds_lcdoffset1_1)"
    );

    gb.ppu.write_oam(0xFE00, 0x01);
    let raw = gb.ppu.oam().read(0);
    assert_eq!(
        raw, 0x01,
        "OAM[0] write at DS tick 3 after LCD-off→on must succeed (expected 0x01, got {:#04x})",
        raw
    );
}

#[test]
fn gambatte_sprites_no_sprites_mode3_duration() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON, sprites disabled (bit 1 = 0)

    // Wait for line 2 so the first-line startup anomaly is past
    advance_to_ly(&mut gb, 2);
    // Measure Mode-3 duration on line 2
    let duration = mode3_duration_ticks(&mut gb, 2, crate::CgbMode::Dmg, false);

    // Without sprites and SCX=0, Mode-3 should be exactly 335 T-ticks (167.5 pixel-clocks)
    assert_eq!(
        duration, 335,
        "Mode-3 duration without sprites should be 335 T-ticks, got {}",
        duration
    );
}

#[test]
fn gambatte_sprites_10spritesprline_mode3_baseline() {
    let mut gb = setup_gb();
    // LCDC = 0x82: LCD on (bit 7), OBJ enable (bit 1)
    gb.write_mem(0xFF40, 0x82);

    // Place 10 sprites at X = 8, 16, …, 80, all on Y = 16 (visible on LY 0)
    // OAM entry = [Y, X, tile, attrs]
    for i in 0u8..10 {
        let base = (i as u16) * 4;
        gb.ppu.write_oam_by_dma(0xFE00 + base, 16); // Y
        gb.ppu.write_oam_by_dma(0xFE00 + base + 1, 8 + i * 8); // X = 8,16,...,80
        gb.ppu.write_oam_by_dma(0xFE00 + base + 2, 0); // tile
        gb.ppu.write_oam_by_dma(0xFE00 + base + 3, 0); // attrs
    }

    advance_to_ly(&mut gb, 1);
    let duration = mode3_duration_ticks(&mut gb, 1, crate::CgbMode::Dmg, false);

    assert!(
        duration > 172,
        "Mode-3 with 10 sprites in active range should exceed 172 ticks, got {}",
        duration
    );
}

#[test]
fn gambatte_sprites_10xposa7_no_mode3_penalty() {
    let mut gb = setup_gb();
    // LCDC = 0x82: LCD on (bit 7), OBJ enable (bit 1)
    gb.write_mem(0xFF40, 0x82);

    // Place 10 sprites all at X = 0xA7 (167), Y = 16 → visible on LY 0–7
    for i in 0u8..10 {
        let base = (i as u16) * 4;
        gb.ppu.write_oam_by_dma(0xFE00 + base, 16); // Y (sprite Y=16 → visible on LY 0)
        gb.ppu.write_oam_by_dma(0xFE00 + base + 1, 0xA7); // X = 167
        gb.ppu.write_oam_by_dma(0xFE00 + base + 2, i); // tile (distinct to avoid dedup)
        gb.ppu.write_oam_by_dma(0xFE00 + base + 3, 0); // attrs
    }

    // Use LY=2 to skip the LY=0 startup-anomaly, which adds 3 extra ticks to mode3
    // duration on the very first line after LCD-on and would mask the real comparison.
    advance_to_ly(&mut gb, 2);
    let duration = mode3_duration_ticks(&mut gb, 2, crate::CgbMode::Dmg, false);

    // With 10 sprites all at X=0xA7, each sprite fetch costs exactly 12 T-ticks
    // (SameBoy-accurate). All 10 are fetched: 344 (baseline) + 10 × 12 = 464 T-ticks.
    assert_eq!(
        duration, 464,
        "10 sprites at X=0xA7 must impose exactly 10 × 12 T-tick penalty (expected 464, got {})",
        duration
    );
}

#[test]
fn gambatte_m2int_m3stat_1() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);

    advance_to_ly(&mut gb, 66);
    advance_to_mode(&mut gb, 3);

    // Enable Mode 2 interrupt
    gb.write_mem(0xFF41, 0x20);
    gb.ints.write_if(0);

    // Wait for the interrupt to fire
    for _ in 0..10_000 {
        if gb.ints.read_if() & 0x02 != 0 {
            break;
        }
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // The interrupt has just fired (IF bit set).
    // In a real CPU, dispatch takes ~20 T-cycles (80 ticks).
    // During these 80 ticks, the PPU continues to run.
    for _ in 0..80 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Now we check what the ISR sees.
    let stat = gb.ppu.read_stat();
    // Expected: The Mode 2 interrupt handler should STILL see Mode 2?
    // Actually, Mode 2 is 160 ticks. If dispatch is 80 ticks, we are
    // only halfway through Mode 2.
    assert_eq!(stat & 0x03, 2, "ISR should see Mode 2");
}

#[test]
fn gambatte_scx_m3_extend_1() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    advance_to_ly(&mut gb, 90);
    advance_to_mode(&mut gb, 3);

    // Wait until lcd_x is ~80
    while gb.ppu.lcd_x() < 80 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // Change SCX to 7 (should extend Mode 3 by 7 dots = 14 ticks)
    gb.write_mem(0xFF43, 7);

    // Wait until what would have been the end of Mode 3 if SCX was 0.
    // Mode 3 for SCX=0 is ~172 dots = 344 ticks.
    // Plus 168 ticks OAM scan = 512 ticks.
    while gb.ppu.dots_in_line() < 512 {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    // With SCX=7, it should still be in Mode 3 at tick 512.
    let mode = gb.ppu.read_stat() & 0x03;
    assert_eq!(
        mode, 3,
        "Should still be in Mode 3 at tick 512 due to SCX=7 extension"
    );
}
