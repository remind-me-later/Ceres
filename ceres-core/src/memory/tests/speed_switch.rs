use super::*;

fn do_speed_switch(gb: &mut Gb) {
    // Arm the speed switch (bit 0 of KEY1).
    gb.write_mem(0xFF4D, 0x01);
    // Write STOP opcode + mandatory padding byte into WRAM.
    gb.write_mem(0xC000, 0x10); // opcode: STOP
    gb.write_mem(0xC001, 0x00); // padding byte consumed by STOP
    // Point CPU at the STOP instruction and execute it.
    gb.set_cpu_pc(0xC000);
    gb.run_cpu();
}

#[test]
fn key1_armed_reads_7f() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF4D, 0x01);
    assert_eq!(
        gb.read_mem(0xFF4D),
        0x7F,
        "KEY1 after arming: should read 0x7F (bit7=0=single, bit0=1=armed, bits6:1=1)"
    );
}

#[test]
fn key1_armed_then_disarmed_reads_7e() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF4D, 0xFF); // arm (only bit 0 is writable)
    gb.write_mem(0xFF4D, 0x00); // disarm
    assert_eq!(
        gb.read_mem(0xFF4D),
        0x7E,
        "KEY1 after arm+disarm: should read 0x7E (bit7=0, bit0=0, bits6:1=1)"
    );
}

#[test]
fn speedchange_key1_reads_fe_after_single_switch() {
    let mut gb = setup_cgb();
    do_speed_switch(&mut gb);
    assert_eq!(
        gb.read_mem(0xFF4D),
        0xFE,
        "KEY1 after one speed switch: should read 0xFE (bit7=1=double, bit0=0=disarmed)"
    );
}

#[test]
fn speedchange_div_resets_to_zero_after_switch() {
    let mut gb = setup_cgb();
    // Advance DIV so we can confirm it truly reset (not just happened to be 0).
    gb.advance_dots(1024);
    assert_ne!(
        gb.read_mem(0xFF04),
        0x00,
        "pre-condition: DIV should not be 0 before switch"
    );
    do_speed_switch(&mut gb);
    assert_eq!(
        gb.read_mem(0xFF04),
        0x00,
        "DIV must be 0x00 immediately after speed switch"
    );
}

#[test]
fn speedchange2_key1_reads_7e_after_double_switch() {
    let mut gb = setup_cgb();
    do_speed_switch(&mut gb); // single → double
    do_speed_switch(&mut gb); // double → single
    assert_eq!(
        gb.read_mem(0xFF4D),
        0x7E,
        "KEY1 after two switches: should read 0x7E (bit7=0=single, bit0=0=disarmed)"
    );
}

#[test]
fn speedchange2_div_resets_to_zero_after_double_switch() {
    let mut gb = setup_cgb();
    do_speed_switch(&mut gb); // first switch
    gb.advance_dots(1024); // let DIV count up
    do_speed_switch(&mut gb); // second switch resets DIV again
    assert_eq!(
        gb.read_mem(0xFF04),
        0x00,
        "DIV must be 0x00 immediately after second speed switch"
    );
}
