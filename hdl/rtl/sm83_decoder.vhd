-- SPDX-License-Identifier: MIT
--
-- The SM83's first decoder: which step of which instruction the CPU core
-- is in. Each step is one line of the decoder (a PLA on the die), which
-- matches the opcode, the M-cycle of the instruction, whether the opcode
-- came after the CB prefix ("cb"), and whether the CPU is entering an
-- interrupt ("int"). The names are those of the netlist, without its
-- prefix ctl_op_: ld_r_r_m0 is LD r,r in its M-cycle 0, ld_x_n is LD r,n
-- and LD (HL),n in all their M-cycles. The other decoders (sm83_control)
-- turn the steps into what the CPU does.
--
-- A line matches during exec_phase (phases B to H) only: the decoder is
-- precharged in phase A.
--
-- @hw die: dmg-schematics, category dec1 (cell decoder1, the lines y1 to
-- y107; y49, y50 and y107 are always off); the patterns were checked to be
-- the same as the lines for all inputs.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

package sm83_decoder is
	type steps_t is record
		nop_or_stop, halt, di_or_ei, cb_prefix : std_ulogic;
		alu, alu_a_r, alu_a_hl_m0, alu_a_n_m0, rxxa, aux_alu_m0 : std_ulogic;
		inc_or_dec_m, inc_or_dec_r, inc_or_dec_hl_m0, inc_or_dec_hl_m1 : std_ulogic;
		inc_ss_m0, dec_ss_m0, add_hl_ss_m0, add_hl_ss_m1, ld_dd_nn_m0,
		ld_dd_nn_m1, ld_dd_nn_m2, ld_sp_hl_m0, add_sp_e_m0, add_sp_e_m1,
		add_sp_e_m2, add_sp_e_m3, ldhl_sp_e_m0, ldhl_sp_e_m1, ldhl_sp_e_m2 : std_ulogic;
		ld_or_halt, ld_r_r_m0, ld_r_hl_m0, ld_hl_r_m0, ld_x_n, ld_x_n_m0,
		ld_r_n_m2, ld_hl_n_m1 : std_ulogic;
		ld_xxx_a_m0, ld_a_xxx_m0, ld_a_xxx_m1, ld_hli_m0, ld_hld_m0, ld_a_n,
		ld_n_a_m0, ld_n_a_m1, ld_a_n_m0, ld_a_n_m1, ld_c_a_m0, ld_a_c_m0,
		ldx_m0, ldx_m1, ldx_nn_a_m2, ldx_a_nn_m2, ldx_a_nn_m3 : std_ulogic;
		ld_nn_sp_m0, ld_nn_sp_m1, ld_nn_sp_m2, ld_nn_sp_m3 : std_ulogic;
		push_qq_m0, push_qq_m1, push_qq_m2, pop_qq_m0, pop_qq_m1, pop_qq_m2 : std_ulogic;
		jr_cc_e_m0, jr_m0, jr_m1, jr_m2, jp_cc_nn_m1, jp_m0, jp_m1, jp_m2,
		jp_hl : std_ulogic;
		call_cc_nn_m1, call_m0, call_m1, call_m2, call_m3, call_m4, ret_cc_m0,
		ret_cc_m1, ret_or_reti_m0, retx_m2, retx_m3, rst_t_m0, rst_t_m1,
		rst_t_m2 : std_ulogic;
		cb_r, cb_hl_m0, cb_rxx_or_sxx_or_swap, bit_op, bit_b_hl_m1,
		cb_rxx_or_sxx_or_swap_or_res_m1, res_b_r_m0, res_b_hl_m1, set_b_r_m0,
		set_b_hl_m1, res_or_set_b_hl_m0 : std_ulogic;
		int_entry_m0, int_entry_m4, int_entry_m5, int_entry_m6 : std_ulogic;
		m6, m7 : std_ulogic;
	end record;

	-- The steps for an opcode, its M-cycle m, the CB prefix and the
	-- interrupt's entry; all off when exec is low.
	function decode(op : std_ulogic_vector(7 downto 0); m : std_ulogic_vector(2 downto 0);
	                cb, int, exec : std_ulogic) return steps_t;
