-- SPDX-License-Identifier: MIT
--
-- The SM83's registers: A, B to L, SP, PC, and W and Z (the temporary
-- register of the instructions that read 16 bits). And the buses that read
-- them: the ALU's two operands (op1, op2), the IDU's input (which is also
-- the address the core puts out), and the data bus (db) for the writes of
-- PC and SP.
--
-- A register takes its new value in write_phase (G and H) while its write
-- enable is on, and shows it when the enable falls, at the end of the
-- M-cycle: during the M-cycle, the buses read the old value. The buses are
-- precharged in phase A, when nothing drives them: they are 0 there (and so
-- is the address).
--
-- @hw die: dmg-schematics, categories reg-a, reg-bc, reg-de, reg-hl,
-- reg-sp, reg-pc, reg-wz and reg-bus

library ieee;
use ieee.std_logic_1164.all;

-- One register of 8 bits (a C2MOS master and slave on the die): the master
-- takes d in write_phase while we is on, the slave follows the master while
-- we is off. reset (if any) clears the master.
entity sm83_register is
	generic (
		-- The value at power-on, as in dmg-sim, whose cells start with their
		-- nodes at 0: the registers that the die keeps inverted (SP, PC, W
		-- and Z) start at FF.
		-- @hw inferred: the chip's power-on state is not known
		INIT : std_ulogic_vector(7 downto 0) := x"00"
	);
	port (
		we          : in  std_ulogic;
		write_phase : in  std_ulogic;
		reset       : in  std_ulogic := '0';
		d           : in  std_ulogic_vector(7 downto 0);
		q           : out std_ulogic_vector(7 downto 0) := INIT
	);
end entity;

architecture rtl of sm83_register is
	signal master : std_ulogic_vector(7 downto 0) := INIT;
begin
	-- @hw die: dff_cc_ee_q_x1_reg_bit, dff_cc_ee_q_x2_reg_bit,
	-- dff_cc_ee_q_n_reg_wz_bit, dff_cc_ee_pch_d_reg_sp_bit,
	-- dffs_cc_ee_pch_d_reg_pc_bit
	take : process (all)
	begin
		if reset = '1' then
			master <= (others => '0');
		elsif we = '1' and write_phase = '1' then
			master <= to_x01(d);
		end if;
	end process;

	show : process (all)
	begin
		if we = '0' then
			q <= master;
		end if;
	end process;
end architecture;

library ieee;
use ieee.std_logic_1164.all;

use work.sm83_decoder.all;
use work.sm83_controls.all;

entity sm83_registers is
	port (
		write_phase : in std_ulogic;  -- G and H
		sys_reset   : in std_ulogic;
		s           : in steps_t;
		c           : in controls_t;
		op          : in std_ulogic_vector(7 downto 0);  -- the opcode
		res         : in std_ulogic_vector(7 downto 0);  -- the ALU's result
		idu_out     : in std_ulogic_vector(15 downto 0);
		int_vector  : in std_ulogic_vector(7 downto 3);
		flags       : in std_ulogic_vector(7 downto 4);  -- Z, N, H, C
		db          : inout std_logic_vector(7 downto 0);

		op1         : out std_ulogic_vector(7 downto 0);
		op2         : out std_ulogic_vector(7 downto 0);
		idu_in      : out std_ulogic_vector(15 downto 0);
		addr_from_w : out std_ulogic;  -- W drives the address's high byte
		addr_late   : out std_ulogic;  -- HL or SP does
		addr_driven : out std_ulogic;  -- something does (else it is precharged)
		reg_a, reg_z, reg_w : out std_ulogic_vector(7 downto 0)
	);
end entity;

architecture rtl of sm83_registers is
	signal a, b, c_reg, d, e, h, l : std_ulogic_vector(7 downto 0);
	signal w, z                    : std_ulogic_vector(7 downto 0);
	signal sph, spl, pch, pcl      : std_ulogic_vector(7 downto 0);
	signal new_hi, new_lo          : std_ulogic_vector(7 downto 0);
	signal new_w, new_z            : std_ulogic_vector(7 downto 0);
	signal new_sph, new_spl        : std_ulogic_vector(7 downto 0);
	signal new_pch, new_pcl        : std_ulogic_vector(7 downto 0);
	signal rst_vector              : std_ulogic_vector(7 downto 0);
	signal add_hl_sp               : std_ulogic;

	-- An enable for each bit
	function all8(x : std_ulogic) return std_ulogic_vector is
		variable r : std_ulogic_vector(7 downto 0);
	begin
		r := (others => x);
		return r;
	end function;
