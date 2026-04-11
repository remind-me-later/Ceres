use super::*;

#[test]
fn diagnostic_intr_2_0_gap() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON

    // 1. Wait for Mode 2 IRQ on line 40
    advance_to_ly(&mut gb, 40);
    // Mode 2 IRQ fires 4 ticks before HBlank ends.
    // In Ceres, tick_oam_scan(tick=0) is when mode_for_interrupt becomes OamScan.
    // That happens at dot 0 of line 40.

    let mut mode2_tick = 0;
    let mut mode0_tick = 0;
    let mut total_ticks = 0;

    // Simplified trace: find ticks between mode_for_interrupt changes
    for t in 0..2000 {
        let stat = gb.ppu.read_stat();
        let mode = stat & 0x03;

        // We look for when the STAT interrupt would fire (if enabled)
        // For Mode 2, it's dot 0 of the line.
        // For Mode 0, it's 8 ticks after Mode 3 ends.

        if mode == 2 && mode2_tick == 0 {
            mode2_tick = t;
            println!("Mode 2 detected at absolute tick {}", t);
        }

        if mode == 0 && mode2_tick != 0 && mode0_tick == 0 {
            mode0_tick = t;
            println!("Mode 0 detected at absolute tick {}", t);
            total_ticks = mode0_tick - mode2_tick;
            break;
        }

        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    println!(
        "Measured Gap: {} ticks ({} dots)",
        total_ticks,
        total_ticks / 2
    );
    // Mooneye expects 172 dots = 344 ticks.
}

#[test]
fn diagnostic_lyc0_startup_trace() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80); // LCD ON
    // 1. Setup like lyc0int_m0irq_1
    // Wait until LY=151 (VBlank)
    advance_to_ly(&mut gb, 151);

    // Enable interrupts
    gb.write_mem(0xFF41, 0x48); // Mode 0 + LYC interrupts
    gb.write_mem(0xFFFF, 0x02); // IE = STAT
    gb.write_mem(0xFF0F, 0x00); // IF = 0

    println!("--- Diagnostic: LYC=0 Startup Trace ---");
    // Write LYC=0
    gb.write_mem(0xFF45, 0);

    // Trace next 2000 ticks
    for t in 0..2000 {
        let ly = gb.ppu.read_ly();
        let stat = gb.ppu.read_stat();
        let ifr = gb.read_mem(0xFF0F);
        let dots = gb.ppu.dots_in_line();

        if t % 100 == 0 {
            println!(
                "Tick {}: LY={}, Dots={}, STAT={:02X}, IF={:02X}",
                t, ly, dots, stat, ifr
            );
        }

        if (ifr & 0x02) != 0 {
            println!(
                "IRQ FIRED at Tick {}: LY={}, Dots={}, STAT={:02X}, IF={:02X}",
                t, ly, dots, stat, ifr
            );
            // Clear IF to see if it re-fires
            gb.write_mem(0xFF0F, 0);
        }

        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    println!("---------------------------------------");
}

#[test]
fn diagnostic_oam_blocking_trace() {
    let mut gb = setup_gb();
    gb.write_mem(0xFF40, 0x80);

    // Advance to end of line 40 Mode 3
    advance_to_ly(&mut gb, 40);
    advance_to_mode(&mut gb, 3);

    // Wait for Mode 3 Running stage to end
    while matches!(
        gb.ppu.phase,
        crate::ppu::PpuPhase::Drawing(crate::ppu::DrawingStage::Running)
    ) {
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }

    println!("--- Diagnostic: OAM Unblocking Trace (Mode 3 -> Mode 0) ---");
    for t in 0..20 {
        let mode = gb.ppu.read_stat() & 0x03;

        // Attempt OAM write
        gb.ppu.write_oam(0xFE00, 0x55);
        let val = gb.ppu.read_oam(0xFE00);
        let blocked = val != 0x55;
        if !blocked {
            gb.ppu.write_oam(0xFE00, 0);
        }

        println!("Tick {:2}: Mode={}, OAM Blocked={}", t, mode, blocked);
        gb.ppu.tick(&mut gb.ints, crate::CgbMode::Dmg, false);
    }
    println!("----------------------------------------------------------");
}
