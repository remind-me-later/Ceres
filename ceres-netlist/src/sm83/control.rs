//! The SM83's second and third decoders (`hdl/rtl/sm83_control.vhd`): from
//! the first decoder's steps and the opcode's bits, the core's control
//! lines: the memory request, read and write, the next M-cycle, which
//! register goes onto which bus, what the ALU and the IDU do, which
//! registers and flags are written. The names are the netlist's, without
//! its prefix `ctl_` or `ctl_op_`; the decoders are precharged, so that a
//! line is on in `exec_phase` (B to H) only, or, for the writes, in
//! `data_phase` (E to H).
//!
//! @hw die: dmg-schematics, categories dec2 (cell decoder2) and dec3 (cell
//! decoder3), and the gates of category reg (REG_AND2_AND3, REG_OR) and of
//! dec3 (DEC3_NOR1, DEC3_NOR2)

use super::decoder::Steps;

/// The control lines.
#[derive(Default)]
#[expect(clippy::struct_excessive_bools, reason = "the decoders' lines")]
pub struct Controls {
    pub mreq: bool,
    pub mread: bool,
    pub mwrite: bool,
    pub fetch: bool,
    pub reg_gp_we_mask: bool,
    pub reg_pc_to_idu_en: bool,
    pub reg_pch_to_db_en: bool,
    pub reg_pcl_to_db_en: bool,
    pub reg_wz_to_reg_pc_en: bool,
    pub idu_inc: bool,
    pub idu_dec: bool,
    pub push_or_pop_exec: bool,
    pub any_cc: bool,
    pub ld_nn_sp_exec: bool,
    pub ld_nn_sp_or_ldx_exec: bool,
    pub ldx_nn_a_or_ld_xxx_a_exec: bool,
    pub ldx_a_nn_or_ld_a_xxx_exec: bool,
    pub ld_xxx_a_or_ld_a_xxx_m0: bool,
    pub ld_n_a_or_ld_c_a_exec: bool,
    pub ld_n_a_or_ld_a_n_m1: bool,
    pub ld_c_a_or_ld_a_c_m0: bool,
    pub inc_or_dec_ss_m0: bool,
    pub add_sp_e_or_ldhl_sp_e_m1: bool,
    pub add_sp_e_or_ldhl_sp_e_m2: bool,
    pub set_exec: bool,
    pub res_exec: bool,
    pub any_ind_hl_or_ld_sp_hl_or_jp_hl_exec: bool,
    pub add_a_x_or_adc_a_x: bool,
    pub sub_or_sbc: bool,
    pub cp: bool,
    pub sub_or_sbc_or_cp: bool,
    pub add_or_adc_or_ldhl_exec: bool,
    pub or_or_set_exec: bool,
    pub rxx: bool,
    pub sxx_or_swap: bool,
    pub rlx_or_slx: bool,
    pub rrx_or_srx: bool,
    pub rlc: bool,
    pub rl: bool,
    pub rrc: bool,
    pub rr: bool,
    pub sra_op: bool,
    pub swap: bool,
    pub daa_m0: bool,
    pub cpl_m0: bool,
    pub scf_or_ccf_m0: bool,
    pub rxxa_or_daa_or_cpl_m0: bool,
    pub reti_m3: bool,
    pub add_sp_e_m1_buf: bool,
    pub add_sp_e_m2_buf: bool,
    pub ldhl_sp_e_m1_buf: bool,
    pub ldhl_sp_e_m2_buf: bool,
    pub inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec: bool,
    pub inc_or_dec_m_or_ld_or_halt: bool,
    pub alu_or_alu_cb_or_ld_or_halt: bool,
    pub cb_rxx_or_sxx_or_swap_or_set_or_res_exec: bool,
    pub alu_add: bool,
    pub alu_or: bool,
    pub alu_xor: bool,
    pub alu_and: bool,
    pub alu_inv_op2: bool,
    pub alu_inv_cin: bool,
    pub flags_c_we: bool,
    pub flags_znh_we: bool,
    pub reg_a_to_op2_en: bool,
    pub reg_b_to_op2_en: bool,
    pub reg_c_to_op2_en: bool,
    pub reg_d_to_op2_en: bool,
    pub reg_e_to_op2_en: bool,
    pub reg_h_to_op2_en: bool,
    pub reg_l_to_op2_en: bool,
    pub reg_z_to_op2_en: bool,
    pub ff_to_op1_en: bool,
    pub reg_bc_to_idu_en: bool,
    pub reg_de_to_idu_en: bool,
    pub reg_hl_to_idu_en: bool,
    pub reg_sp_to_idu_en: bool,
    pub op2_to_db_en: bool,
    pub alu_to_db_en: bool,
    pub reg_a_we: bool,
    pub reg_b_we: bool,
    pub reg_c_we: bool,
    pub reg_d_we: bool,
    pub reg_e_we: bool,
    pub reg_h_we: bool,
    pub reg_l_we: bool,
    pub dec3_y56: bool,
    pub dec3_y58: bool,
    pub reg_wz_to_reg_gp_en: bool,
    pub reg_w_we: bool,
    pub reg_z_we: bool,
    pub reg_sp_we: bool,
    pub idu_to_reg_sp_en: bool,
    pub reg_wz_to_reg_sp_en: bool,
    pub reg_pc_we: bool,
    pub idu_to_reg_pc_en: bool,
    /// The next M-cycle.
    pub next_mcyc: u8,
    pub ff_to_op2_en: bool,
    pub flags_to_op2_en: bool,
    pub ld_nn_sp_or_jr_exec: bool,
    pub idu_to_reg_gp_en: bool,
    pub alu_to_reg_gp_en: bool,
    pub stop: bool,
}

