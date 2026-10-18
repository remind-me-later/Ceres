-- SPDX-License-Identifier: MIT
--
-- The SM83's second and third decoders: from the steps of the first
-- (sm83_decoder) and the opcode's bits, the control lines of the CPU
-- core: the memory request, read and write; the next M-cycle; which
-- register goes onto which bus; what the ALU and the IDU do; which
-- registers and flags are written. Each line is a line of the decoders on
-- the die, with the netlist's name (without its prefix ctl_ or ctl_op_):
-- the decoders are precharged, so that a line is only on in exec_phase
-- (phases B to H), or, for the writes, in data_phase (E to H).
--
-- @hw die: dmg-schematics, categories dec2 (cell decoder2) and dec3 (cell
-- decoder3), and the gates of category reg (REG_AND2_AND3, REG_OR) and of
-- dec3 (DEC3_NOR1, DEC3_NOR2)

library ieee;
use ieee.std_logic_1164.all;

package sm83_controls is
	type controls_t is record
		mreq, mread, mwrite, fetch, reg_gp_we_mask : std_ulogic;
		reg_pc_to_idu_en, reg_pch_to_db_en, reg_pcl_to_db_en, reg_wz_to_reg_pc_en,
		idu_inc, idu_dec, push_or_pop_exec : std_ulogic;
		any_cc, ld_nn_sp_exec, ld_nn_sp_or_ldx_exec, ldx_nn_a_or_ld_xxx_a_exec,
		ldx_a_nn_or_ld_a_xxx_exec, ld_xxx_a_or_ld_a_xxx_m0, ld_n_a_or_ld_c_a_exec,
		ld_n_a_or_ld_a_n_m1, ld_c_a_or_ld_a_c_m0, inc_or_dec_ss_m0,
		add_sp_e_or_ldhl_sp_e_m1, add_sp_e_or_ldhl_sp_e_m2, set_exec, res_exec,
		any_ind_hl_or_ld_sp_hl_or_jp_hl_exec : std_ulogic;
		add_a_x_or_adc_a_x, sub_or_sbc, cp, sub_or_sbc_or_cp, add_or_adc_or_ldhl_exec,
		or_or_set_exec, rxx, sxx_or_swap, rlx_or_slx, rrx_or_srx, rlc, rl, rrc, rr, sra_op,
		swap, daa_m0, cpl_m0, scf_or_ccf_m0, rxxa_or_daa_or_cpl_m0, reti_m3,
		add_sp_e_m1_buf, add_sp_e_m2_buf, ldhl_sp_e_m1_buf, ldhl_sp_e_m2_buf,
		inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec, inc_or_dec_m_or_ld_or_halt,
		alu_or_alu_cb_or_ld_or_halt, cb_rxx_or_sxx_or_swap_or_set_or_res_exec : std_ulogic;
		alu_add, alu_or, alu_xor, alu_and, alu_inv_op2, alu_inv_cin : std_ulogic;
		flags_c_we, flags_znh_we : std_ulogic;
		reg_a_to_op2_en, reg_b_to_op2_en, reg_c_to_op2_en, reg_d_to_op2_en,
		reg_e_to_op2_en, reg_h_to_op2_en, reg_l_to_op2_en, reg_z_to_op2_en,
		ff_to_op1_en, reg_bc_to_idu_en, reg_de_to_idu_en, reg_hl_to_idu_en,
		reg_sp_to_idu_en, op2_to_db_en, alu_to_db_en : std_ulogic;
		reg_a_we, reg_b_we, reg_c_we, reg_d_we, reg_e_we, reg_h_we, reg_l_we, dec3_y56,
		dec3_y58, reg_wz_to_reg_gp_en, reg_w_we, reg_z_we, reg_sp_we, idu_to_reg_sp_en,
		reg_wz_to_reg_sp_en, reg_pc_we, idu_to_reg_pc_en : std_ulogic;
		next_mcyc : std_ulogic_vector(2 downto 0);
		-- The registers' gates (category reg) and decoder 3's NORs
		ff_to_op2_en, flags_to_op2_en, ld_nn_sp_or_jr_exec, idu_to_reg_gp_en,
		alu_to_reg_gp_en, stop : std_ulogic;
	end record;
end package;

library ieee;
use ieee.std_logic_1164.all;

use work.sm83_decoder.all;
use work.sm83_controls.all;

entity sm83_control is
	port (
		s           : in  steps_t;
		op          : in  std_ulogic_vector(7 downto 0);  -- the opcode
		table_cb    : in  std_ulogic;  -- the opcode came after the CB prefix
		exec_phase  : in  std_ulogic;  -- B to H
		data_phase  : in  std_ulogic;  -- E to H
		write_phase : in  std_ulogic;  -- G and H
		sel_reg_w   : in  std_ulogic;  -- a read goes to W (else to Z)
		sel_reg_z   : in  std_ulogic;
		z7_delayed  : in  std_ulogic;  -- Z's sign, from the M-cycle before
		c           : out controls_t
	);