end package;

package body sm83_decoder is
	function decode(op : std_ulogic_vector(7 downto 0); m : std_ulogic_vector(2 downto 0);
	                cb, int, exec : std_ulogic) return steps_t is
		variable s : steps_t;

		-- One line: the patterns of cb, int, m and the opcode, most
		-- significant bit first, with - for the bits that do not count.
		function step(cb_p, int_p, m_p, op_p : std_ulogic_vector) return std_ulogic is
		begin
			if exec = '1' and std_match((0 => cb), cb_p) and std_match((0 => int), int_p)
			   and std_match(m, m_p) and std_match(op, op_p) then
				return '1';
			end if;
			return '0';
		end function;

		function "and"(l : std_ulogic; r : boolean) return std_ulogic is
		begin
			if r then
				return l;
			end if;
			return '0';
		end function;
	begin
		-- No operand: NOP and STOP (00, 10), HALT (76), DI and EI (F3, FB), the CB prefix
		s.nop_or_stop                      := step("0", "0", "0--", "000-0000");
		s.halt                             := step("0", "0", "0--", "01110110");
		s.di_or_ei                         := step("0", "0", "0--", "1111-011");
		s.cb_prefix                        := step("0", "0", "0--", "11001011");

		-- The ALU on A: ALU A,r (80 to BF), ALU A,(HL), ALU A,n (C6 to FE); the
		-- rotations of A and the other ALU operations of 00 to 3F (RLCA, RRCA,
		-- RLA, RRA, DAA, CPL, SCF, CCF)
		s.alu                              := (step("0", "0", "---", "10------") or step("0", "0", "---", "11---110"));
		s.alu_a_r                          := step("0", "0", "0--", "10------") and not std_match(op, "-----110");
		s.alu_a_hl_m0                      := step("0", "0", "-00", "10---110");
		s.alu_a_n_m0                       := step("0", "0", "-00", "11---110");
		s.rxxa                             := step("0", "0", "---", "000--111");
		s.aux_alu_m0                       := step("0", "0", "0--", "00---111");

		-- INC r, DEC r, INC (HL), DEC (HL)
		s.inc_or_dec_m                     := step("0", "0", "---", "00---10-");
		s.inc_or_dec_r                     := step("0", "0", "0--", "00---10-") and not std_match(op, "0011010-");
		s.inc_or_dec_hl_m0                 := step("0", "0", "-00", "0011010-");
		s.inc_or_dec_hl_m1                 := step("0", "0", "-01", "0011010-");

		-- 16-bit: INC rr, DEC rr, ADD HL,rr, LD rr,nn, LD SP,HL, ADD SP,e, LD HL,SP+e
		s.inc_ss_m0                        := step("0", "0", "-00", "00--0011");
		s.dec_ss_m0                        := step("0", "0", "-00", "00--1011");
		s.add_hl_ss_m0                     := step("0", "0", "--0", "00--1001");
		s.add_hl_ss_m1                     := step("0", "0", "-01", "00--1001");
		s.ld_dd_nn_m0                      := step("0", "0", "-00", "00--0001");
		s.ld_dd_nn_m1                      := step("0", "0", "-01", "00--0001");
		s.ld_dd_nn_m2                      := step("0", "0", "-10", "00--0001");
		s.ld_sp_hl_m0                      := step("0", "0", "-00", "11111001");
		s.add_sp_e_m0                      := step("0", "0", "000", "11101000");
		s.add_sp_e_m1                      := step("0", "0", "001", "11101000");
		s.add_sp_e_m2                      := step("0", "0", "010", "11101000");
		s.add_sp_e_m3                      := step("0", "0", "011", "11101000");
		s.ldhl_sp_e_m0                     := step("0", "0", "-00", "11111000");
		s.ldhl_sp_e_m1                     := step("0", "0", "-01", "11111000");
		s.ldhl_sp_e_m2                     := step("0", "0", "-10", "11111000");

		-- Loads of 8 bits: LD r,r (40 to 7F), LD r,(HL), LD (HL),r, LD r,n, LD (HL),n
		s.ld_or_halt                       := step("0", "0", "---", "01------");
		s.ld_r_r_m0                        := step("0", "0", "0--", "01------") and not std_match(op, "-----110") and not std_match(op, "01110---");
		s.ld_r_hl_m0                       := step("0", "0", "-00", "01---110") and not std_match(op, "01110110");
		s.ld_hl_r_m0                       := step("0", "0", "-00", "01110---") and not std_match(op, "-----110");
		s.ld_x_n                           := step("0", "0", "---", "00---110");
		s.ld_x_n_m0                        := step("0", "0", "-00", "00---110");
		s.ld_r_n_m2                        := step("0", "0", "-01", "00---110") and not std_match(op, "00110110");
		s.ld_hl_n_m1                       := step("0", "0", "-01", "00110110");

		-- Loads of A: LD (BC),A, LD (DE),A, LD (HL+),A, LD (HL-),A and back (02 to
		-- 3A); LDH (n),A, LDH A,(n), LDH (C),A, LDH A,(C), LD (nn),A, LD A,(nn)
		s.ld_xxx_a_m0                      := step("0", "0", "-00", "00--0010");
		s.ld_a_xxx_m0                      := step("0", "0", "-00", "00--1010");
		s.ld_a_xxx_m1                      := step("0", "0", "-01", "00--1010");
		s.ld_hli_m0                        := step("0", "0", "-00", "0010-010");
		s.ld_hld_m0                        := step("0", "0", "-00", "0011-010");
		s.ld_a_n                           := step("0", "0", "---", "111100-0");
		s.ld_n_a_m0                        := step("0", "0", "-00", "11100000");
		s.ld_n_a_m1                        := step("0", "0", "-01", "11100000");
		s.ld_a_n_m0                        := step("0", "0", "-00", "11110000");
		s.ld_a_n_m1                        := step("0", "0", "-01", "11110000");
		s.ld_c_a_m0                        := step("0", "0", "-00", "11100010");
		s.ld_a_c_m0                        := step("0", "0", "-00", "11110010");
		s.ldx_m0                           := step("0", "0", "000", "111-1010");
		s.ldx_m1                           := step("0", "0", "001", "111-1010");
		s.ldx_nn_a_m2                      := step("0", "0", "010", "11101010");
		s.ldx_a_nn_m2                      := step("0", "0", "010", "11111010");
		s.ldx_a_nn_m3                      := step("0", "0", "011", "11111010");

		-- LD (nn),SP (08)
		s.ld_nn_sp_m0                      := step("0", "0", "000", "00001000");
		s.ld_nn_sp_m1                      := step("0", "0", "001", "00001000");
		s.ld_nn_sp_m2                      := step("0", "0", "010", "00001000");
		s.ld_nn_sp_m3                      := step("0", "0", "011", "00001000");

		-- PUSH and POP
		s.push_qq_m0                       := step("0", "0", "-00", "11--0101");
		s.push_qq_m1                       := step("0", "0", "-01", "11--0101");
		s.push_qq_m2                       := step("0", "0", "-10", "11--0101");
		s.pop_qq_m0                        := step("0", "0", "-00", "11--0001");
		s.pop_qq_m1                        := step("0", "0", "-01", "11--0001");
		s.pop_qq_m2                        := step("0", "0", "-10", "11--0001");

		-- Jumps: JR e, JR cc,e, JP nn, JP cc,nn, JP HL
		s.jr_cc_e_m0                       := step("0", "0", "-00", "001--000");
		s.jr_m0                            := (step("0", "0", "-00", "001--000") or step("0", "0", "-00", "00011000"));
		s.jr_m1                            := (step("0", "0", "-01", "001--000") or step("0", "0", "-01", "00011000"));
		s.jr_m2                            := (step("0", "0", "-10", "001--000") or step("0", "0", "-10", "00011000"));
		s.jp_cc_nn_m1                      := step("0", "0", "-01", "110--010");
		s.jp_m0                            := (step("0", "0", "-00", "110--010") or step("0", "0", "-00", "11000011"));
		s.jp_m1                            := (step("0", "0", "-01", "110--010") or step("0", "0", "-01", "11000011"));
		s.jp_m2                            := (step("0", "0", "-10", "110--010") or step("0", "0", "-10", "11000011"));
		s.jp_hl                            := step("0", "0", "0--", "11101001");

		-- Calls and returns: CALL nn, CALL cc,nn, RET, RETI, RET cc, RST
		s.call_cc_nn_m1                    := step("0", "0", "-01", "110--100");
		s.call_m0                          := (step("0", "0", "000", "110--100") or step("0", "0", "000", "11001101"));
		s.call_m1                          := (step("0", "0", "001", "110--100") or step("0", "0", "001", "11001101"));
		s.call_m2                          := (step("0", "0", "010", "110--100") or step("0", "0", "010", "11001101"));
		s.call_m3                          := (step("0", "0", "011", "110--100") or step("0", "0", "011", "11001101"));
		s.call_m4                          := (step("0", "0", "100", "110--100") or step("0", "0", "100", "11001101"));
		s.ret_cc_m0                        := step("0", "0", "-00", "110--000");
		s.ret_cc_m1                        := step("0", "0", "-01", "110--000");
		s.ret_or_reti_m0                   := step("0", "0", "-00", "110-1001");
		s.retx_m2                          := (step("0", "0", "010", "110--000") or step("0", "0", "010", "110-1001"));
		s.retx_m3                          := (step("0", "0", "011", "110--000") or step("0", "0", "011", "110-1001"));
		s.rst_t_m0                         := step("0", "0", "-00", "11---111");
		s.rst_t_m1                         := step("0", "0", "-01", "11---111");
		s.rst_t_m2                         := step("0", "0", "-10", "11---111");

		-- After the CB prefix: the rotations and shifts and SWAP (00 to 3F), BIT
		-- (40 to 7F), RES (80 to BF), SET (C0 to FF), on a register or (HL)
		s.cb_r                             := step("1", "0", "0--", "--------") and not std_match(op, "-----110");
		s.cb_hl_m0                         := step("1", "0", "-00", "-----110");
		s.cb_rxx_or_sxx_or_swap            := step("1", "0", "---", "00------");
		s.bit_op                              := step("1", "0", "---", "01------");
		s.bit_b_hl_m1                      := step("1", "0", "-01", "01---110");
		s.cb_rxx_or_sxx_or_swap_or_res_m1  := step("1", "0", "-01", "-0---110");
		s.res_b_r_m0                       := step("1", "0", "-00", "10------") and not std_match(op, "-----110");
		s.res_b_hl_m1                      := step("1", "0", "-01", "10---110");
		s.set_b_r_m0                       := step("1", "0", "-00", "11------") and not std_match(op, "-----110");
		s.set_b_hl_m1                      := step("1", "0", "-01", "11---110");
		s.res_or_set_b_hl_m0               := step("1", "0", "-00", "1----110");

		-- The interrupt's entry, with the M-cycles 0, 4, 5 and 6 ("int" set)
		s.int_entry_m0                     := step("0", "1", "000", "--------");
		s.int_entry_m4                     := step("0", "1", "100", "--------");
		s.int_entry_m5                     := step("0", "1", "101", "--------");
		s.int_entry_m6                     := step("0", "1", "110", "--------");

		-- M-cycles 6 and 7 of any instruction: 7 is the one that the sequencer
		-- jumps to when a condition fails, or while the CPU is idle
		s.m6                               := step("0", "0", "110", "--------");
		s.m7                               := step("0", "-", "111", "--------");

		return s;
	end function;
end package body;
