use super::*;

#[test]
fn test_irq_ds_1() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1024);
    assert_eq!(gb.ints.read_if() & 0x04, 0, "IRQ should fire after reload");
    gb.advance_dots(4);
    assert_eq!(gb.ints.read_if() & 0x04, 0x04);
}

#[test]
fn test_irq_ds_timing_boundary() {
    let mut gb = setup_cgb();
    gb.write_mem(0xFF04, 0);
    gb.write_mem(0xFF06, 0x00);
    gb.write_mem(0xFF05, 0xFF);
    gb.write_mem(0xFF07, 0x04);

    gb.advance_dots(1024); // T=1024: Reloading starts
    assert_eq!(gb.ints.read_if() & 0x04, 0);
    gb.advance_dots(3); // T=1027: Still Reloading
    assert_eq!(gb.ints.read_if() & 0x04, 0);
    gb.advance_dots(1); // T=1028: Transition to Reloaded, IRQ should fire
    assert_eq!(gb.ints.read_if() & 0x04, 0x04);
}

#[test]
fn test_repro_speedchange_double_to_normal_dots() {
    let mut gb = setup_gb();
    gb.change_model_and_soft_reset(Model::CgbE);

    // 1. Enter double speed
    let addr = 0xC000;
    gb.set_cpu_pc(addr);
    gb.write_mem(addr, 0x10); // STOP
    gb.write_mem(addr + 1, 0x00);
    gb.write_mem(0xFF4D, 0x01);
    gb.run_cpu();
    assert!(gb.key1.is_enabled());

    // 2. Request speed change (Double -> Normal)
    gb.set_cpu_pc(addr);
    gb.write_mem(0xFF4D, 0x01);

    let start_dots = gb.total_dots();
    gb.run_cpu();
    let end_dots = gb.total_dots();

    let elapsed = end_dots - start_dots;
    // STOP takes 32768 M-cycles.
    // In double speed, 1 M-cycle = 2 dots.
    // FETCH (2 cycles) = 4 dots.
    // STOP (1 cycle) = 2 dots.
    // DELAY (32768 cycles) = 65536 dots.
    // Total: 4 + 2 + 65536 = 65542 dots.
    assert_eq!(elapsed, 65542);
}