end entity;

architecture rtl of sm83_control is
begin
	-- Memory and the M-cycles (decoder 2): the request, the read, the write,
	-- the opcode's fetch (the last M-cycle of an instruction), the next
	-- M-cycle, and whether the general registers may be written
	c.mreq <= s.jr_m2 or s.ld_a_xxx_m1 or s.aux_alu_m0 or s.push_qq_m1 or s.add_hl_ss_m1 or
	          s.push_qq_m2 or s.pop_qq_m2 or s.ld_nn_sp_m2 or s.add_sp_e_m3 or s.ld_nn_sp_m3 or
	          s.jp_hl or s.ld_dd_nn_m2 or s.rst_t_m2 or s.int_entry_m6 or s.di_or_ei or s.halt
	          or s.nop_or_stop or s.cb_prefix or s.ldx_nn_a_m2 or s.ldx_a_nn_m2 or s.ldx_a_nn_m3
	          or s.call_m0 or s.call_m1 or s.call_m3 or s.call_m4 or s.ld_x_n_m0 or s.ld_r_n_m2
	          or s.m6 or s.m7 or s.jr_m0 or s.ldhl_sp_e_m2 or s.res_b_r_m0 or s.res_b_hl_m1 or
	          s.ld_xxx_a_m0 or s.ld_a_xxx_m0 or s.ld_c_a_m0 or s.ld_n_a_m0 or s.ld_n_a_m1 or
	          s.ld_r_hl_m0 or s.ld_r_r_m0 or s.jp_m0 or s.jp_m1 or s.ld_hl_n_m1 or s.pop_qq_m0
	          or s.pop_qq_m1 or s.set_b_r_m0 or s.set_b_hl_m1 or s.res_or_set_b_hl_m0 or
	          s.ld_a_n_m1 or s.ld_nn_sp_m0 or s.add_sp_e_m0 or s.ldhl_sp_e_m0 or s.ld_nn_sp_m1
	          or s.ld_hl_r_m0 or s.inc_or_dec_hl_m0 or s.inc_or_dec_hl_m1 or s.ld_a_c_m0 or
	          s.ld_a_n_m0 or s.rst_t_m1 or s.int_entry_m5 or s.alu_a_r or s.ret_or_reti_m0 or
	          s.ret_cc_m1 or s.retx_m2 or s.ld_dd_nn_m0 or s.ld_dd_nn_m1 or s.inc_or_dec_r or
	          s.alu_a_hl_m0 or s.alu_a_n_m0 or s.cb_r or s.cb_hl_m0 or s.bit_b_hl_m1 or
	          s.cb_rxx_or_sxx_or_swap_or_res_m1 or s.ldx_m0 or s.ldx_m1;
	c.mread <= s.ldx_a_nn_m2 or s.call_m0 or s.call_m1 or s.ld_x_n_m0 or s.jr_m0 or
	           s.ld_a_xxx_m0 or s.ld_n_a_m0 or s.ld_r_hl_m0 or s.jp_m0 or s.jp_m1 or s.pop_qq_m0
	           or s.pop_qq_m1 or s.res_or_set_b_hl_m0 or s.ld_a_n_m1 or s.ld_nn_sp_m0 or
	           s.add_sp_e_m0 or s.ldhl_sp_e_m0 or s.ld_nn_sp_m1 or s.inc_or_dec_hl_m0 or
	           s.ld_a_c_m0 or s.ld_a_n_m0 or s.ret_or_reti_m0 or s.ret_cc_m1 or s.retx_m2 or
	           s.ld_dd_nn_m0 or s.ld_dd_nn_m1 or s.alu_a_hl_m0 or s.alu_a_n_m0 or s.cb_hl_m0 or
	           s.ldx_m0 or s.ldx_m1;
	c.mwrite <= s.push_qq_m1 or s.push_qq_m2 or s.ld_nn_sp_m2 or s.ld_nn_sp_m3 or s.rst_t_m2 or
	            s.int_entry_m6 or s.ldx_nn_a_m2 or s.call_m3 or s.call_m4 or s.res_b_hl_m1 or
	            s.ld_xxx_a_m0 or s.ld_c_a_m0 or s.ld_n_a_m1 or s.ld_hl_n_m1 or s.set_b_hl_m1 or
	            s.ld_hl_r_m0 or s.inc_or_dec_hl_m1 or s.rst_t_m1 or s.int_entry_m5 or
	            s.cb_rxx_or_sxx_or_swap_or_res_m1;
	c.fetch <= s.jr_m2 or s.ld_a_xxx_m1 or s.aux_alu_m0 or s.add_hl_ss_m1 or s.pop_qq_m2 or
	           s.add_sp_e_m3 or s.jp_hl or s.ld_dd_nn_m2 or s.di_or_ei or s.halt or
	           s.nop_or_stop or s.cb_prefix or s.ldx_a_nn_m3 or s.ld_r_n_m2 or s.m6 or s.m7 or
	           s.ldhl_sp_e_m2 or s.res_b_r_m0 or s.ld_r_r_m0 or s.set_b_r_m0 or s.alu_a_r or
	           s.inc_or_dec_r or s.cb_r or s.bit_b_hl_m1;
	c.next_mcyc(0) <= s.add_hl_ss_m0 or s.push_qq_m2 or s.ld_nn_sp_m2 or s.ld_sp_hl_m0 or
	                  s.ld_nn_sp_m3 or s.retx_m3 or s.rst_t_m2 or s.int_entry_m6 or
	                  s.ldx_nn_a_m2 or s.ldx_a_nn_m2 or s.ret_cc_m0 or s.call_m0 or s.call_m2 or
	                  s.call_m4 or s.ld_x_n_m0 or s.jr_m0 or s.add_sp_e_m2 or s.res_b_hl_m1 or
	                  s.ld_xxx_a_m0 or s.ld_a_xxx_m0 or s.ld_c_a_m0 or s.ld_n_a_m0 or
	                  s.ld_n_a_m1 or s.dec_ss_m0 or s.inc_ss_m0 or s.push_qq_m0 or s.jp_m0 or
	                  s.jp_m2 or s.ld_hl_n_m1 or s.pop_qq_m0 or s.set_b_hl_m1 or
	                  s.res_or_set_b_hl_m0 or s.ld_nn_sp_m0 or s.add_sp_e_m0 or s.ldhl_sp_e_m0
	                  or s.ld_hl_r_m0 or s.inc_or_dec_hl_m0 or s.inc_or_dec_hl_m1 or s.ld_a_n_m0
	                  or s.rst_t_m0 or s.int_entry_m4 or s.retx_m2 or s.ld_dd_nn_m0 or
	                  s.cb_hl_m0 or s.cb_rxx_or_sxx_or_swap_or_res_m1 or s.ldx_m0;
	c.next_mcyc(1) <= s.jr_m1 or s.push_qq_m1 or s.push_qq_m2 or s.ld_nn_sp_m2 or s.ld_sp_hl_m0
	                  or s.ld_nn_sp_m3 or s.retx_m3 or s.rst_t_m2 or s.int_entry_m6 or
	                  s.ldx_nn_a_m2 or s.ldx_a_nn_m2 or s.call_m1 or s.call_m2 or s.call_m4 or
	                  s.add_sp_e_m2 or s.res_b_hl_m1 or s.ld_xxx_a_m0 or s.ld_c_a_m0 or
	                  s.ld_n_a_m1 or s.ld_r_hl_m0 or s.dec_ss_m0 or s.inc_ss_m0 or s.jp_m1 or
	                  s.jp_m2 or s.ld_hl_n_m1 or s.pop_qq_m1 or s.add_sp_e_m1 or s.ldhl_sp_e_m1
	                  or s.set_b_hl_m1 or s.ld_a_n_m1 or s.ld_nn_sp_m1 or s.ld_hl_r_m0 or
	                  s.inc_or_dec_hl_m1 or s.ld_a_c_m0 or s.rst_t_m1 or s.int_entry_m5 or
	                  s.ret_or_reti_m0 or s.ret_cc_m1 or s.retx_m2 or s.ld_dd_nn_m1 or
	                  s.alu_a_hl_m0 or s.alu_a_n_m0 or s.cb_rxx_or_sxx_or_swap_or_res_m1 or
	                  s.ldx_m1;
	c.next_mcyc(2) <= s.push_qq_m2 or s.ld_sp_hl_m0 or s.ld_nn_sp_m3 or s.retx_m3 or s.rst_t_m2
	                  or s.int_entry_m6 or s.ldx_nn_a_m2 or s.call_m3 or s.call_m4 or
	                  s.res_b_hl_m1 or s.ld_xxx_a_m0 or s.ld_c_a_m0 or s.ld_n_a_m1 or
	                  s.ld_r_hl_m0 or s.dec_ss_m0 or s.inc_ss_m0 or s.jp_m2 or s.ld_hl_n_m1 or
	                  s.set_b_hl_m1 or s.ld_a_n_m1 or s.ld_hl_r_m0 or s.inc_or_dec_hl_m1 or
	                  s.ld_a_c_m0 or s.int_entry_m5 or s.int_entry_m4 or s.int_entry_m0 or
	                  s.alu_a_hl_m0 or s.alu_a_n_m0 or s.cb_rxx_or_sxx_or_swap_or_res_m1;
	c.reg_gp_we_mask <= s.ld_a_xxx_m1 or s.aux_alu_m0 or s.add_hl_ss_m0 or s.push_qq_m1 or
	                    s.add_hl_ss_m1 or s.push_qq_m2 or s.pop_qq_m2 or s.ld_sp_hl_m0 or
	                    s.jp_hl or s.ld_dd_nn_m2 or s.rst_t_m2 or s.int_entry_m6 or s.di_or_ei
	                    or s.halt or s.nop_or_stop or s.ldx_a_nn_m3 or s.call_m4 or s.ld_r_n_m2
	                    or s.m6 or s.ldhl_sp_e_m2 or s.res_b_r_m0 or s.ld_xxx_a_m0 or
	                    s.ld_a_xxx_m0 or s.dec_ss_m0 or s.inc_ss_m0 or s.ld_r_r_m0 or s.jp_m2 or
	                    s.add_sp_e_m1 or s.ldhl_sp_e_m1 or s.set_b_r_m0 or s.ld_hl_r_m0 or
	                    s.inc_or_dec_hl_m1 or s.alu_a_r or s.inc_or_dec_r or s.cb_r or
	                    s.bit_b_hl_m1 or s.cb_rxx_or_sxx_or_swap_or_res_m1;

	-- The program counter and the IDU (decoder 2)
	c.reg_pc_to_idu_en <= s.ld_a_xxx_m1 or s.aux_alu_m0 or s.add_hl_ss_m1 or s.pop_qq_m2 or
	                      s.add_sp_e_m3 or s.ld_dd_nn_m2 or s.di_or_ei or s.halt or
	                      s.nop_or_stop or s.cb_prefix or s.ldx_a_nn_m3 or s.call_m0 or
	                      s.call_m1 or s.ld_x_n_m0 or s.ld_r_n_m2 or s.m6 or s.m7 or s.jr_m0 or
	                      s.ldhl_sp_e_m2 or s.res_b_r_m0 or s.ld_n_a_m0 or s.ld_r_r_m0 or
	                      s.jp_m0 or s.jp_m1 or s.set_b_r_m0 or s.ld_nn_sp_m0 or s.add_sp_e_m0
	                      or s.ldhl_sp_e_m0 or s.ld_nn_sp_m1 or s.ld_a_n_m0 or s.int_entry_m0 or
	                      s.alu_a_r or s.ld_dd_nn_m0 or s.ld_dd_nn_m1 or s.inc_or_dec_r or
	                      s.alu_a_n_m0 or s.cb_r or s.bit_b_hl_m1 or s.ldx_m0 or s.ldx_m1;
	c.reg_pch_to_db_en <= s.call_m3 or s.rst_t_m1 or s.int_entry_m5;
	c.reg_pcl_to_db_en <= s.rst_t_m2 or s.int_entry_m6 or s.call_m4;
	c.reg_wz_to_reg_pc_en <= s.retx_m3 or s.call_m4 or s.jp_m2;
	c.idu_inc <= s.jr_m2 or s.ld_a_xxx_m1 or s.aux_alu_m0 or s.add_hl_ss_m1 or s.pop_qq_m2 or
	             s.ld_nn_sp_m2 or s.add_sp_e_m3 or s.jp_hl or s.ld_dd_nn_m2 or s.di_or_ei or
	             s.nop_or_stop or s.cb_prefix or s.ldx_a_nn_m3 or s.call_m0 or s.call_m1 or
	             s.ld_x_n_m0 or s.ld_r_n_m2 or s.m6 or s.m7 or s.jr_m0 or s.ldhl_sp_e_m2 or
	             s.res_b_r_m0 or s.ld_n_a_m0 or s.inc_ss_m0 or s.ld_r_r_m0 or s.jp_m0 or s.jp_m1
	             or s.pop_qq_m0 or s.pop_qq_m1 or s.set_b_r_m0 or s.ld_nn_sp_m0 or s.add_sp_e_m0
	             or s.ldhl_sp_e_m0 or s.ld_nn_sp_m1 or s.ld_a_n_m0 or s.alu_a_r or
	             s.ret_or_reti_m0 or s.ret_cc_m1 or s.retx_m2 or s.ld_hli_m0 or s.ld_dd_nn_m0 or
	             s.ld_dd_nn_m1 or s.inc_or_dec_r or s.alu_a_n_m0 or s.cb_r or s.bit_b_hl_m1 or
	             s.ldx_m0 or s.ldx_m1;
	c.idu_dec <= s.push_qq_m1 or s.call_m2 or s.call_m3 or s.dec_ss_m0 or s.push_qq_m0 or
	             s.rst_t_m1 or s.rst_t_m0 or s.int_entry_m5 or s.int_entry_m4 or s.int_entry_m0
	             or s.ld_hld_m0;
	c.push_or_pop_exec <= s.push_qq_m1 or s.push_qq_m2 or s.rst_t_m2 or s.int_entry_m6 or
	                      s.call_m2 or s.call_m3 or s.call_m4 or s.push_qq_m0 or s.pop_qq_m0 or
	                      s.pop_qq_m1 or s.rst_t_m1 or s.rst_t_m0 or s.int_entry_m5 or
	                      s.int_entry_m4 or s.ret_or_reti_m0 or s.ret_cc_m1 or s.retx_m2;

	-- Steps of several instructions together (decoder 2)
	c.any_cc <= s.jp_cc_nn_m1 or s.call_cc_nn_m1 or s.ret_cc_m0 or s.jr_cc_e_m0;
	c.ld_nn_sp_exec <= s.ld_nn_sp_m2 or s.ld_nn_sp_m3;
	c.ld_nn_sp_or_ldx_exec <= c.ld_nn_sp_exec or s.ldx_nn_a_m2 or s.ldx_a_nn_m2;
	c.ldx_nn_a_or_ld_xxx_a_exec <= s.ldx_nn_a_m2 or s.ld_xxx_a_m0;
	c.ldx_a_nn_or_ld_a_xxx_exec <= s.ld_a_xxx_m1 or s.ldx_a_nn_m3;
	c.ld_xxx_a_or_ld_a_xxx_m0 <= s.ld_xxx_a_m0 or s.ld_a_xxx_m0;
	c.ld_n_a_or_ld_c_a_exec <= s.ld_c_a_m0 or s.ld_n_a_m1;
	c.ld_n_a_or_ld_a_n_m1 <= s.ld_n_a_m1 or s.ld_a_n_m1;
	c.ld_c_a_or_ld_a_c_m0 <= s.ld_c_a_m0 or s.ld_a_c_m0;
	c.inc_or_dec_ss_m0 <= s.dec_ss_m0 or s.inc_ss_m0;
	c.add_sp_e_or_ldhl_sp_e_m1 <= s.add_sp_e_m1 or s.ldhl_sp_e_m1;
	c.add_sp_e_or_ldhl_sp_e_m2 <= s.add_sp_e_m2 or s.ldhl_sp_e_m2;
	c.set_exec <= s.set_b_r_m0 or s.set_b_hl_m1;
	c.res_exec <= s.res_b_r_m0 or s.res_b_hl_m1;
	c.any_ind_hl_or_ld_sp_hl_or_jp_hl_exec <= s.ld_sp_hl_m0 or s.jp_hl or s.res_b_hl_m1 or
	                                          s.ld_r_hl_m0 or s.ld_hl_n_m1 or s.set_b_hl_m1 or
	                                          s.res_or_set_b_hl_m0 or s.ld_hl_r_m0 or
	                                          s.inc_or_dec_hl_m0 or s.inc_or_dec_hl_m1 or
	                                          s.alu_a_hl_m0 or s.cb_hl_m0 or
	                                          s.cb_rxx_or_sxx_or_swap_or_res_m1;

	-- The operations (decoder 3): which instruction of a group, from the
	-- opcode's bits
	c.add_a_x_or_adc_a_x <= exec_phase and not op(5) and not op(4) and s.alu;
	c.sub_or_sbc <= exec_phase and not op(5) and op(4) and s.alu;
	c.cp <= exec_phase and op(5) and op(4) and op(3) and s.alu;
	c.sub_or_sbc_or_cp <= exec_phase and (c.sub_or_sbc or c.cp);
	c.add_or_adc_or_ldhl_exec <= exec_phase and (c.add_a_x_or_adc_a_x or s.add_hl_ss_m1 or
	                             s.add_hl_ss_m0 or c.add_sp_e_or_ldhl_sp_e_m1);
	c.or_or_set_exec <= exec_phase and (c.set_exec or (op(5) and op(4) and not op(3) and
	                    s.alu));
	c.rxx <= exec_phase and (s.rxxa or (not op(5) and s.cb_rxx_or_sxx_or_swap));
	c.sxx_or_swap <= exec_phase and op(5) and s.cb_rxx_or_sxx_or_swap;
	c.rlx_or_slx <= exec_phase and not op(3) and (c.rxx or (not op(4) and c.sxx_or_swap));
	c.rrx_or_srx <= exec_phase and op(3) and (c.rxx or c.sxx_or_swap);
	c.rlc <= exec_phase and not op(4) and not op(3) and c.rxx;
	c.rl <= exec_phase and op(4) and not op(3) and c.rxx;
	c.rrc <= exec_phase and not op(4) and op(3) and c.rxx;
	c.rr <= exec_phase and op(4) and op(3) and c.rxx;
	c.sra_op <= exec_phase and not op(4) and op(3) and c.sxx_or_swap;
	c.swap <= exec_phase and op(4) and not op(3) and c.sxx_or_swap;
	c.daa_m0 <= exec_phase and op(5) and not op(4) and not op(3) and s.aux_alu_m0;
	c.cpl_m0 <= exec_phase and op(5) and not op(4) and op(3) and s.aux_alu_m0;
	c.scf_or_ccf_m0 <= exec_phase and op(5) and op(4) and s.aux_alu_m0;
	c.rxxa_or_daa_or_cpl_m0 <= exec_phase and s.aux_alu_m0 and (not op(5) or not op(4));
	c.reti_m3 <= exec_phase and op(4) and op(0) and s.retx_m3;
	c.add_sp_e_m1_buf <= exec_phase and not op(4) and c.add_sp_e_or_ldhl_sp_e_m1;
	c.add_sp_e_m2_buf <= exec_phase and not op(4) and c.add_sp_e_or_ldhl_sp_e_m2;
	c.ldhl_sp_e_m1_buf <= exec_phase and op(4) and c.add_sp_e_or_ldhl_sp_e_m1;
	c.ldhl_sp_e_m2_buf <= exec_phase and op(4) and c.add_sp_e_or_ldhl_sp_e_m2;
	c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec <= exec_phase and (s.pop_qq_m2 or s.ld_dd_nn_m2
	                                              or c.inc_or_dec_ss_m0);
	c.inc_or_dec_m_or_ld_or_halt <= exec_phase and (s.ld_x_n or s.ld_or_halt or s.inc_or_dec_m);
	c.alu_or_alu_cb_or_ld_or_halt <= exec_phase and (s.alu or s.ld_or_halt or table_cb);
	c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec <= exec_phase and (c.set_exec or c.res_exec or
	                                              s.cb_rxx_or_sxx_or_swap);

	-- The ALU (decoder 3): add, or, xor, and; the inverted operand 2 and carry
	c.alu_add <= exec_phase and (c.add_or_adc_or_ldhl_exec or s.inc_or_dec_m or
	             c.sub_or_sbc_or_cp or s.jr_m1 or c.add_sp_e_or_ldhl_sp_e_m2 or c.daa_m0 or
	             c.add_sp_e_or_ldhl_sp_e_m1);
	c.alu_or <= exec_phase and (c.cpl_m0 or s.ld_x_n or c.or_or_set_exec or s.ld_or_halt or
	            s.ld_a_n or c.ldx_a_nn_or_ld_a_xxx_exec);
	c.alu_xor <= exec_phase and op(5) and not op(4) and op(3) and s.alu;
	c.alu_and <= (exec_phase and s.alu and op(5) and not op(4) and not op(3)) or (exec_phase and
	             c.res_exec);
	c.alu_inv_op2 <= exec_phase and (c.cp or c.cpl_m0 or c.sub_or_sbc or s.bit_op);
	c.alu_inv_cin <= exec_phase and (c.cp or c.sub_or_sbc or (s.inc_or_dec_m and op(0)));

	-- The flags' writes (decoder 3, in the second half of the M-cycle)
	c.flags_c_we <= data_phase and c.reg_gp_we_mask and (c.add_or_adc_or_ldhl_exec or s.alu or
	                s.cb_rxx_or_sxx_or_swap or s.aux_alu_m0 or (s.pop_qq_m2 and op(5) and
	                op(4)));
	c.flags_znh_we <= data_phase and c.reg_gp_we_mask and (s.inc_or_dec_m or c.flags_c_we or
	                  s.bit_op);

	-- What goes onto the ALU's operands, the IDU's input and the data bus
	-- (decoder 3)
	c.reg_a_to_op2_en <= (exec_phase and c.rxxa_or_daa_or_cpl_m0) or (((exec_phase and
	                     s.push_qq_m1) or (exec_phase and s.inc_or_dec_m and op(3))) and op(5)
	                     and op(4)) or (exec_phase and (c.ldx_nn_a_or_ld_xxx_a_exec or
	                     c.ld_n_a_or_ld_c_a_exec or (op(2) and op(1) and op(0) and
	                     c.alu_or_alu_cb_or_ld_or_halt)));
	c.reg_b_to_op2_en <= exec_phase and ((not op(2) and not op(1) and not op(0) and
	                     c.alu_or_alu_cb_or_ld_or_halt) or (not op(5) and not op(4) and
	                     (s.add_hl_ss_m1 or s.push_qq_m1 or (s.inc_or_dec_m and not op(3)))));
	c.reg_c_to_op2_en <= exec_phase and ((not op(2) and not op(1) and op(0) and
	                     c.alu_or_alu_cb_or_ld_or_halt) or (not op(5) and not op(4) and
	                     (s.add_hl_ss_m0 or s.push_qq_m2 or (s.inc_or_dec_m and op(3)))));
	c.reg_d_to_op2_en <= exec_phase and ((not op(2) and op(1) and not op(0) and
	                     c.alu_or_alu_cb_or_ld_or_halt) or (not op(5) and op(4) and
	                     (s.add_hl_ss_m1 or s.push_qq_m1 or (s.inc_or_dec_m and not op(3)))));
	c.reg_e_to_op2_en <= exec_phase and ((not op(2) and op(1) and op(0) and
	                     c.alu_or_alu_cb_or_ld_or_halt) or (not op(5) and op(4) and
	                     (s.add_hl_ss_m0 or s.push_qq_m2 or (s.inc_or_dec_m and op(3)))));
	c.reg_h_to_op2_en <= exec_phase and ((op(2) and not op(1) and not op(0) and
	                     c.alu_or_alu_cb_or_ld_or_halt) or (op(5) and not op(4) and
	                     (s.add_hl_ss_m1 or s.push_qq_m1 or (s.inc_or_dec_m and not op(3)))));
	c.reg_l_to_op2_en <= exec_phase and ((op(2) and not op(1) and op(0) and
	                     c.alu_or_alu_cb_or_ld_or_halt) or (op(5) and not op(4) and
	                     (s.add_hl_ss_m0 or s.push_qq_m2 or (s.inc_or_dec_m and op(3)))));
	c.reg_z_to_op2_en <= (exec_phase and (s.ld_x_n or c.ldx_a_nn_or_ld_a_xxx_exec or s.ld_a_n or
	                     c.add_sp_e_or_ldhl_sp_e_m1 or (op(2) and op(1) and not op(0) and
	                     c.alu_or_alu_cb_or_ld_or_halt))) or (exec_phase and (s.jr_m1 or
	                     (s.inc_or_dec_m and op(5) and op(4) and not op(3))));
	c.ff_to_op1_en <= exec_phase and s.inc_or_dec_m and op(0);
	c.reg_bc_to_idu_en <= (exec_phase and c.ld_c_a_or_ld_a_c_m0) or (((exec_phase and
	                      c.ld_xxx_a_or_ld_a_xxx_m0) or (exec_phase and c.inc_or_dec_ss_m0)) and
	                      not op(4) and not op(5));
	c.reg_de_to_idu_en <= ((exec_phase and c.ld_xxx_a_or_ld_a_xxx_m0) or (exec_phase and
	                      c.inc_or_dec_ss_m0)) and not op(5) and op(4);
	c.reg_hl_to_idu_en <= (exec_phase and ((c.ld_xxx_a_or_ld_a_xxx_m0 and op(5)) or
	                      (c.inc_or_dec_ss_m0 and not op(4) and op(5)))) or (exec_phase and
	                      c.any_ind_hl_or_ld_sp_hl_or_jp_hl_exec);
	c.reg_sp_to_idu_en <= exec_phase and (c.push_or_pop_exec or (op(5) and op(4) and
	                      c.inc_or_dec_ss_m0));
	c.op2_to_db_en <= data_phase and c.mwrite and (s.push_qq_m2 or c.ld_n_a_or_ld_c_a_exec or
	                  s.push_qq_m1 or s.ld_x_n or c.ldx_nn_a_or_ld_xxx_a_exec or s.ld_or_halt);
	c.alu_to_db_en <= (data_phase and s.jr_m1) or (data_phase and (c.add_sp_e_m1_buf or
	                  c.add_sp_e_m2_buf or (c.mwrite and (c.set_exec or s.inc_or_dec_m or
	                  c.res_exec or s.cb_rxx_or_sxx_or_swap))));

	-- The registers' writes (decoder 3): the general registers in the second
	-- half of the M-cycle, from the ALU, from WZ or from the IDU; W and Z
	-- from the data bus; SP and PC from the IDU or from WZ
	c.reg_a_we <= (data_phase and c.reg_gp_we_mask and (c.rxxa_or_daa_or_cpl_m0 or (s.alu and
	              (not op(5) or not op(4) or not op(3))))) or (data_phase and c.reg_gp_we_mask
	              and (c.ldx_a_nn_or_ld_a_xxx_exec or s.ld_a_n or ((s.pop_qq_m2 or
	              (c.inc_or_dec_m_or_ld_or_halt and op(3))) and op(4) and op(5)) or (op(2) and
	              op(1) and op(0) and c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec)));
	c.reg_b_we <= (data_phase and c.reg_gp_we_mask and not op(2) and not op(1) and not op(0) and
	              c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec) or (((data_phase and
	              c.reg_gp_we_mask and c.inc_or_dec_m_or_ld_or_halt and not op(3)) or
	              (data_phase and c.reg_gp_we_mask and
	              c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec)) and not op(5) and not op(4));
	c.reg_c_we <= (data_phase and c.reg_gp_we_mask and not op(2) and not op(1) and op(0) and
	              c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec) or (((data_phase and
	              c.reg_gp_we_mask and c.inc_or_dec_m_or_ld_or_halt and op(3)) or (data_phase
	              and c.reg_gp_we_mask and c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec)) and not
	              op(5) and not op(4));
	c.reg_d_we <= (data_phase and c.reg_gp_we_mask and not op(2) and op(1) and not op(0) and
	              c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec) or (((data_phase and
	              c.reg_gp_we_mask and c.inc_or_dec_m_or_ld_or_halt and not op(3)) or
	              (data_phase and c.reg_gp_we_mask and
	              c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec)) and not op(5) and op(4));
	c.reg_e_we <= (data_phase and c.reg_gp_we_mask and not op(2) and op(1) and op(0) and
	              c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec) or (((data_phase and
	              c.reg_gp_we_mask and c.inc_or_dec_m_or_ld_or_halt and op(3)) or (data_phase
	              and c.reg_gp_we_mask and c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec)) and not
	              op(5) and op(4));
	c.reg_h_we <= (data_phase and c.reg_gp_we_mask and c.ld_xxx_a_or_ld_a_xxx_m0 and op(5)) or
	              (data_phase and c.reg_gp_we_mask and (c.ldhl_sp_e_m2_buf or s.add_hl_ss_m1 or
	              ((c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec or (c.inc_or_dec_m_or_ld_or_halt
	              and not op(3))) and not op(4) and op(5)) or (op(2) and not op(1) and not op(0)
	              and c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec)));
	c.reg_l_we <= (data_phase and c.reg_gp_we_mask and c.ld_xxx_a_or_ld_a_xxx_m0 and op(5)) or
	              (data_phase and c.reg_gp_we_mask and (s.add_hl_ss_m0 or c.ldhl_sp_e_m1_buf or
	              ((c.inc_or_dec_ss_or_ld_dd_nn_or_pop_qq_exec or (c.inc_or_dec_m_or_ld_or_halt
	              and op(3))) and not op(4) and op(5)) or (op(2) and not op(1) and op(0) and
	              c.cb_rxx_or_sxx_or_swap_or_set_or_res_exec)));
	c.dec3_y56 <= exec_phase and (c.ld_xxx_a_or_ld_a_xxx_m0 or c.inc_or_dec_ss_m0);
	c.dec3_y58 <= exec_phase and (s.alu or c.rxxa_or_daa_or_cpl_m0 or c.set_exec or c.res_exec
	              or c.inc_or_dec_m_or_ld_or_halt or s.add_hl_ss_m1 or s.add_hl_ss_m0 or
	              s.ld_a_n or c.ldx_a_nn_or_ld_a_xxx_exec or c.ldhl_sp_e_m1_buf or
	              c.ldhl_sp_e_m2_buf or s.cb_rxx_or_sxx_or_swap);
	c.reg_wz_to_reg_gp_en <= data_phase and (s.pop_qq_m2 or s.ld_dd_nn_m2);
	c.reg_w_we <= exec_phase and (s.jr_m1 or s.ld_nn_sp_m2 or c.add_sp_e_m2_buf or (c.mread and
	              sel_reg_w));
	c.reg_z_we <= exec_phase and (s.jr_m1 or c.ld_n_a_or_ld_a_n_m1 or s.ld_nn_sp_m2 or
	              c.add_sp_e_m1_buf or (c.mread and sel_reg_z));
	c.reg_sp_we <= exec_phase and (c.idu_to_reg_sp_en or c.reg_wz_to_reg_sp_en);
	c.idu_to_reg_sp_en <= exec_phase and (s.ld_sp_hl_m0 or c.push_or_pop_exec or (op(5) and
	                      op(4) and c.inc_or_dec_ss_m0));
	c.reg_wz_to_reg_sp_en <= exec_phase and (s.add_sp_e_m3 or (op(5) and op(4) and
	                         s.ld_dd_nn_m2));
	c.reg_pc_we <= exec_phase and (c.idu_to_reg_pc_en or c.reg_wz_to_reg_pc_en or s.rst_t_m2 or
	               s.int_entry_m6);
	c.idu_to_reg_pc_en <= exec_phase and (s.jr_m2 or c.reg_pc_to_idu_en or s.jp_hl);

	-- The registers' gates: FF onto operand 2 for ADD SP,e and LD HL,SP+e
	-- with a negative e; the flags onto operand 2 for PUSH AF; WZ's write
	-- from the IDU for LD (nn),SP and JR.
	-- @hw die: REG_AND2_AND3, REG_OR
	c.ff_to_op2_en        <= z7_delayed and c.add_sp_e_or_ldhl_sp_e_m2;
	c.flags_to_op2_en     <= op(4) and s.push_qq_m2 and op(5);
	c.ld_nn_sp_or_jr_exec <= s.ld_nn_sp_m2 or s.jr_m1;

	-- The general registers take the IDU's result in write_phase, the ALU's
	-- in data_phase.
	-- @hw die: DEC3_NOR1, DEC3_NOR2
	c.idu_to_reg_gp_en <= c.dec3_y56 and write_phase;
	c.alu_to_reg_gp_en <= c.dec3_y58 and data_phase;

	-- STOP is NOP's line with bit 4.
	-- @hw die: ZIUL
	c.stop <= s.nop_or_stop and op(4);
end architecture;