begin
	----------------------------------------------------------------------
	-- The general registers. A, B, D and H take the high bus, C, E and L
	-- the low one: from the ALU (in data_phase), from WZ (POP and LD rr,nn)
	-- or from the IDU (in write_phase: INC rr, DEC rr, LD (HL+)).
	-- @hw die: REG_WZ_OUT (aoi_a, aoi_b), REG_NOT
	----------------------------------------------------------------------
	new_hi <= (all8(c.alu_to_reg_gp_en) and res) or (all8(c.reg_wz_to_reg_gp_en) and w)
	          or (all8(c.idu_to_reg_gp_en) and idu_out(15 downto 8));
	new_lo <= (all8(c.alu_to_reg_gp_en) and res) or (all8(c.reg_wz_to_reg_gp_en) and z)
	          or (all8(c.idu_to_reg_gp_en) and idu_out(7 downto 0));

	-- @hw die: REG_A, REG_B, REG_C, REG_D, REG_E, REG_H, REG_L
	u_a : entity work.sm83_register port map (we => c.reg_a_we, write_phase => write_phase, d => new_hi, q => a);
	u_b : entity work.sm83_register port map (we => c.reg_b_we, write_phase => write_phase, d => new_hi, q => b);
	u_c : entity work.sm83_register port map (we => c.reg_c_we, write_phase => write_phase, d => new_lo, q => c_reg);
	u_d : entity work.sm83_register port map (we => c.reg_d_we, write_phase => write_phase, d => new_hi, q => d);
	u_e : entity work.sm83_register port map (we => c.reg_e_we, write_phase => write_phase, d => new_lo, q => e);
	u_h : entity work.sm83_register port map (we => c.reg_h_we, write_phase => write_phase, d => new_hi, q => h);
	u_l : entity work.sm83_register port map (we => c.reg_l_we, write_phase => write_phase, d => new_lo, q => l);

	----------------------------------------------------------------------
	-- W and Z take the data bus (a read), or, for LD (nn),SP and JR, the
	-- IDU's result.
	-- @hw die: REG_W, REG_Z, REG_WZ_OUT (muxi_a, muxi_b)
	----------------------------------------------------------------------
	new_z <= idu_out(7 downto 0) when s.ld_nn_sp_m2 = '1' else to_x01(db);
	new_w <= idu_out(15 downto 8) when c.ld_nn_sp_or_jr_exec = '1' else to_x01(db);

	u_w : entity work.sm83_register generic map (INIT => x"FF") port map (we => c.reg_w_we, write_phase => write_phase, d => new_w, q => w);
	u_z : entity work.sm83_register generic map (INIT => x"FF") port map (we => c.reg_z_we, write_phase => write_phase, d => new_z, q => z);

	----------------------------------------------------------------------
	-- SP takes the IDU's result or WZ; PC too, or the vector of RST or of
	-- the interrupt's entry. The reset clears PC.
	-- @hw die: REG_SP_OUT (y6, y7), REG_SPH, REG_SPL; REG_PC_OUT (y5, y6),
	-- REG_PCH, REG_PCL, REG_PCH_NOT_WE REG_PCL_NOT_WE
	----------------------------------------------------------------------
	new_sph <= all8(write_phase) and ((all8(c.reg_wz_to_reg_sp_en) and w)
	                                  or (all8(c.idu_to_reg_sp_en) and idu_out(15 downto 8)));
	new_spl <= all8(write_phase) and ((all8(c.idu_to_reg_sp_en) and idu_out(7 downto 0))
	                                  or (all8(c.reg_wz_to_reg_sp_en) and z));

	-- RST's vector is the opcode's bits 3 to 5.
	rst_vector <= "00" & (op(5 downto 3) and all8(s.rst_t_m2)(2 downto 0)) & "000";

	new_pch <= all8(write_phase) and ((all8(c.reg_wz_to_reg_pc_en) and w)
	                                  or (all8(c.idu_to_reg_pc_en) and idu_out(15 downto 8)));
	new_pcl <= all8(write_phase) and ((all8(c.reg_wz_to_reg_pc_en) and z)
	                                  or (all8(c.idu_to_reg_pc_en) and idu_out(7 downto 0))
	                                  or rst_vector or (int_vector & "000"));

	u_sph : entity work.sm83_register generic map (INIT => x"FF") port map (we => c.reg_sp_we, write_phase => write_phase, d => new_sph, q => sph);
	u_spl : entity work.sm83_register generic map (INIT => x"FF") port map (we => c.reg_sp_we, write_phase => write_phase, d => new_spl, q => spl);
	u_pch : entity work.sm83_register generic map (INIT => x"FF")
		port map (we => c.reg_pc_we, write_phase => write_phase, reset => sys_reset, d => new_pch, q => pch);
	u_pcl : entity work.sm83_register generic map (INIT => x"FF")
		port map (we => c.reg_pc_we, write_phase => write_phase, reset => sys_reset, d => new_pcl, q => pcl);

	----------------------------------------------------------------------
	-- The ALU's operand 1: A (the ALU's instructions), H or L (ADD HL,rr),
	-- PCL (JR), SPL or SPH (ADD SP,e and LD HL,SP+e), or FF (DEC).
	-- @hw die: REG_A_OUT (a), REG_HL_OUT (y3), REG_PC_OUT (y4), REG_SP_OUT
	-- (y2), REG_BUS_PCH_A (b)
	----------------------------------------------------------------------
	op1 <= (all8(s.alu) and a) or (all8(s.add_hl_ss_m1) and h) or (all8(s.add_hl_ss_m0) and l)
	       or (all8(s.jr_m1) and pcl) or all8(c.ff_to_op1_en)
	       or (all8(c.add_sp_e_or_ldhl_sp_e_m1) and spl) or (all8(c.add_sp_e_or_ldhl_sp_e_m2) and sph);

	----------------------------------------------------------------------
	-- The ALU's operand 2: a register, Z (an immediate or a read), SP (ADD
	-- HL,SP), FF (the sign of ADD SP,e), or the flags (PUSH AF).
	-- @hw die: REG_A_OUT (b), REG_BC_OUT, REG_DE_OUT, REG_HL_OUT (y2),
	-- REG_WZ_OUT (nand), REG_SP_OUT (y3), REG_BUS_PCH_A (c)
	----------------------------------------------------------------------
	add_hl_sp <= op(5) and op(4);

	op2 <= (all8(c.reg_a_to_op2_en) and a) or (all8(c.reg_b_to_op2_en) and b)
	       or (all8(c.reg_c_to_op2_en) and c_reg) or (all8(c.reg_d_to_op2_en) and d)
	       or (all8(c.reg_e_to_op2_en) and e) or (all8(c.reg_h_to_op2_en) and h)
	       or (all8(c.reg_l_to_op2_en) and l) or (all8(c.reg_z_to_op2_en) and z)
	       or (all8(s.add_hl_ss_m0 and add_hl_sp) and spl) or (all8(s.add_hl_ss_m1 and add_hl_sp) and sph)
	       or all8(c.ff_to_op2_en) or ((all8(c.flags_to_op2_en)(3 downto 0) and flags) & "0000");

	----------------------------------------------------------------------
	-- The IDU's input, which is also the address: a register pair, PC, SP
	-- or WZ, with FF as the high byte for LDH.
	-- @hw die: REG_BC_OUT, REG_DE_OUT (y1 and d_zero), REG_HL_OUT (y1),
	-- REG_PC_OUT (y2, y3), REG_SP_OUT (y4, y5), REG_WZ_OUT (oai21, oai31),
	-- REG_BUS_PCH_B
	----------------------------------------------------------------------
	idu_in(7 downto 0) <= (all8(c.reg_bc_to_idu_en) and c_reg) or (all8(c.reg_de_to_idu_en) and e)
	                      or (all8(c.reg_hl_to_idu_en) and l) or (all8(c.reg_pc_to_idu_en) and pcl)
	                      or (all8(c.reg_sp_to_idu_en) and spl)
	                      or (all8(s.jr_m2 or c.ld_nn_sp_or_ldx_exec or c.ld_n_a_or_ld_a_n_m1) and z);
	idu_in(15 downto 8) <= (all8(c.reg_bc_to_idu_en) and b) or (all8(c.reg_de_to_idu_en) and d)
	                       or all8(c.ld_c_a_or_ld_a_c_m0 or c.ld_n_a_or_ld_a_n_m1)
	                       or (all8(c.reg_hl_to_idu_en) and h)
	                       or (all8(c.reg_pc_to_idu_en or s.jr_m1) and pch)
	                       or (all8(c.reg_sp_to_idu_en) and sph)
	                       or (all8(s.jr_m2 or c.ld_nn_sp_or_ldx_exec) and w);

	-- Which drives the high byte, for the delay with which it leaves the
	-- core (../rtl/sm83.vhd): W's own gates are faster than the register
	-- file's outputs, and HL's and SP's are the slowest.
	addr_from_w <= s.jr_m2 or c.ld_nn_sp_or_ldx_exec;
	addr_late   <= c.reg_hl_to_idu_en or c.reg_sp_to_idu_en;
	addr_driven <= c.reg_bc_to_idu_en or c.reg_de_to_idu_en or c.reg_hl_to_idu_en
	               or c.reg_pc_to_idu_en or s.jr_m1 or c.reg_sp_to_idu_en
	               or c.ld_c_a_or_ld_a_c_m0 or c.ld_n_a_or_ld_a_n_m1
	               or s.jr_m2 or c.ld_nn_sp_or_ldx_exec;

	----------------------------------------------------------------------
	-- PC and SP onto the data bus (CALL, RST and the interrupt's entry push
	-- PC; LD (nn),SP writes SP): their bits that are 0 pull it down.
	-- @hw die: REG_PC_OUT (y1), REG_SP_OUT (y1)
	----------------------------------------------------------------------
	to_db : for i in 0 to 7 generate
		db(i) <= '0' when (c.reg_pcl_to_db_en = '1' and pcl(i) = '0')
		               or (c.reg_pch_to_db_en = '1' and pch(i) = '0')
		               or (s.ld_nn_sp_m2 = '1' and spl(i) = '0')
		               or (s.ld_nn_sp_m3 = '1' and sph(i) = '0') else 'Z';
	end generate;

	reg_a <= a;
	reg_z <= z;
	reg_w <= w;
end architecture;
