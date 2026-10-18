-- SPDX-License-Identifier: MIT
--
-- The SM83's ALU and flags. Operand 1 goes through a decoder first, which
-- makes the masks of SET and RES and the corrections of DAA; operand 2 can
-- be inverted (subtraction, CP, CPL, BIT). Then, bit by bit, the ALU
-- computes the generate (both 1) and the propagate (one of them 1), which
-- give AND, OR, XOR and, with a carry chain, the sum; the shifter does the
-- rotations, the shifts and SWAP of operand 2.
--
-- The new flags are computed in write_phase and written when the flags'
-- write enables fall, at the end of the M-cycle, like the registers.
--
-- @hw die: dmg-schematics, categories alu, alu-core, alu-dec (ALU_DECODER),
-- alu-daa and alu-flag

library ieee;
use ieee.std_logic_1164.all;

use work.sm83_decoder.all;
use work.sm83_controls.all;

entity sm83_alu is
	port (
		exec_phase  : in std_ulogic;  -- B to H
		data_phase  : in std_ulogic;  -- E to H
		write_phase : in std_ulogic;  -- G and H
		pch_phase   : in std_ulogic;  -- B to F
		s           : in steps_t;
		c           : in controls_t;
		op          : in std_ulogic_vector(7 downto 0);  -- the opcode
		op1, op2    : in std_ulogic_vector(7 downto 0);
		reg_a       : in std_ulogic_vector(7 downto 0);  -- for DAA
		reg_z       : in std_ulogic_vector(7 downto 0);  -- the flags of POP AF; e's sign

		res         : out std_ulogic_vector(7 downto 0);
		carry7      : out std_ulogic;  -- the carry out of bit 7
		flags       : out std_ulogic_vector(7 downto 4);  -- Z, N, H, C
		cond_fail   : out std_ulogic;  -- the condition of JR/JP/CALL/RET cc fails
		z7_delayed  : out std_ulogic   -- Z's sign, from the M-cycle before
	);
end entity;

architecture rtl of sm83_alu is
	signal flag_z, flag_n, flag_h, flag_c : std_ulogic := '1';
	signal daa_lo_gt9, daa_hi_gt9, daa_hi_ge9 : std_ulogic;
	signal op1dec       : std_ulogic_vector(7 downto 0);
	signal carry_in     : std_ulogic;
	signal operand2     : std_ulogic_vector(7 downto 0);  -- tap_xor: op2, maybe inverted
	signal gen, prop    : std_ulogic_vector(7 downto 0);
	signal half_sum     : std_ulogic_vector(7 downto 0);  -- alu_and: one of them, not both
	signal shift        : std_ulogic_vector(7 downto 0);
	signal logic_out    : std_ulogic_vector(7 downto 0);  -- alu_pggen_y
	signal carry        : std_ulogic_vector(7 downto 0);  -- res_carry
	signal res_i        : std_ulogic_vector(7 downto 0);
	signal res_zero     : std_ulogic;
	signal new_z, new_n, new_h, new_c : std_ulogic;
	-- The flags start at 1, as in dmg-sim (the die keeps them inverted).
	-- @hw inferred: the chip's power-on state is not known
	signal z_master, n_master, h_master, c_master : std_ulogic := '1';
	signal bit_set      : std_ulogic_vector(7 downto 0);  -- the bit of SET and RES
	signal fail         : std_ulogic;
	signal cond_fail_i  : std_ulogic := '0';
	signal z7_master    : std_ulogic := '0';

	-- The opcode's bits 3 to 5, as one bit of eight (BIT, SET, RES)
	function one_of_8(b : std_ulogic_vector(2 downto 0)) return std_ulogic_vector is
		variable r : std_ulogic_vector(7 downto 0) := (others => '0');
	begin
		case b is
			when "000" => r(0) := '1';
			when "001" => r(1) := '1';
			when "010" => r(2) := '1';
			when "011" => r(3) := '1';
			when "100" => r(4) := '1';
			when "101" => r(5) := '1';
			when "110" => r(6) := '1';
			when "111" => r(7) := '1';
			when others => r := (others => 'X');
		end case;
		return r;
	end function;
