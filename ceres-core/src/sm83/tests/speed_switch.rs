use super::*;

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
    // plus the STOP instruction fetch/execute (4 + 4 = 8 dots)
    // plus the next instruction fetch (4 dots)
    // Total should be 131084 dots, counting the fetches.
    let elapsed = end_dots - start_dots;
    assert_eq!(
        elapsed, 131084,
        "Speed change should take 131084 dots (fetches + switch)"
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

    // Speed change from double to normal speed takes 32768 M-cycles (65536 dots)
    // plus the STOP instruction fetch/execute (2 + 2 = 4 dots)
    // plus the next instruction fetch (2 dots).
    // Total should be 65542 dots, counting the fetches.
    let elapsed = end_dots - start_dots;
    assert_eq!(
        elapsed, 65542,
        "Speed change from double to normal should take 65542 normal dots (fetches + switch)"
    );

    assert!(!gb.key1.is_enabled());
    assert_eq!(
        gb.read_mem(0xFF05),
        128,
        "TIMA should increment 128 times during speed switch (Double -> Normal)"
    );
}