/// What the decoders take in besides the steps: the opcode, the CB prefix,
/// the phases, which of W and Z a read goes to, and Z's sign from the
/// M-cycle before.
#[derive(Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "the decoders' inputs")]
pub struct ControlInputs {
    pub op: u8,
    pub table_cb: bool,
    pub exec_phase: bool,
    pub data_phase: bool,
    pub write_phase: bool,
    pub sel_reg_w: bool,
    pub sel_reg_z: bool,
    pub z7_delayed: bool,
}

/// The control lines for the steps `s`.
#[must_use]
#[expect(clippy::too_many_lines, reason = "a line of the decoders a line")]
#[expect(
    clippy::field_reassign_with_default,
    reason = "the lines in the decoders' order"
)]
pub fn control(s: &Steps, i: ControlInputs) -> Controls {
    let o = |bit: u32| i.op >> bit & 1 != 0;
    let (exec, data, write) = (i.exec_phase, i.data_phase, i.write_phase);
    let mut c = Controls::default();

    // Memory and the M-cycles (decoder 2): the request, the read, the
    // write, the opcode's fetch (the last M-cycle of an instruction), the
    // next M-cycle, and whether the general registers may be written
    c.mreq = s.jr_m2
        | s.ld_a_xxx_m1
        | s.aux_alu_m0
        | s.push_qq_m1
        | s.add_hl_ss_m1
        | s.push_qq_m2
        | s.pop_qq_m2
        | s.ld_nn_sp_m2
        | s.add_sp_e_m3
        | s.ld_nn_sp_m3
        | s.jp_hl
        | s.ld_dd_nn_m2
        | s.rst_t_m2
        | s.int_entry_m6
        | s.di_or_ei
        | s.halt
        | s.nop_or_stop
        | s.cb_prefix
        | s.ldx_nn_a_m2
        | s.ldx_a_nn_m2
        | s.ldx_a_nn_m3
        | s.call_m0
        | s.call_m1
        | s.call_m3
        | s.call_m4
        | s.ld_x_n_m0
        | s.ld_r_n_m2
        | s.m6
        | s.m7
        | s.jr_m0
        | s.ldhl_sp_e_m2
        | s.res_b_r_m0
        | s.res_b_hl_m1
        | s.ld_xxx_a_m0
        | s.ld_a_xxx_m0
        | s.ld_c_a_m0
        | s.ld_n_a_m0
        | s.ld_n_a_m1
        | s.ld_r_hl_m0
        | s.ld_r_r_m0
        | s.jp_m0
        | s.jp_m1
        | s.ld_hl_n_m1
        | s.pop_qq_m0
        | s.pop_qq_m1
        | s.set_b_r_m0
        | s.set_b_hl_m1
        | s.res_or_set_b_hl_m0
        | s.ld_a_n_m1
        | s.ld_nn_sp_m0
        | s.add_sp_e_m0
        | s.ldhl_sp_e_m0
        | s.ld_nn_sp_m1
        | s.ld_hl_r_m0
        | s.inc_or_dec_hl_m0
        | s.inc_or_dec_hl_m1
        | s.ld_a_c_m0
        | s.ld_a_n_m0
        | s.rst_t_m1
        | s.int_entry_m5
        | s.alu_a_r
        | s.ret_or_reti_m0
        | s.ret_cc_m1
        | s.retx_m2
        | s.ld_dd_nn_m0
        | s.ld_dd_nn_m1
        | s.inc_or_dec_r
        | s.alu_a_hl_m0
        | s.alu_a_n_m0
        | s.cb_r
        | s.cb_hl_m0
        | s.bit_b_hl_m1
        | s.cb_rxx_or_sxx_or_swap_or_res_m1
        | s.ldx_m0
        | s.ldx_m1;
    c.mread = s.ldx_a_nn_m2
        | s.call_m0
        | s.call_m1
        | s.ld_x_n_m0
        | s.jr_m0
        | s.ld_a_xxx_m0
        | s.ld_n_a_m0
        | s.ld_r_hl_m0
        | s.jp_m0
        | s.jp_m1
        | s.pop_qq_m0
        | s.pop_qq_m1
        | s.res_or_set_b_hl_m0
        | s.ld_a_n_m1
        | s.ld_nn_sp_m0
        | s.add_sp_e_m0
        | s.ldhl_sp_e_m0
        | s.ld_nn_sp_m1
        | s.inc_or_dec_hl_m0
        | s.ld_a_c_m0
        | s.ld_a_n_m0
        | s.ret_or_reti_m0
        | s.ret_cc_m1
        | s.retx_m2
        | s.ld_dd_nn_m0
        | s.ld_dd_nn_m1
        | s.alu_a_hl_m0
        | s.alu_a_n_m0
        | s.cb_hl_m0
        | s.ldx_m0
        | s.ldx_m1;
    c.mwrite = s.push_qq_m1
        | s.push_qq_m2
        | s.ld_nn_sp_m2
        | s.ld_nn_sp_m3
        | s.rst_t_m2
        | s.int_entry_m6
        | s.ldx_nn_a_m2
        | s.call_m3
        | s.call_m4
        | s.res_b_hl_m1
        | s.ld_xxx_a_m0
        | s.ld_c_a_m0
        | s.ld_n_a_m1
        | s.ld_hl_n_m1
        | s.set_b_hl_m1
        | s.ld_hl_r_m0
        | s.inc_or_dec_hl_m1
        | s.rst_t_m1
        | s.int_entry_m5
        | s.cb_rxx_or_sxx_or_swap_or_res_m1;
    c.fetch = s.jr_m2
        | s.ld_a_xxx_m1
        | s.aux_alu_m0
        | s.add_hl_ss_m1
        | s.pop_qq_m2
        | s.add_sp_e_m3
        | s.jp_hl
        | s.ld_dd_nn_m2
        | s.di_or_ei
        | s.halt
        | s.nop_or_stop
        | s.cb_prefix
        | s.ldx_a_nn_m3
        | s.ld_r_n_m2
        | s.m6
        | s.m7
        | s.ldhl_sp_e_m2
        | s.res_b_r_m0
        | s.ld_r_r_m0
        | s.set_b_r_m0
        | s.alu_a_r
        | s.inc_or_dec_r
        | s.cb_r
        | s.bit_b_hl_m1;
    let next_0 = s.add_hl_ss_m0
        | s.push_qq_m2
        | s.ld_nn_sp_m2
        | s.ld_sp_hl_m0
        | s.ld_nn_sp_m3
        | s.retx_m3
        | s.rst_t_m2
        | s.int_entry_m6
        | s.ldx_nn_a_m2
        | s.ldx_a_nn_m2
        | s.ret_cc_m0
        | s.call_m0
        | s.call_m2
        | s.call_m4
        | s.ld_x_n_m0
        | s.jr_m0
        | s.add_sp_e_m2
        | s.res_b_hl_m1
        | s.ld_xxx_a_m0
        | s.ld_a_xxx_m0
        | s.ld_c_a_m0
        | s.ld_n_a_m0
        | s.ld_n_a_m1
        | s.dec_ss_m0
        | s.inc_ss_m0
        | s.push_qq_m0
        | s.jp_m0
        | s.jp_m2
        | s.ld_hl_n_m1
        | s.pop_qq_m0
        | s.set_b_hl_m1
        | s.res_or_set_b_hl_m0
        | s.ld_nn_sp_m0
        | s.add_sp_e_m0
        | s.ldhl_sp_e_m0
        | s.ld_hl_r_m0
        | s.inc_or_dec_hl_m0
        | s.inc_or_dec_hl_m1
        | s.ld_a_n_m0
        | s.rst_t_m0
        | s.int_entry_m4
        | s.retx_m2
        | s.ld_dd_nn_m0
        | s.cb_hl_m0
        | s.cb_rxx_or_sxx_or_swap_or_res_m1
        | s.ldx_m0;
    let next_1 = s.jr_m1
        | s.push_qq_m1
        | s.push_qq_m2
        | s.ld_nn_sp_m2
        | s.ld_sp_hl_m0
        | s.ld_nn_sp_m3
        | s.retx_m3
        | s.rst_t_m2
        | s.int_entry_m6
        | s.ldx_nn_a_m2
        | s.ldx_a_nn_m2
        | s.call_m1
        | s.call_m2
        | s.call_m4
        | s.add_sp_e_m2
        | s.res_b_hl_m1
        | s.ld_xxx_a_m0
        | s.ld_c_a_m0
        | s.ld_n_a_m1
        | s.ld_r_hl_m0
        | s.dec_ss_m0
        | s.inc_ss_m0
        | s.jp_m1
        | s.jp_m2
        | s.ld_hl_n_m1
        | s.pop_qq_m1
        | s.add_sp_e_m1
        | s.ldhl_sp_e_m1
        | s.set_b_hl_m1
        | s.ld_a_n_m1
        | s.ld_nn_sp_m1
        | s.ld_hl_r_m0
        | s.inc_or_dec_hl_m1
        | s.ld_a_c_m0
        | s.rst_t_m1
        | s.int_entry_m5
        | s.ret_or_reti_m0
        | s.ret_cc_m1
        | s.retx_m2
        | s.ld_dd_nn_m1
        | s.alu_a_hl_m0
        | s.alu_a_n_m0
        | s.cb_rxx_or_sxx_or_swap_or_res_m1
        | s.ldx_m1;
    let next_2 = s.push_qq_m2
        | s.ld_sp_hl_m0
        | s.ld_nn_sp_m3
        | s.retx_m3
        | s.rst_t_m2
        | s.int_entry_m6
        | s.ldx_nn_a_m2
        | s.call_m3
        | s.call_m4
        | s.res_b_hl_m1
        | s.ld_xxx_a_m0
        | s.ld_c_a_m0
        | s.ld_n_a_m1
        | s.ld_r_hl_m0
        | s.dec_ss_m0
        | s.inc_ss_m0
        | s.jp_m2
        | s.ld_hl_n_m1
        | s.set_b_hl_m1
        | s.ld_a_n_m1
        | s.ld_hl_r_m0
        | s.inc_or_dec_hl_m1
        | s.ld_a_c_m0
        | s.int_entry_m5
        | s.int_entry_m4
        | s.int_entry_m0
        | s.alu_a_hl_m0
        | s.alu_a_n_m0
        | s.cb_rxx_or_sxx_or_swap_or_res_m1;
    c.next_mcyc = u8::from(next_0) | u8::from(next_1) << 1 | u8::from(next_2) << 2;
    c.reg_gp_we_mask = s.ld_a_xxx_m1
        | s.aux_alu_m0
        | s.add_hl_ss_m0
        | s.push_qq_m1
        | s.add_hl_ss_m1
        | s.push_qq_m2
        | s.pop_qq_m2
        | s.ld_sp_hl_m0
        | s.jp_hl
        | s.ld_dd_nn_m2
        | s.rst_t_m2
        | s.int_entry_m6
        | s.di_or_ei
        | s.halt
        | s.nop_or_stop
        | s.ldx_a_nn_m3
        | s.call_m4
        | s.ld_r_n_m2
        | s.m6
        | s.ldhl_sp_e_m2
        | s.res_b_r_m0
        | s.ld_xxx_a_m0
        | s.ld_a_xxx_m0
        | s.dec_ss_m0
        | s.inc_ss_m0
        | s.ld_r_r_m0
        | s.jp_m2
        | s.add_sp_e_m1
        | s.ldhl_sp_e_m1
        | s.set_b_r_m0
        | s.ld_hl_r_m0
        | s.inc_or_dec_hl_m1
        | s.alu_a_r
        | s.inc_or_dec_r
        | s.cb_r
        | s.bit_b_hl_m1
        | s.cb_rxx_or_sxx_or_swap_or_res_m1;

    // The program counter and the IDU (decoder 2)
    c.reg_pc_to_idu_en = s.ld_a_xxx_m1
        | s.aux_alu_m0
        | s.add_hl_ss_m1
        | s.pop_qq_m2
        | s.add_sp_e_m3
        | s.ld_dd_nn_m2
        | s.di_or_ei
        | s.halt
        | s.nop_or_stop
        | s.cb_prefix
        | s.ldx_a_nn_m3
        | s.call_m0
        | s.call_m1
        | s.ld_x_n_m0
        | s.ld_r_n_m2
        | s.m6
        | s.m7
        | s.jr_m0
        | s.ldhl_sp_e_m2
        | s.res_b_r_m0
        | s.ld_n_a_m0
        | s.ld_r_r_m0
        | s.jp_m0
        | s.jp_m1
        | s.set_b_r_m0
        | s.ld_nn_sp_m0
        | s.add_sp_e_m0
        | s.ldhl_sp_e_m0
        | s.ld_nn_sp_m1
        | s.ld_a_n_m0
        | s.int_entry_m0
        | s.alu_a_r
        | s.ld_dd_nn_m0
        | s.ld_dd_nn_m1
        | s.inc_or_dec_r
        | s.alu_a_n_m0
        | s.cb_r
        | s.bit_b_hl_m1
        | s.ldx_m0
        | s.ldx_m1;
    c.reg_pch_to_db_en = s.call_m3 | s.rst_t_m1 | s.int_entry_m5;
    c.reg_pcl_to_db_en = s.rst_t_m2 | s.int_entry_m6 | s.call_m4;
    c.reg_wz_to_reg_pc_en = s.retx_m3 | s.call_m4 | s.jp_m2;
    c.idu_inc = s.jr_m2
        | s.ld_a_xxx_m1
        | s.aux_alu_m0
        | s.add_hl_ss_m1
        | s.pop_qq_m2
        | s.ld_nn_sp_m2
        | s.add_sp_e_m3
        | s.jp_hl
        | s.ld_dd_nn_m2
        | s.di_or_ei
        | s.nop_or_stop
        | s.cb_prefix
        | s.ldx_a_nn_m3
        | s.call_m0
        | s.call_m1
        | s.ld_x_n_m0
        | s.ld_r_n_m2
        | s.m6
        | s.m7
        | s.jr_m0
        | s.ldhl_sp_e_m2
        | s.res_b_r_m0
        | s.ld_n_a_m0
        | s.inc_ss_m0
        | s.ld_r_r_m0
        | s.jp_m0
        | s.jp_m1
        | s.pop_qq_m0
        | s.pop_qq_m1
        | s.set_b_r_m0
        | s.ld_nn_sp_m0
        | s.add_sp_e_m0
        | s.ldhl_sp_e_m0
        | s.ld_nn_sp_m1
        | s.ld_a_n_m0
        | s.alu_a_r
        | s.ret_or_reti_m0
        | s.ret_cc_m1
        | s.retx_m2
        | s.ld_hli_m0
        | s.ld_dd_nn_m0
        | s.ld_dd_nn_m1
        | s.inc_or_dec_r
        | s.alu_a_n_m0
        | s.cb_r
        | s.bit_b_hl_m1
        | s.ldx_m0
        | s.ldx_m1;
    c.idu_dec = s.push_qq_m1
        | s.call_m2
        | s.call_m3
        | s.dec_ss_m0
        | s.push_qq_m0
        | s.rst_t_m1
        | s.rst_t_m0
        | s.int_entry_m5
        | s.int_entry_m4
        | s.int_entry_m0
        | s.ld_hld_m0;
    c.push_or_pop_exec = s.push_qq_m1
        | s.push_qq_m2
        | s.rst_t_m2
        | s.int_entry_m6
        | s.call_m2
        | s.call_m3
        | s.call_m4
        | s.push_qq_m0
        | s.pop_qq_m0
        | s.pop_qq_m1
        | s.rst_t_m1
        | s.rst_t_m0
        | s.int_entry_m5
        | s.int_entry_m4
        | s.ret_or_reti_m0
        | s.ret_cc_m1
        | s.retx_m2;

    // Steps of several instructions together (decoder 2)
    c.any_cc = s.jp_cc_nn_m1 | s.call_cc_nn_m1 | s.ret_cc_m0 | s.jr_cc_e_m0;
    c.ld_nn_sp_exec = s.ld_nn_sp_m2 | s.ld_nn_sp_m3;
    c.ld_nn_sp_or_ldx_exec = c.ld_nn_sp_exec | s.ldx_nn_a_m2 | s.ldx_a_nn_m2;
    c.ldx_nn_a_or_ld_xxx_a_exec = s.ldx_nn_a_m2 | s.ld_xxx_a_m0;
    c.ldx_a_nn_or_ld_a_xxx_exec = s.ld_a_xxx_m1 | s.ldx_a_nn_m3;
    c.ld_xxx_a_or_ld_a_xxx_m0 = s.ld_xxx_a_m0 | s.ld_a_xxx_m0;
    c.ld_n_a_or_ld_c_a_exec = s.ld_c_a_m0 | s.ld_n_a_m1;
    c.ld_n_a_or_ld_a_n_m1 = s.ld_n_a_m1 | s.ld_a_n_m1;
    c.ld_c_a_or_ld_a_c_m0 = s.ld_c_a_m0 | s.ld_a_c_m0;
    c.inc_or_dec_ss_m0 = s.dec_ss_m0 | s.inc_ss_m0;
    c.add_sp_e_or_ldhl_sp_e_m1 = s.add_sp_e_m1 | s.ldhl_sp_e_m1;
    c.add_sp_e_or_ldhl_sp_e_m2 = s.add_sp_e_m2 | s.ldhl_sp_e_m2;
    c.set_exec = s.set_b_r_m0 | s.set_b_hl_m1;
    c.res_exec = s.res_b_r_m0 | s.res_b_hl_m1;
    c.any_ind_hl_or_ld_sp_hl_or_jp_hl_exec = s.ld_sp_hl_m0
        | s.jp_hl
        | s.res_b_hl_m1
        | s.ld_r_hl_m0
        | s.ld_hl_n_m1
        | s.set_b_hl_m1
        | s.res_or_set_b_hl_m0
        | s.ld_hl_r_m0
        | s.inc_or_dec_hl_m0
        | s.inc_or_dec_hl_m1
        | s.alu_a_hl_m0
        | s.cb_hl_m0
        | s.cb_rxx_or_sxx_or_swap_or_res_m1;

    // The operations (decoder 3): which instruction of a group, from the
    // opcode's bits
    c.add_a_x_or_adc_a_x = exec & !o(5) & !o(4) & s.alu;
    c.sub_or_sbc = exec & !o(5) & o(4) & s.alu;
    c.cp = exec & o(5) & o(4) & o(3) & s.alu;
    c.sub_or_sbc_or_cp = exec & (c.sub_or_sbc | c.cp);
    c.add_or_adc_or_ldhl_exec = exec
        & (c.add_a_x_or_adc_a_x | s.add_hl_ss_m1 | s.add_hl_ss_m0 | c.add_sp_e_or_ldhl_sp_e_m1);
    c.or_or_set_exec = exec & (c.set_exec | (o(5) & o(4) & !o(3) & s.alu));
    c.rxx = exec & (s.rxxa | (!o(5) & s.cb_rxx_or_sxx_or_swap));
    c.sxx_or_swap = exec & o(5) & s.cb_rxx_or_sxx_or_swap;
    c.rlx_or_slx = exec & !o(3) & (c.rxx | (!o(4) & c.sxx_or_swap));
    c.rrx_or_srx = exec & o(3) & (c.rxx | c.sxx_or_swap);
    c.rlc = exec & !o(4) & !o(3) & c.rxx;
    c.rl = exec & o(4) & !o(3) & c.rxx;
    c.rrc = exec & !o(4) & o(3) & c.rxx;
    c.rr = exec & o(4) & o(3) & c.rxx;
    c.sra_op = exec & !o(4) & o(3) & c.sxx_or_swap;
    c.swap = exec & o(4) & !o(3) & c.sxx_or_swap;
    c.daa_m0 = exec & o(5) & !o(4) & !o(3) & s.aux_alu_m0;
    c.cpl_m0 = exec & o(5) & !o(4) & o(3) & s.aux_alu_m0;
    c.scf_or_ccf_m0 = exec & o(5) & o(4) & s.aux_alu_m0;
    c.rxxa_or_daa_or_cpl_m0 = exec & s.aux_alu_m0 & (!o(5) | !o(4));
    c.reti_m3 = exec & o(4) & o(0) & s.retx_m3;
    c.add_sp_e_m1_buf = exec & !o(4) & c.add_sp_e_or_ldhl_sp_e_m1;
    c.add_sp_e_m2_buf = exec & !o(4) & c.add_sp_e_or_ldhl_sp_e_m2;
    c.ldhl_sp_e_m1_buf = exec & o(4) & c.add_sp_e_or_ldhl_sp_e_m1;
    c.ldhl_sp_e_m2_buf = exec & o(4) & c.add_sp_e_or_ldhl_sp_e_m2;
    c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec =
        exec & (s.pop_qq_m2 | s.ld_dd_nn_m2 | c.inc_or_dec_ss_m0);
    c.inc_or_dec_m_or_ld_or_halt = exec & (s.ld_x_n | s.ld_or_halt | s.inc_or_dec_m);
    c.alu_or_alu_cb_or_ld_or_halt = exec & (s.alu | s.ld_or_halt | i.table_cb);
    c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec =
        exec & (c.set_exec | c.res_exec | s.cb_rxx_or_sxx_or_swap);

    // The ALU (decoder 3): add, or, xor, and; the inverted operand 2 and
    // carry
    c.alu_add = exec
        & (c.add_or_adc_or_ldhl_exec
            | s.inc_or_dec_m
            | c.sub_or_sbc_or_cp
            | s.jr_m1
            | c.add_sp_e_or_ldhl_sp_e_m2
            | c.daa_m0
            | c.add_sp_e_or_ldhl_sp_e_m1);
    c.alu_or = exec
        & (c.cpl_m0
            | s.ld_x_n
            | c.or_or_set_exec
            | s.ld_or_halt
            | s.ld_a_n
            | c.ldx_a_nn_or_ld_a_xxx_exec);
    c.alu_xor = exec & o(5) & !o(4) & o(3) & s.alu;
    c.alu_and = (exec & s.alu & o(5) & !o(4) & !o(3)) | (exec & c.res_exec);
    c.alu_inv_op2 = exec & (c.cp | c.cpl_m0 | c.sub_or_sbc | s.bit_op);
    c.alu_inv_cin = exec & (c.cp | c.sub_or_sbc | (s.inc_or_dec_m & o(0)));

    // The flags' writes (decoder 3, in the second half of the M-cycle)
    c.flags_c_we = data
        & c.reg_gp_we_mask
        & (c.add_or_adc_or_ldhl_exec
            | s.alu
            | s.cb_rxx_or_sxx_or_swap
            | s.aux_alu_m0
            | (s.pop_qq_m2 & o(5) & o(4)));
    c.flags_znh_we = data & c.reg_gp_we_mask & (s.inc_or_dec_m | c.flags_c_we | s.bit_op);

    // What goes onto the ALU's operands, the IDU's input and the data bus
    // (decoder 3)
    let alu_r = c.alu_or_alu_cb_or_ld_or_halt;
    c.reg_a_to_op2_en = (exec & c.rxxa_or_daa_or_cpl_m0)
        | (((exec & s.push_qq_m1) | (exec & s.inc_or_dec_m & o(3))) & o(5) & o(4))
        | (exec
            & (c.ldx_nn_a_or_ld_xxx_a_exec
                | c.ld_n_a_or_ld_c_a_exec
                | (o(2) & o(1) & o(0) & alu_r)));
    c.reg_b_to_op2_en = exec
        & ((!o(2) & !o(1) & !o(0) & alu_r)
            | (!o(5) & !o(4) & (s.add_hl_ss_m1 | s.push_qq_m1 | (s.inc_or_dec_m & !o(3)))));
    c.reg_c_to_op2_en = exec
        & ((!o(2) & !o(1) & o(0) & alu_r)
            | (!o(5) & !o(4) & (s.add_hl_ss_m0 | s.push_qq_m2 | (s.inc_or_dec_m & o(3)))));
    c.reg_d_to_op2_en = exec
        & ((!o(2) & o(1) & !o(0) & alu_r)
            | (!o(5) & o(4) & (s.add_hl_ss_m1 | s.push_qq_m1 | (s.inc_or_dec_m & !o(3)))));
    c.reg_e_to_op2_en = exec
        & ((!o(2) & o(1) & o(0) & alu_r)
            | (!o(5) & o(4) & (s.add_hl_ss_m0 | s.push_qq_m2 | (s.inc_or_dec_m & o(3)))));
    c.reg_h_to_op2_en = exec
        & ((o(2) & !o(1) & !o(0) & alu_r)
            | (o(5) & !o(4) & (s.add_hl_ss_m1 | s.push_qq_m1 | (s.inc_or_dec_m & !o(3)))));
    c.reg_l_to_op2_en = exec
        & ((o(2) & !o(1) & o(0) & alu_r)
            | (o(5) & !o(4) & (s.add_hl_ss_m0 | s.push_qq_m2 | (s.inc_or_dec_m & o(3)))));
    c.reg_z_to_op2_en = (exec
        & (s.ld_x_n
            | c.ldx_a_nn_or_ld_a_xxx_exec
            | s.ld_a_n
            | c.add_sp_e_or_ldhl_sp_e_m1
            | (o(2) & o(1) & !o(0) & alu_r)))
        | (exec & (s.jr_m1 | (s.inc_or_dec_m & o(5) & o(4) & !o(3))));
    c.ff_to_op1_en = exec & s.inc_or_dec_m & o(0);
    c.reg_bc_to_idu_en = (exec & c.ld_c_a_or_ld_a_c_m0)
        | (((exec & c.ld_xxx_a_or_ld_a_xxx_m0) | (exec & c.inc_or_dec_ss_m0)) & !o(4) & !o(5));
    c.reg_de_to_idu_en =
        ((exec & c.ld_xxx_a_or_ld_a_xxx_m0) | (exec & c.inc_or_dec_ss_m0)) & !o(5) & o(4);
    c.reg_hl_to_idu_en = (exec
        & ((c.ld_xxx_a_or_ld_a_xxx_m0 & o(5)) | (c.inc_or_dec_ss_m0 & !o(4) & o(5))))
        | (exec & c.any_ind_hl_or_ld_sp_hl_or_jp_hl_exec);
    c.reg_sp_to_idu_en = exec & (c.push_or_pop_exec | (o(5) & o(4) & c.inc_or_dec_ss_m0));
    c.op2_to_db_en = data
        & c.mwrite
        & (s.push_qq_m2
            | c.ld_n_a_or_ld_c_a_exec
            | s.push_qq_m1
            | s.ld_x_n
            | c.ldx_nn_a_or_ld_xxx_a_exec
            | s.ld_or_halt);
    c.alu_to_db_en = (data & s.jr_m1)
        | (data
            & (c.add_sp_e_m1_buf
                | c.add_sp_e_m2_buf
                | (c.mwrite
                    & (c.set_exec | s.inc_or_dec_m | c.res_exec | s.cb_rxx_or_sxx_or_swap))));

    // The registers' writes (decoder 3): the general registers in the
    // second half of the M-cycle, from the ALU, from WZ or from the IDU; W
    // and Z from the data bus; SP and PC from the IDU or from WZ
    let gp = data & c.reg_gp_we_mask;
    let cb_r = c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec;
    let ld_r = c.inc_or_dec_m_or_ld_or_halt;
    let ss = c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec;
    c.reg_a_we = (gp & (c.rxxa_or_daa_or_cpl_m0 | (s.alu & (!o(5) | !o(4) | !o(3)))))
        | (gp
            & (c.ldx_a_nn_or_ld_a_xxx_exec
                | s.ld_a_n
                | ((s.pop_qq_m2 | (ld_r & o(3))) & o(4) & o(5))
                | (o(2) & o(1) & o(0) & cb_r)));
    c.reg_b_we =
        (gp & !o(2) & !o(1) & !o(0) & cb_r) | (((gp & ld_r & !o(3)) | (gp & ss)) & !o(5) & !o(4));
    c.reg_c_we =
        (gp & !o(2) & !o(1) & o(0) & cb_r) | (((gp & ld_r & o(3)) | (gp & ss)) & !o(5) & !o(4));
    c.reg_d_we =
        (gp & !o(2) & o(1) & !o(0) & cb_r) | (((gp & ld_r & !o(3)) | (gp & ss)) & !o(5) & o(4));
    c.reg_e_we =
        (gp & !o(2) & o(1) & o(0) & cb_r) | (((gp & ld_r & o(3)) | (gp & ss)) & !o(5) & o(4));
    c.reg_h_we = (gp & c.ld_xxx_a_or_ld_a_xxx_m0 & o(5))
        | (gp
            & (c.ldhl_sp_e_m2_buf
                | s.add_hl_ss_m1
                | ((ss | (ld_r & !o(3))) & !o(4) & o(5))
                | (o(2) & !o(1) & !o(0) & cb_r)));
    c.reg_l_we = (gp & c.ld_xxx_a_or_ld_a_xxx_m0 & o(5))
        | (gp
            & (s.add_hl_ss_m0
                | c.ldhl_sp_e_m1_buf
                | ((ss | (ld_r & o(3))) & !o(4) & o(5))
                | (o(2) & !o(1) & o(0) & cb_r)));
    c.dec3_y56 = exec & (c.ld_xxx_a_or_ld_a_xxx_m0 | c.inc_or_dec_ss_m0);
    c.dec3_y58 = exec
        & (s.alu
            | c.rxxa_or_daa_or_cpl_m0
            | c.set_exec
            | c.res_exec
            | c.inc_or_dec_m_or_ld_or_halt
            | s.add_hl_ss_m1
            | s.add_hl_ss_m0
            | s.ld_a_n
            | c.ldx_a_nn_or_ld_a_xxx_exec
            | c.ldhl_sp_e_m1_buf
            | c.ldhl_sp_e_m2_buf
            | s.cb_rxx_or_sxx_or_swap);
    c.reg_wz_to_reg_gp_en = data & (s.pop_qq_m2 | s.ld_dd_nn_m2);
    c.reg_w_we = exec & (s.jr_m1 | s.ld_nn_sp_m2 | c.add_sp_e_m2_buf | (c.mread & i.sel_reg_w));
    c.reg_z_we = exec
        & (s.jr_m1
            | c.ld_n_a_or_ld_a_n_m1
            | s.ld_nn_sp_m2
            | c.add_sp_e_m1_buf
            | (c.mread & i.sel_reg_z));
    c.idu_to_reg_sp_en =
        exec & (s.ld_sp_hl_m0 | c.push_or_pop_exec | (o(5) & o(4) & c.inc_or_dec_ss_m0));
    c.reg_wz_to_reg_sp_en = exec & (s.add_sp_e_m3 | (o(5) & o(4) & s.ld_dd_nn_m2));
    c.reg_sp_we = exec & (c.idu_to_reg_sp_en | c.reg_wz_to_reg_sp_en);
    c.idu_to_reg_pc_en = exec & (s.jr_m2 | c.reg_pc_to_idu_en | s.jp_hl);
    c.reg_pc_we = exec & (c.idu_to_reg_pc_en | c.reg_wz_to_reg_pc_en | s.rst_t_m2 | s.int_entry_m6);

    // The registers' gates: FF onto operand 2 for ADD SP,e and LD HL,SP+e
    // with a negative e; the flags onto operand 2 for PUSH AF; WZ's write
    // from the IDU for LD (nn),SP and JR.
    // @hw die: REG_AND2_AND3, REG_OR
    c.ff_to_op2_en = i.z7_delayed & c.add_sp_e_or_ldhl_sp_e_m2;
    c.flags_to_op2_en = o(4) & s.push_qq_m2 & o(5);
    c.ld_nn_sp_or_jr_exec = s.ld_nn_sp_m2 | s.jr_m1;

    // The general registers take the IDU's result in write_phase, the
    // ALU's in data_phase.
    // @hw die: DEC3_NOR1, DEC3_NOR2
    c.idu_to_reg_gp_en = c.dec3_y56 & write;
    c.alu_to_reg_gp_en = c.dec3_y58 & data;

    // STOP is NOP's line with bit 4.
    // @hw die: ZIUL
    c.stop = s.nop_or_stop & o(4);
    c
}