begin
	-- DAA's comparisons of A's nibbles.
	-- @hw die: REG_OA1, REG_OA2, REG_AND
	daa_lo_gt9 <= (reg_a(1) or reg_a(2)) and reg_a(3);
	daa_hi_gt9 <= (reg_a(5) or reg_a(6)) and reg_a(7);
	daa_hi_ge9 <= reg_a(7) and reg_a(4);

	----------------------------------------------------------------------
	-- Operand 1, decoded: SET ORs its bit in (with operand 2 the register),
	-- RES ANDs all the others in (operand 1 is then FF but that bit); DAA
	-- adds 06, 60, FA, A0 or 9A to A.
	-- @hw die: ALU_DECODER (y1, y4 to y7, y9 to y11), ALU_NOT1 to ALU_NOT8
	----------------------------------------------------------------------
	bit_set <= one_of_8(op(5 downto 3));

	decode_op1 : process (all)
		variable daa : std_ulogic_vector(7 downto 0);
	begin
		daa    := (others => '0');
		daa(1) := flag_h or (daa_lo_gt9 and not flag_n);
		daa(2) := not flag_n and (flag_h or daa_lo_gt9);
		daa(3) := flag_h and flag_n;
		daa(4) := flag_h and flag_n;
		daa(5) := (flag_n and (flag_h xor flag_c))
		          or (not flag_n and (daa_hi_gt9 or flag_c or (daa_hi_ge9 and daa_lo_gt9)));
		daa(6) := (flag_h and not flag_c and flag_n)
		          or (not flag_n and (daa_hi_gt9 or flag_c or (daa_hi_ge9 and daa_lo_gt9)));
		daa(7) := flag_n and (flag_h or flag_c);
		for i in 0 to 7 loop
			op1dec(i) <= exec_phase and (op1(i) or (c.set_exec and bit_set(i))
			                             or (c.res_exec and not bit_set(i)) or (c.daa_m0 and daa(i)));
		end loop;
	end process;

	-- The carry into bit 0: 1 for CP, SUB and INC (inverted: no borrow), the
	-- C flag for ADC, SBC (inverted), ADD SP,e's high byte and ADD HL,rr's.
	-- @hw die: ALU_DECODER (y14), ALU_NOT10
	carry_in <= exec_phase and (c.cp or (not op(0) and s.inc_or_dec_m)
	                            or (c.add_sp_e_or_ldhl_sp_e_m2 and flag_c)
	                            or (c.sub_or_sbc and (not flag_c or not op(3)))
	                            or (flag_c and (s.add_hl_ss_m1 or (c.add_a_x_or_adc_a_x and op(3)))));

	----------------------------------------------------------------------
	-- The shifter: rotations, shifts and SWAP of operand 2.
	-- @hw die: ALU_SHIFTER
	----------------------------------------------------------------------
	shift(0) <= exec_phase and ((flag_c and c.rl) or (op2(4) and c.swap) or (op2(1) and c.rrx_or_srx)
	                            or (op2(7) and c.rlc));
	shift(1) <= exec_phase and ((op2(5) and c.swap) or (op2(2) and c.rrx_or_srx) or (op2(0) and c.rlx_or_slx));
	shift(2) <= exec_phase and ((op2(6) and c.swap) or (op2(3) and c.rrx_or_srx) or (op2(1) and c.rlx_or_slx));
	shift(3) <= exec_phase and ((op2(7) and c.swap) or (op2(4) and c.rrx_or_srx) or (op2(2) and c.rlx_or_slx));
	shift(4) <= exec_phase and ((op2(5) and c.rrx_or_srx) or (op2(0) and c.swap) or (op2(3) and c.rlx_or_slx));
	shift(5) <= exec_phase and ((op2(1) and c.swap) or (op2(6) and c.rrx_or_srx) or (op2(4) and c.rlx_or_slx));
	shift(6) <= exec_phase and ((op2(2) and c.swap) or (op2(5) and c.rlx_or_slx) or (op2(7) and c.rrx_or_srx));
	shift(7) <= exec_phase and ((flag_c and c.rr) or (op2(7) and c.sra_op) or (op2(0) and c.rrc)
	                            or (op2(3) and c.swap) or (op2(6) and c.rlx_or_slx));

	----------------------------------------------------------------------
	-- The core: generate, propagate, the logic operations, the carries and
	-- the result.
	-- @hw die: ALU_PGGEN, ALU_AND, ALU_CGEN_L, ALU_CGEN_H, ALU_RESULT,
	-- ALU_ZGEN, ALU_NOT11, ALU_NOT12
	----------------------------------------------------------------------
	operand2  <= op2 xor (7 downto 0 => c.alu_inv_op2);
	gen       <= operand2 and op1dec;
	prop      <= operand2 or op1dec;
	half_sum  <= prop and not gen;
	logic_out <= (gen and (7 downto 0 => c.alu_and)) or ((7 downto 0 => c.alu_or) and prop) or shift;

	carries : process (all)
		variable cin : std_ulogic;
	begin
		cin := carry_in;
		for i in 0 to 7 loop
			cin      := gen(i) or (prop(i) and cin);
			carry(i) <= cin;
		end loop;
	end process;

	result : process (all)
		variable cin : std_ulogic;
	begin
		for i in 0 to 7 loop
			if i = 0 then
				cin := carry_in;
			else
				cin := carry(i - 1);
			end if;
			res_i(i) <= (half_sum(i) and c.alu_xor) or (c.alu_add and (half_sum(i) xor cin)) or logic_out(i);
		end loop;
	end process;
	res      <= res_i;
	carry7   <= carry(7);
	res_zero <= '1' when res_i = x"00" else '0';

	----------------------------------------------------------------------
	-- The new flags, in write_phase.
	-- @hw die: ALU_DECODER (y2, y3, y8, y13)
	----------------------------------------------------------------------
	new_z <= write_phase and ((operand2(0) and s.bit_op and bit_set(0)) or (operand2(1) and s.bit_op and bit_set(1))
	                          or (operand2(2) and s.bit_op and bit_set(2)) or (operand2(3) and s.bit_op and bit_set(3))
	                          or (operand2(4) and s.bit_op and bit_set(4)) or (operand2(5) and s.bit_op and bit_set(5))
	                          or (operand2(6) and s.bit_op and bit_set(6)) or (operand2(7) and s.bit_op and bit_set(7))
	                          or (s.pop_qq_m2 and reg_z(7))
	                          or (res_zero and (s.cb_rxx_or_sxx_or_swap or s.alu or s.inc_or_dec_m or c.daa_m0))
	                          or (flag_z and (c.cpl_m0 or s.add_hl_ss_m0 or c.scf_or_ccf_m0 or s.add_hl_ss_m1)));
	new_n <= write_phase and (c.cpl_m0 or c.alu_inv_cin or (s.pop_qq_m2 and reg_z(6)) or (flag_n and c.daa_m0));
	new_h <= write_phase and (c.alu_and or s.bit_op or c.cpl_m0 or (s.pop_qq_m2 and reg_z(5))
	                          or (not carry(3) and c.alu_inv_cin)
	                          or (carry(3) and (c.add_or_adc_or_ldhl_exec or (not op(0) and s.inc_or_dec_m))));
	new_c <= write_phase and ((c.rrx_or_srx and operand2(0)) or (c.rlx_or_slx and operand2(7))
	                          or (s.pop_qq_m2 and reg_z(4)) or (c.sub_or_sbc_or_cp and not carry(7))
	                          or (flag_c and c.cpl_m0) or (not op(3) and c.scf_or_ccf_m0)
	                          or (c.add_or_adc_or_ldhl_exec and carry(7))
	                          or (c.scf_or_ccf_m0 and op(3) and not flag_c)
	                          or (c.daa_m0 and (flag_c or (not flag_n and carry(7)))));

	----------------------------------------------------------------------
	-- The flags: taken in write_phase while their write enable is on, shown
	-- when it falls.
	-- @hw die: FLAG_Z, FLAG_N, FLAG_H, FLAG_C (dffn_ee_pch_d_alu_flag)
	----------------------------------------------------------------------
	flags_take : process (all)
	begin
		if write_phase = '1' then
			if c.flags_znh_we = '1' then
				z_master <= new_z;
				n_master <= new_n;
				h_master <= new_h;
			end if;
			if c.flags_c_we = '1' then
				c_master <= new_c;
			end if;
		end if;
	end process;

	flags_show : process (all)
	begin
		if c.flags_znh_we = '0' then
			flag_z <= z_master;
			flag_n <= n_master;
			flag_h <= h_master;
		end if;
		if c.flags_c_we = '0' then
			flag_c <= c_master;
		end if;
	end process;
	flags <= flag_z & flag_n & flag_h & flag_c;

	----------------------------------------------------------------------
	-- The condition (NZ, Z, NC, C: the opcode's bits 3 and 4) fails: found
	-- in write_phase, and kept into phase A, where the M-cycle counter
	-- takes it; cleared from phase B (a precharged node).
	-- @hw die: ALU_DECODER (y12), ALU_NOT9
	----------------------------------------------------------------------
	fail <= (not op(3) and op(4) and flag_c) or (op(3) and op(4) and not flag_c)
	        or (not flag_z and op(3) and not op(4)) or (not op(3) and not op(4) and flag_z);

	cond_fail_node : process (all)
	begin
		if pch_phase = '1' then
			cond_fail_i <= '0';
		elsif write_phase = '1' then
			cond_fail_i <= c.any_cc and fail;
		end if;
	end process;
	cond_fail <= cond_fail_i;

	-- Z's sign, kept for the next M-cycle (ADD SP,e and LD HL,SP+e with a
	-- negative e).
	-- @hw die: ALU_DFF (dffn_ee_q_alu_sign)
	z7_take : process (all)
	begin
		if data_phase = '1' and write_phase = '1' then
			z7_master <= reg_z(7);
		end if;
	end process;

	z7_show : process (all)
	begin
		if data_phase = '0' then
			z7_delayed <= z7_master;
		end if;
	end process;
end architecture;
