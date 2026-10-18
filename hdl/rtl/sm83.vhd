-- SPDX-License-Identifier: MIT
--
-- The SM83, the DMG's CPU core: the decoders (sm83_decoder, sm83_control),
-- the sequencer, the registers, the IDU, the ALU and the interrupt logic,
-- around the core's data bus (db).
--
-- An M-cycle is the chip's eight phases: the core's flip-flops change at
-- the rise of clk (phase A); the decoders and the buses are precharged in
-- phase A and work from B (exec_phase); reads take the data bus in the
-- second half (data_phase, E to H); registers take their new values in
-- write_phase (G and H) and show them at the end of the M-cycle.
--
-- db joins the chip's data bus through a bridge of wired ANDs: each side
-- pulls the other down where it is 0, so that the core's own values show
-- on the chip's bus whenever the core does not read, and a value that
-- changes from 0 to 1 during a read meets the core still pulling it down.
--
-- @hw die: dmg-schematics, the SM83 (sm83/); the categories dbus,
-- dbus-mreq and dbus-rd here

library ieee;
use ieee.std_logic_1164.all;

use work.sm83_decoder.all;
use work.sm83_controls.all;

entity sm83 is
	port (
		clk             : in std_ulogic;  -- high in phase A
		exec_phase      : in std_ulogic;  -- B to H
		data_phase      : in std_ulogic;  -- E to H
		write_phase     : in std_ulogic;  -- G and H
		pch_phase       : in std_ulogic;  -- B to F
		sys_reset       : in std_ulogic;
		pwron_reset     : in std_ulogic;
		clk_ready       : in std_ulogic;  -- the oscillator is stable
		wake            : in std_ulogic;  -- ends STOP
		nmi             : in std_ulogic := '0';
		oe_n            : in std_ulogic;  -- the test mode T1 alone: the core lets go of the buses
		internal_access : in std_ulogic;  -- FE00 to FFFF: no memory request
		shadow_access   : in std_ulogic;  -- the boot ROM: no memory request
		shadow_override : in std_ulogic;  -- unless in the test mode T2 alone
		int             : in std_ulogic_vector(7 downto 0);  -- the interrupt requests

		a               : out std_logic_vector(15 downto 0);  -- the address
		d               : inout std_logic_vector(7 downto 0);  -- the chip's data bus
		ctl_mreq        : out std_ulogic := '0';  -- the decoder's memory request
		mreq            : out std_ulogic := '0';  -- the memory request, to the chip
		rd              : out std_ulogic := '0';
		wr              : out std_ulogic := '0';
		m1              : out std_ulogic := '0';  -- the opcode's fetch
		halt_n          : out std_ulogic := '0';
		stop_n          : out std_ulogic := '1';
		inta            : out std_ulogic_vector(7 downto 0) := (others => '0')  -- the acknowledges
	);
end entity;

architecture rtl of sm83 is
	-- The core's outputs reach the chip later than its flip-flops change:
	-- the control lines about 15 ns after the edge, the address's high byte
	-- at 22 ns and its low byte at 33 ns, the data at 40 ns.
	-- @hw measured: dmg-sim (the delays of its gates and wires)
	constant t_control : time := 14 ns;
	constant t_addr_hi : time := 21 ns;
	constant t_addr_lo : time := 32 ns;
	constant t_data    : time := 39 ns;
	constant t_mreq_gate : time := 8 ns;

	signal s           : steps_t;
	signal c           : controls_t;
	signal db          : std_logic_vector(7 downto 0);
	signal kept        : std_ulogic_vector(7 downto 0) := (others => '1');
	signal opcode      : std_ulogic_vector(7 downto 0);
	signal mcyc        : std_ulogic_vector(2 downto 0);
	signal table_cb    : std_ulogic;
	signal in_intr     : std_ulogic;
	signal sel_reg_w, sel_reg_z : std_ulogic;
	signal halt, stop  : std_ulogic;
	signal int_entry, nmi_entry : std_ulogic;
	signal ime         : std_ulogic;
	signal op1, op2    : std_ulogic_vector(7 downto 0);
	signal res         : std_ulogic_vector(7 downto 0);
	signal carry7      : std_ulogic;
	signal flags       : std_ulogic_vector(7 downto 4);
	signal cond_fail   : std_ulogic;
	signal z7_delayed  : std_ulogic;
	signal idu_in, idu_out : std_ulogic_vector(15 downto 0);
	signal reg_a, reg_z, reg_w : std_ulogic_vector(7 downto 0);
	signal int_vector  : std_ulogic_vector(7 downto 3);
	signal int_take, int_pending : std_ulogic;
	signal ffxx, ffff  : std_ulogic;
	signal rd_i        : std_ulogic;
	signal ctl_mreq_i  : std_ulogic := '0';
	signal oe          : std_ulogic;
	signal d_out       : std_ulogic_vector(7 downto 0);  -- what the core pulls down on d
