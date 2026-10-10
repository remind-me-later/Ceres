//! Integration tests using blargg's test ROMs.
//!
//! The single ROMs print their result over the serial port or leave it in the cartridge RAM; the combined ROMs
//! (which run all of them) are compared with the screenshot of the real
//! hardware. The tests that are ignored fail the same way in SameBoy, the
//! reference this emulator follows.

use ceres_core::Model;
use ceres_test_runner::{
    Run,
    checks::{BlarggCheck, TestResult},
    rom_test, run_ranked_screenshot, timeouts,
};

fn blargg(rom: &str, model: Model) -> TestResult {
    Run::new(rom, model)
        .timeout(timeouts::BLARGG)
        .check(BlarggCheck)
}

fn combined(rom: &str, screenshot: &str, model: Model) -> TestResult {
    run_ranked_screenshot(rom, screenshot, model, timeouts::BLARGG_COMBINED)
}

// The single ROMs.
rom_test!(
    blargg_cpu_instrs_individual_01_special_dmg: blargg("blargg/cpu_instrs/individual/01-special.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_01_special_cgbc: blargg("blargg/cpu_instrs/individual/01-special.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_01_special_cgbe: blargg("blargg/cpu_instrs/individual/01-special.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_02_interrupts_dmg: blargg("blargg/cpu_instrs/individual/02-interrupts.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_02_interrupts_cgbc: blargg("blargg/cpu_instrs/individual/02-interrupts.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_02_interrupts_cgbe: blargg("blargg/cpu_instrs/individual/02-interrupts.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_03_op_sp_hl_dmg: blargg("blargg/cpu_instrs/individual/03-op sp,hl.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_03_op_sp_hl_cgbc: blargg("blargg/cpu_instrs/individual/03-op sp,hl.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_03_op_sp_hl_cgbe: blargg("blargg/cpu_instrs/individual/03-op sp,hl.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_04_op_r_imm_dmg: blargg("blargg/cpu_instrs/individual/04-op r,imm.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_04_op_r_imm_cgbc: blargg("blargg/cpu_instrs/individual/04-op r,imm.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_04_op_r_imm_cgbe: blargg("blargg/cpu_instrs/individual/04-op r,imm.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_05_op_rp_dmg: blargg("blargg/cpu_instrs/individual/05-op rp.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_05_op_rp_cgbc: blargg("blargg/cpu_instrs/individual/05-op rp.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_05_op_rp_cgbe: blargg("blargg/cpu_instrs/individual/05-op rp.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_06_ld_r_r_dmg: blargg("blargg/cpu_instrs/individual/06-ld r,r.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_06_ld_r_r_cgbc: blargg("blargg/cpu_instrs/individual/06-ld r,r.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_06_ld_r_r_cgbe: blargg("blargg/cpu_instrs/individual/06-ld r,r.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_07_jr_jp_call_ret_rst_dmg: blargg("blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_07_jr_jp_call_ret_rst_cgbc: blargg("blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_07_jr_jp_call_ret_rst_cgbe: blargg("blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_08_misc_instrs_dmg: blargg("blargg/cpu_instrs/individual/08-misc instrs.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_08_misc_instrs_cgbc: blargg("blargg/cpu_instrs/individual/08-misc instrs.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_08_misc_instrs_cgbe: blargg("blargg/cpu_instrs/individual/08-misc instrs.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_09_op_r_r_dmg: blargg("blargg/cpu_instrs/individual/09-op r,r.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_09_op_r_r_cgbc: blargg("blargg/cpu_instrs/individual/09-op r,r.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_09_op_r_r_cgbe: blargg("blargg/cpu_instrs/individual/09-op r,r.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_10_bit_ops_dmg: blargg("blargg/cpu_instrs/individual/10-bit ops.gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_10_bit_ops_cgbc: blargg("blargg/cpu_instrs/individual/10-bit ops.gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_10_bit_ops_cgbe: blargg("blargg/cpu_instrs/individual/10-bit ops.gb", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_individual_11_op_a_hl_dmg: blargg("blargg/cpu_instrs/individual/11-op a,(hl).gb", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_individual_11_op_a_hl_cgbc: blargg("blargg/cpu_instrs/individual/11-op a,(hl).gb", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_individual_11_op_a_hl_cgbe: blargg("blargg/cpu_instrs/individual/11-op a,(hl).gb", Model::CgbE)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_01_read_timing_dmg: blargg("blargg/mem_timing-2/rom_singles/01-read_timing.gb", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_01_read_timing_cgbc: blargg("blargg/mem_timing-2/rom_singles/01-read_timing.gb", Model::CgbC)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_01_read_timing_cgbe: blargg("blargg/mem_timing-2/rom_singles/01-read_timing.gb", Model::CgbE)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_02_write_timing_dmg: blargg("blargg/mem_timing-2/rom_singles/02-write_timing.gb", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_02_write_timing_cgbc: blargg("blargg/mem_timing-2/rom_singles/02-write_timing.gb", Model::CgbC)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_02_write_timing_cgbe: blargg("blargg/mem_timing-2/rom_singles/02-write_timing.gb", Model::CgbE)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_03_modify_timing_dmg: blargg("blargg/mem_timing-2/rom_singles/03-modify_timing.gb", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_03_modify_timing_cgbc: blargg("blargg/mem_timing-2/rom_singles/03-modify_timing.gb", Model::CgbC)
);
rom_test!(
    blargg_mem_timing_2_rom_singles_03_modify_timing_cgbe: blargg("blargg/mem_timing-2/rom_singles/03-modify_timing.gb", Model::CgbE)
);
rom_test!(
    blargg_mem_timing_individual_01_read_timing_dmg: blargg("blargg/mem_timing/individual/01-read_timing.gb", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_individual_01_read_timing_cgbc: blargg("blargg/mem_timing/individual/01-read_timing.gb", Model::CgbC)
);
rom_test!(
    blargg_mem_timing_individual_01_read_timing_cgbe: blargg("blargg/mem_timing/individual/01-read_timing.gb", Model::CgbE)
);
rom_test!(
    blargg_mem_timing_individual_02_write_timing_dmg: blargg("blargg/mem_timing/individual/02-write_timing.gb", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_individual_02_write_timing_cgbc: blargg("blargg/mem_timing/individual/02-write_timing.gb", Model::CgbC)
);
rom_test!(
    blargg_mem_timing_individual_02_write_timing_cgbe: blargg("blargg/mem_timing/individual/02-write_timing.gb", Model::CgbE)
);
rom_test!(
    blargg_mem_timing_individual_03_modify_timing_dmg: blargg("blargg/mem_timing/individual/03-modify_timing.gb", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_individual_03_modify_timing_cgbc: blargg("blargg/mem_timing/individual/03-modify_timing.gb", Model::CgbC)
);
rom_test!(
    blargg_mem_timing_individual_03_modify_timing_cgbe: blargg("blargg/mem_timing/individual/03-modify_timing.gb", Model::CgbE)
);
rom_test!(
    blargg_dmg_sound_rom_singles_01_registers_dmg: blargg("blargg/dmg_sound/rom_singles/01-registers.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_02_len_ctr_dmg: blargg("blargg/dmg_sound/rom_singles/02-len ctr.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_03_trigger_dmg: blargg("blargg/dmg_sound/rom_singles/03-trigger.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_04_sweep_dmg: blargg("blargg/dmg_sound/rom_singles/04-sweep.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_05_sweep_details_dmg: blargg("blargg/dmg_sound/rom_singles/05-sweep details.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_06_overflow_on_trigger_dmg: blargg("blargg/dmg_sound/rom_singles/06-overflow on trigger.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_07_len_sweep_period_sync_dmg: blargg("blargg/dmg_sound/rom_singles/07-len sweep period sync.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_08_len_ctr_during_power_dmg: blargg("blargg/dmg_sound/rom_singles/08-len ctr during power.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_09_wave_read_while_on_dmg: blargg("blargg/dmg_sound/rom_singles/09-wave read while on.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_10_wave_trigger_while_on_dmg: blargg("blargg/dmg_sound/rom_singles/10-wave trigger while on.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_11_regs_after_power_dmg: blargg("blargg/dmg_sound/rom_singles/11-regs after power.gb", Model::DmgB)
);
rom_test!(
    blargg_dmg_sound_rom_singles_12_wave_write_while_on_dmg: blargg("blargg/dmg_sound/rom_singles/12-wave write while on.gb", Model::DmgB)
);
rom_test!(
    blargg_oam_bug_rom_singles_1_lcd_sync_dmg: blargg("blargg/oam_bug/rom_singles/1-lcd_sync.gb", Model::DmgB)
);
rom_test!(
    blargg_oam_bug_rom_singles_2_causes_dmg: blargg("blargg/oam_bug/rom_singles/2-causes.gb", Model::DmgB)
);
rom_test!(
    blargg_oam_bug_rom_singles_3_non_causes_dmg: blargg("blargg/oam_bug/rom_singles/3-non_causes.gb", Model::DmgB)
);
rom_test!(
    blargg_oam_bug_rom_singles_4_scanline_timing_dmg: blargg("blargg/oam_bug/rom_singles/4-scanline_timing.gb", Model::DmgB)
);
rom_test!(
    blargg_oam_bug_rom_singles_5_timing_bug_dmg: blargg("blargg/oam_bug/rom_singles/5-timing_bug.gb", Model::DmgB)
);
rom_test!(
    blargg_oam_bug_rom_singles_6_timing_no_bug_dmg: blargg("blargg/oam_bug/rom_singles/6-timing_no_bug.gb", Model::DmgB)
);
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    blargg_oam_bug_rom_singles_7_timing_effect_dmg: blargg("blargg/oam_bug/rom_singles/7-timing_effect.gb", Model::DmgB)
);
rom_test!(
    blargg_oam_bug_rom_singles_8_instr_effect_dmg: blargg("blargg/oam_bug/rom_singles/8-instr_effect.gb", Model::DmgB)
);
rom_test!(
    blargg_cgb_sound_rom_singles_01_registers_cgbc: blargg("blargg/cgb_sound/rom_singles/01-registers.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_01_registers_cgbe: blargg("blargg/cgb_sound/rom_singles/01-registers.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_02_len_ctr_cgbc: blargg("blargg/cgb_sound/rom_singles/02-len ctr.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_02_len_ctr_cgbe: blargg("blargg/cgb_sound/rom_singles/02-len ctr.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_03_trigger_cgbc: blargg("blargg/cgb_sound/rom_singles/03-trigger.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_03_trigger_cgbe: blargg("blargg/cgb_sound/rom_singles/03-trigger.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_04_sweep_cgbc: blargg("blargg/cgb_sound/rom_singles/04-sweep.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_04_sweep_cgbe: blargg("blargg/cgb_sound/rom_singles/04-sweep.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_05_sweep_details_cgbc: blargg("blargg/cgb_sound/rom_singles/05-sweep details.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_05_sweep_details_cgbe: blargg("blargg/cgb_sound/rom_singles/05-sweep details.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_06_overflow_on_trigger_cgbc: blargg("blargg/cgb_sound/rom_singles/06-overflow on trigger.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_06_overflow_on_trigger_cgbe: blargg("blargg/cgb_sound/rom_singles/06-overflow on trigger.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_07_len_sweep_period_sync_cgbc: blargg("blargg/cgb_sound/rom_singles/07-len sweep period sync.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_07_len_sweep_period_sync_cgbe: blargg("blargg/cgb_sound/rom_singles/07-len sweep period sync.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_08_len_ctr_during_power_cgbc: blargg("blargg/cgb_sound/rom_singles/08-len ctr during power.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_08_len_ctr_during_power_cgbe: blargg("blargg/cgb_sound/rom_singles/08-len ctr during power.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_09_wave_read_while_on_cgbc: blargg("blargg/cgb_sound/rom_singles/09-wave read while on.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_09_wave_read_while_on_cgbe: blargg("blargg/cgb_sound/rom_singles/09-wave read while on.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_10_wave_trigger_while_on_cgbc: blargg("blargg/cgb_sound/rom_singles/10-wave trigger while on.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_10_wave_trigger_while_on_cgbe: blargg("blargg/cgb_sound/rom_singles/10-wave trigger while on.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_11_regs_after_power_cgbc: blargg("blargg/cgb_sound/rom_singles/11-regs after power.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_11_regs_after_power_cgbe: blargg("blargg/cgb_sound/rom_singles/11-regs after power.gb", Model::CgbE)
);
rom_test!(
    blargg_cgb_sound_rom_singles_12_wave_cgbc: blargg("blargg/cgb_sound/rom_singles/12-wave.gb", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_rom_singles_12_wave_cgbe: blargg("blargg/cgb_sound/rom_singles/12-wave.gb", Model::CgbE)
);

// The combined ROMs.
rom_test!(
    blargg_cgb_sound_cgbc: combined("blargg/cgb_sound/cgb_sound.gb", "blargg/cgb_sound/cgb_sound-cgb.png", Model::CgbC)
);
rom_test!(
    blargg_cgb_sound_cgbe: combined("blargg/cgb_sound/cgb_sound.gb", "blargg/cgb_sound/cgb_sound-cgb.png", Model::CgbE)
);
rom_test!(
    blargg_cpu_instrs_dmg: combined("blargg/cpu_instrs/cpu_instrs.gb", "blargg/cpu_instrs/cpu_instrs-dmg-cgb.png", Model::DmgB)
);
rom_test!(
    blargg_cpu_instrs_cgbc: combined("blargg/cpu_instrs/cpu_instrs.gb", "blargg/cpu_instrs/cpu_instrs-dmg-cgb.png", Model::CgbC)
);
rom_test!(
    blargg_cpu_instrs_cgbe: combined("blargg/cpu_instrs/cpu_instrs.gb", "blargg/cpu_instrs/cpu_instrs-dmg-cgb.png", Model::CgbE)
);
rom_test!(
    blargg_dmg_sound_dmg: combined("blargg/dmg_sound/dmg_sound.gb", "blargg/dmg_sound/dmg_sound-dmg.png", Model::DmgB)
);
rom_test!(
    blargg_halt_bug_dmg: combined("blargg/halt_bug.gb", "blargg/halt_bug-dmg-cgb.png", Model::DmgB)
);
rom_test!(
    blargg_halt_bug_cgbc: combined("blargg/halt_bug.gb", "blargg/halt_bug-dmg-cgb.png", Model::CgbC)
);
rom_test!(
    blargg_halt_bug_cgbe: combined("blargg/halt_bug.gb", "blargg/halt_bug-dmg-cgb.png", Model::CgbE)
);
rom_test!(
    blargg_instr_timing_dmg: combined("blargg/instr_timing/instr_timing.gb", "blargg/instr_timing/instr_timing-dmg-cgb.png", Model::DmgB)
);
rom_test!(
    blargg_instr_timing_cgbc: combined("blargg/instr_timing/instr_timing.gb", "blargg/instr_timing/instr_timing-dmg-cgb.png", Model::CgbC)
);
rom_test!(
    blargg_instr_timing_cgbe: combined("blargg/instr_timing/instr_timing.gb", "blargg/instr_timing/instr_timing-dmg-cgb.png", Model::CgbE)
);
rom_test!(
    blargg_interrupt_time_cgbc: combined("blargg/interrupt_time/interrupt_time.gb", "blargg/interrupt_time/interrupt_time-cgb.png", Model::CgbC)
);
rom_test!(
    blargg_interrupt_time_cgbe: combined("blargg/interrupt_time/interrupt_time.gb", "blargg/interrupt_time/interrupt_time-cgb.png", Model::CgbE)
);
rom_test!(
    blargg_interrupt_time_dmg: combined("blargg/interrupt_time/interrupt_time.gb", "blargg/interrupt_time/interrupt_time-dmg.png", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_2_mem_timing_dmg: combined("blargg/mem_timing-2/mem_timing.gb", "blargg/mem_timing-2/mem_timing-dmg-cgb.png", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_2_mem_timing_cgbc: combined("blargg/mem_timing-2/mem_timing.gb", "blargg/mem_timing-2/mem_timing-dmg-cgb.png", Model::CgbC)
);
rom_test!(
    blargg_mem_timing_2_mem_timing_cgbe: combined("blargg/mem_timing-2/mem_timing.gb", "blargg/mem_timing-2/mem_timing-dmg-cgb.png", Model::CgbE)
);
rom_test!(
    blargg_mem_timing_dmg: combined("blargg/mem_timing/mem_timing.gb", "blargg/mem_timing/mem_timing-dmg-cgb.png", Model::DmgB)
);
rom_test!(
    blargg_mem_timing_cgbc: combined("blargg/mem_timing/mem_timing.gb", "blargg/mem_timing/mem_timing-dmg-cgb.png", Model::CgbC)
);
rom_test!(
    blargg_mem_timing_cgbe: combined("blargg/mem_timing/mem_timing.gb", "blargg/mem_timing/mem_timing-dmg-cgb.png", Model::CgbE)
);
rom_test!(
    blargg_oam_bug_cgbc: combined("blargg/oam_bug/oam_bug.gb", "blargg/oam_bug/oam_bug-cgb.png", Model::CgbC)
);
rom_test!(
    blargg_oam_bug_cgbe: combined("blargg/oam_bug/oam_bug.gb", "blargg/oam_bug/oam_bug-cgb.png", Model::CgbE)
);
rom_test!(
    #[ignore = "fails the same way in SameBoy"]
    blargg_oam_bug_dmg: combined("blargg/oam_bug/oam_bug.gb", "blargg/oam_bug/oam_bug-dmg.png", Model::DmgB)
);
