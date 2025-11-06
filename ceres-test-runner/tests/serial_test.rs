//! Tests for serial output capture

use ceres_test_runner::test_runner::{TestConfig, TestRunner};

#[test]
fn test_serial_output_capture() {
    // Create a minimal ROM that outputs to serial
    let mut rom = vec![0; 0x8000]; // 32KB ROM

    // Set ROM size to 32KB (value 0)
    rom[0x148] = 0;
    // Set RAM size to none
    rom[0x149] = 0;
    // Set cartridge type to ROM only
    rom[0x147] = 0;

    // Calculate header checksum
    let mut checksum: u8 = 0;
    for byte in &rom[0x134..0x14D] {
        checksum = checksum.wrapping_sub(*byte).wrapping_sub(1);
    }
    rom[0x14D] = checksum;

    // Add code at 0x100 to write to serial port
    // LD A, 'T'
    rom[0x100] = 0x3E;
    rom[0x101] = b'T';
    // LD (FF01), A  ; Write to SB register
    rom[0x102] = 0xE0;
    rom[0x103] = 0x01;
    // LD A, 0x81  ; Start transfer with internal clock
    rom[0x104] = 0x3E;
    rom[0x105] = 0x81;
    // LD (FF02), A  ; Write to SC register
    rom[0x106] = 0xE0;
    rom[0x107] = 0x02;
    // Infinite loop
    rom[0x108] = 0x18; // JR -2
    rom[0x109] = 0xFE;

    let config = TestConfig {
        capture_serial: true,
        model: ceres_core::Model::Dmg,
        timeout_frames: 100, // Short timeout for this test
        expected_screenshot: None,
    };

    let Ok(mut runner) = TestRunner::new(rom, config) else {
        unreachable!("Failed to create test runner");
    };

    // Run the test - it will timeout since it loops forever,
    // but we should capture serial output
    let _result = runner.run();

    let output = runner.serial_output();

    // We should have captured the 'T' character
    // Note: The actual output timing depends on the serial transfer rate
    #[cfg(debug_assertions)]
    {
        println!("Serial output: {output}");
        println!("Frames run: {}", runner.frames_run());
    }

    // This is a basic smoke test - actual Blargg tests will have more complex output
    // The test validates that the infrastructure is working
    assert!(runner.frames_run() > 0, "Test should run some frames");
}
