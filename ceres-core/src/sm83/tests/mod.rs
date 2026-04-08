mod arithmetic;
mod gambatte;
mod interrupts;
mod speed_switch;
mod timing;

use super::*;
use crate::CgbMode;
use crate::test_util::setup_gb;
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

    println!(
        "DEBUG: elapsed_int={}, elapsed_no_int={}",
        elapsed_int, elapsed_no_int
    );

    // The net ISR overhead is the difference.
    elapsed_int - elapsed_no_int
}

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

fn setup_dmg_with_rom(patches: &[(usize, u8)]) -> Gb {
    setup_model_with_rom(crate::Model::DmgB, patches)
}

fn setup_cgb_with_rom(patches: &[(usize, u8)]) -> Gb {
    let mut gb = setup_model_with_rom(crate::Model::CgbE, patches);
    gb.write_mem(0xFF40, 0x00);
    gb
}

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