begin
	oe <= not oe_n;

	s <= decode(opcode, mcyc, table_cb, in_intr, exec_phase);

	u_control : entity work.sm83_control
		port map (
			s => s, op => opcode, table_cb => table_cb, exec_phase => exec_phase,
			data_phase => data_phase, write_phase => write_phase,
			sel_reg_w => sel_reg_w, sel_reg_z => sel_reg_z, z7_delayed => z7_delayed, c => c
		);

	u_sequencer : entity work.sm83_sequencer
		port map (
			clk => clk, exec_phase => exec_phase, data_phase => data_phase,
			write_phase => write_phase, sys_reset => sys_reset, pwron_reset => pwron_reset,
			clk_ready => clk_ready, wake => wake, nmi => nmi, test_freeze => '0',
			s => s, c => c, db => db, alu_cond_fail => cond_fail, int_take => int_take,
			int_pending => int_pending,
			opcode => opcode, mcyc => mcyc, table_cb => table_cb, in_intr => in_intr,
			sel_reg_w => sel_reg_w, sel_reg_z => sel_reg_z, halt => halt, stop => stop,
			int_entry => int_entry, nmi_entry => nmi_entry, ime => ime
		);

	u_registers : entity work.sm83_registers
		port map (
			write_phase => write_phase, sys_reset => sys_reset, s => s, c => c, op => opcode,
			res => res, idu_out => idu_out, int_vector => int_vector, flags => flags, db => db,
			op1 => op1, op2 => op2, idu_in => idu_in, reg_a => reg_a, reg_z => reg_z, reg_w => reg_w
		);

	u_idu : entity work.sm83_idu
		port map (
			data_phase => data_phase, s => s, c => c, z7 => reg_z(7), carry7 => carry7,
			idu_in => idu_in, idu_out => idu_out
		);

	u_alu : entity work.sm83_alu
		port map (
			exec_phase => exec_phase, data_phase => data_phase, write_phase => write_phase,
			pch_phase => pch_phase, s => s, c => c, op => opcode, op1 => op1, op2 => op2,
			reg_a => reg_a, reg_z => reg_z,
			res => res, carry7 => carry7, flags => flags, cond_fail => cond_fail,
			z7_delayed => z7_delayed
		);

	u_interrupts : entity work.sm83_interrupts
		port map (
			data_phase => data_phase, write_phase => write_phase, sys_reset => sys_reset,
			s => s, c => c, rd => rd_i, a => idu_in, db => db, int => int, nmi_entry => nmi_entry,
			ffxx => ffxx, ffff => ffff, int_pending => int_pending, int_take => int_take,
			inta => inta, int_vector => int_vector
		);

	----------------------------------------------------------------------
	-- The core's data bus: precharged in phase A, pulled down by what drives
	-- it (the chip's bus through the bridge, the ALU, operand 2, PC, SP,
	-- IE), and kept while nothing does.
	-- @hw die: DBUS_BRIDGE (b2b_wand_inj_a), DBUS_NAND, DBUS_NOT,
	-- REG_BUS_PCH_A (a)
	----------------------------------------------------------------------
	core_bus : for i in 0 to 7 generate
		db(i) <= '1' when exec_phase = '0' else 'Z';
		db(i) <= '0' when exec_phase = '1' and oe = '1' and d(i) = '0' else 'Z';
		db(i) <= '0' when c.alu_to_db_en = '1' and res(i) = '0' else 'Z';
		db(i) <= '0' when c.op2_to_db_en = '1' and op2(i) = '0' else 'Z';
		db(i) <= 'H' when kept(i) = '1' else 'L';
	end generate;

	keeper : process (db)
	begin
		for i in db'range loop
			if db(i) = '0' or db(i) = '1' then
				kept(i) <= db(i);
			end if;
		end loop;
	end process;

	-- The chip's side of the bridge: the core pulls it down where db is 0,
	-- and precharges it in phase A.
	d_out <= to_x01(db);
	chip_bus : for i in 0 to 7 generate
		d(i) <= transport '1' after t_data when exec_phase = '0' and oe = '1' else
		        '0' after t_data when oe = '1' and d_out(i) = '0' else
		        'Z' after t_data;
	end generate;

	----------------------------------------------------------------------
	-- The memory request (unless the address is FE00 to FFFF, or the boot
	-- ROM's), the read (in data_phase) and the write.
	-- @hw die: ZLYZ (oa211); WAFR, WDIN (oa21)
	----------------------------------------------------------------------
	rd_i <= (c.fetch or c.mread) and data_phase and oe;

	-- The gate adds about 8 ns: in phase A, the address (precharged to 0)
	-- reaches the chip's latches before the request falls.
	ctl_mreq_i <= transport c.mreq after t_control;
	ctl_mreq   <= ctl_mreq_i;
	mreq       <= transport ((not shadow_access) or shadow_override) and not internal_access
	              and ctl_mreq_i after t_mreq_gate;
	rd       <= transport rd_i after t_control;
	wr       <= transport c.mwrite after t_control;
	m1       <= transport c.fetch after t_control;
	halt_n   <= transport not halt after t_control;
	stop_n   <= transport not stop after t_control;

	-- The address: the IDU's input, the high byte first.
	-- @hw die: IDU_BIT (buf_a, buf_b)
	a(15 downto 8) <= transport std_logic_vector(idu_in(15 downto 8)) after t_addr_hi when oe = '1' else
	                  (others => 'Z') after t_addr_hi;
	a(7 downto 0)  <= transport std_logic_vector(idu_in(7 downto 0)) after t_addr_lo when oe = '1' else
	                  (others => 'Z') after t_addr_lo;
end architecture;
