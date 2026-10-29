-- SPDX-License-Identifier: MIT
--
-- The SM83's sequencer: the opcode register (IR), the M-cycle counter, the
-- CB prefix, which of W and Z a read goes to, HALT and STOP, and the
-- interrupt master enable (IME) with the entry into an interrupt.
--
-- The M-cycle counter takes the next M-cycle from the decoders at the start
-- of every M-cycle (the rise of clk, in phase A). M-cycle 7 is special: the
-- counter goes there when a condition fails (JR cc, JP cc, CALL cc, RET cc)
-- and while the CPU is idle (halted, or in reset), and its step fetches the
-- next opcode.
--
-- @hw die: dmg-schematics, categories seq, seq-irq and reg-ir

library ieee;
use ieee.std_logic_1164.all;

use work.clocking.all;
use work.sm83_decoder.all;
use work.sm83_controls.all;
use work.revisions.all;

entity sm83_sequencer is
	generic (
		REVISION : revision_t := DMG_B
	);
	port (
		clk         : in std_ulogic;  -- high in phase A
		exec_phase  : in std_ulogic;  -- B to H
		data_phase  : in std_ulogic;  -- E to H
		write_phase : in std_ulogic;  -- G and H
		sys_reset   : in std_ulogic;
		pwron_reset : in std_ulogic;
		clk_ready   : in std_ulogic;  -- the oscillator is stable
		wake        : in std_ulogic;  -- a joypad line went low (ends STOP)
		nmi         : in std_ulogic;  -- unused on the DMG: always low
		test_freeze : in std_ulogic;  -- unused on the DMG: always low

		s           : in steps_t;
		c           : in controls_t;
		db          : in std_logic_vector(7 downto 0);  -- the core's data bus
		alu_cond_fail : in std_ulogic;  -- the condition of JR/JP/CALL/RET cc fails
		int_take    : in std_ulogic;  -- an interrupt is enabled and requested
		int_pending : in std_ulogic;  -- an interrupt is requested (for HALT)
		int_now     : in std_ulogic := '0';  -- ... before the latch of data_phase (the CGB's)

		opcode      : out std_ulogic_vector(7 downto 0);
		mcyc        : out std_ulogic_vector(2 downto 0);
		table_cb    : out std_ulogic;  -- the opcode came after the CB prefix
		in_intr     : out std_ulogic;  -- the decoder's "int": the interrupt's entry
		sel_reg_w   : out std_ulogic;  -- a read goes to W
		sel_reg_z   : out std_ulogic;  -- a read goes to Z
		halt        : out std_ulogic;
		stop        : out std_ulogic;
		int_entry   : out std_ulogic;
		nmi_entry   : out std_ulogic;
		ime         : out std_ulogic
	);
end entity;

architecture rtl of sm83_sequencer is
	signal ir_next      : std_ulogic_vector(7 downto 0) := (others => '0');
	signal opcode_i     : std_ulogic_vector(7 downto 0) := (others => '0');
	signal mcyc_i       : std_ulogic_vector(2 downto 0) := (others => '0');
	signal set_mcyc7    : std_ulogic;
	signal sel_reg_w_i  : std_ulogic := '0';  -- XDQF
	signal cb_next      : std_ulogic := '0';
	signal cb_flag      : std_ulogic := '0';  -- XURG
	signal idle         : std_ulogic;
	signal boundary     : std_ulogic;  -- XOGS: an interrupt may start

	-- HALT and STOP
	signal halt_i       : std_ulogic := '0';  -- YNKW
	signal stop_i       : std_ulogic := '0';  -- ZUMN
	signal halt_delayed : std_ulogic := '0';  -- YSBT
	signal stop_delayed : std_ulogic := '0';  -- ZKAI
	signal halt_set     : std_ulogic;
	signal halt_reset   : std_ulogic;
	signal any_reset    : std_ulogic;
	signal pending_delayed : std_ulogic := '0';  -- YOII
	signal pending_at_start : std_ulogic := '0';
	signal not_ready    : std_ulogic;  -- ZIKS
	signal not_ready_1  : std_ulogic := '1';  -- ZORP
	signal not_ready_2  : std_ulogic := '1';  -- ZAZA

	-- IME and the interrupt's entry
	signal ime_i        : std_ulogic := '0';  -- ~ZIVV
	signal ime_state    : std_ulogic := '0';  -- ZJJE
	signal ime_set      : std_ulogic;
	signal ime_reset    : std_ulogic;
	signal no_nmi       : std_ulogic := '1';  -- ZRSY
	signal no_nmi_1     : std_ulogic := '1';  -- ZKDU
	signal entering     : std_ulogic := '0';  -- ZKOG
	signal int_start    : std_ulogic;  -- ZAIJ
	signal int_entry_i  : std_ulogic := '0';  -- ZACW
	signal not_di       : std_ulogic;  -- ZZOM

	-- NMI (the pin is not bonded out on the DMG)
	signal nmi_low_1    : std_ulogic := '1';  -- YDXA
	signal nmi_low_2    : std_ulogic := '1';  -- ~YODP
	signal nmi_rise     : std_ulogic;  -- YNIU
	signal nmi_req      : std_ulogic := '0';  -- ZOJZ
	signal nmi_entering : std_ulogic := '0';  -- ZLOZ
	signal nmi_entry_i  : std_ulogic := '0';  -- ZRBY
	signal nmi_clear    : std_ulogic;  -- ~YNEU
begin
	----------------------------------------------------------------------
	-- The opcode register takes the data bus in write_phase while the CPU
	-- fetches; the new opcode shows when the fetch ends (in phase A).
	-- @hw die: REG_IR (dff_cc_ee_q_x2_reg_bit), REG_IR_NOT_WE
	----------------------------------------------------------------------
	ir_capture : process (all)
	begin
		if c.fetch = '1' and write_phase = '1' then
			ir_next <= to_x01(db);
		end if;
	end process;

	ir_update : process (c.fetch)
	begin
		if falling_edge(c.fetch) then
			opcode_i <= ir_next after tco;
		end if;
	end process;
	opcode <= opcode_i;

	----------------------------------------------------------------------
	-- The M-cycle counter. It goes to 7 when a condition fails or while the
	-- CPU is idle. W and Z: two reads in a row go to Z, then W.
	-- @hw die: XAYM XIRF XUFU (dff), WUDZ WEEN WERF, WAGR; XDQF (dff), VEQU
	----------------------------------------------------------------------
	idle      <= halt_i or sys_reset or test_freeze;
	set_mcyc7 <= idle or alu_cond_fail;

	m_cycles : process (clk)
	begin
		if rising_edge(clk) then
			mcyc_i      <= c.next_mcyc or (2 downto 0 => set_mcyc7) after tco;
			sel_reg_w_i <= c.mread and not sel_reg_w_i after tco;
		end if;
	end process;
	mcyc      <= mcyc_i;
	sel_reg_w <= sel_reg_w_i;
	sel_reg_z <= not sel_reg_w_i;

	-- The CB prefix: taken while fetching, in write_phase, and on when the
	-- fetch ends (the output follows while the CPU does not fetch); HALT
	-- clears it. It counts only in M-cycles 0 to 3.
	-- @hw die: XURG (dffre), XURI, XTIP, XUDO
	cb_capture : process (all)
	begin
		if halt_i = '1' then
			cb_next <= '0';
		elsif c.fetch = '1' and write_phase = '1' then
			cb_next <= s.cb_prefix;
		end if;
	end process;

	cb_update : process (all)
	begin
		if c.fetch = '0' then
			cb_flag <= cb_next;
		end if;
	end process;
	table_cb <= (cb_flag and not mcyc_i(2)) or test_freeze;

	-- An instruction ends where it fetches the next opcode (but not after
	-- the CB prefix): an interrupt may start there, or while idle.
	-- @hw die: XOGS
	boundary <= (data_phase and c.fetch and not s.cb_prefix) or idle;

	----------------------------------------------------------------------
	-- HALT and STOP. HALT (and STOP, and the power-on reset) set halt, one
	-- M-cycle after their step; an interrupt request clears it, and so does
	-- the oscillator becoming stable after the reset. STOP stops the
	-- oscillator until a joypad line goes low.
	--
	-- The CGB takes the request for HALT as it is at an M-cycle's start,
	-- not as the requests' latch holds it from the M-cycle's middle (phase
	-- E): a request in an M-cycle's second half wakes it an M-cycle later
	-- than the DMG.
	-- @hw inferred: ceres (from SameBoy's GB_cpu_run: the DMG samples the
	-- interrupts 2 dots into a halted step, the CGB at its start); with it,
	-- cgb-acid2's and Gambatte's HALTs end where ceres's do
	-- @hw die: YSBT ZKAI YOII ZORP ZAZA (dff), YNOZ, ZIKS ZUDN, YOLU, ZHZO
	-- ZKON, YKUA, YNKW (srlatch), ZUMN (srlatch), ZWLM, XWEE ZJBF, YCNF
	----------------------------------------------------------------------
	delays : process (clk)
	begin
		if rising_edge(clk) then
			halt_delayed    <= s.halt after tco;
			stop_delayed    <= c.stop after tco;
			if REVISION = DMG_B then
				pending_delayed <= int_pending after tco;
			else
				pending_at_start <= int_now after tco;
				pending_delayed  <= pending_at_start after tco;
			end if;
			not_ready_1     <= not_ready after tco;
			not_ready_2     <= not_ready_1 after tco;
		end if;
	end process;

	any_reset  <= sys_reset or pwron_reset;
	not_ready  <= not (clk_ready and not sys_reset);
	halt_set   <= pwron_reset or stop_delayed or halt_delayed;
	halt_reset <= ((not not_ready_1 and not_ready_2) or pending_delayed) and not any_reset;

	halt_latch : process (all)
	begin
		if halt_reset = '1' then
			halt_i <= '0';
		elsif halt_set = '1' then
			halt_i <= '1';
		end if;
	end process;
	halt <= halt_i;

	stop_latch : process (all)
	begin
		if wake = '1' or any_reset = '1' then
			stop_i <= '0';
		elsif stop_delayed = '1' then
			stop_i <= '1';
		end if;
	end process;
	stop <= stop_i;

	----------------------------------------------------------------------
	-- IME: EI sets it, DI clears it (in data_phase of their step), RETI
	-- sets it, the interrupt's entry clears it; it takes effect at the end
	-- of the M-cycle (the fall of exec_phase).
	-- @hw die: ZJJE (srlatch), ZBPP, ZWUU, ZIVV (dff), ZOXC, ZRSY
	-- (srlatch), ZNDA ZGNA, ZKDU (dff)
	----------------------------------------------------------------------
	ime_set   <= data_phase and ((opcode_i(3) and s.di_or_ei) or (c.reti_m3 and no_nmi_1));
	ime_reset <= (data_phase and ((not opcode_i(3) and s.di_or_ei) or (no_nmi_1 and int_entry_i)))
	             or sys_reset;

	ime_latch : process (all)
	begin
		if ime_reset = '1' then
			ime_state <= '0';
		elsif ime_set = '1' then
			ime_state <= '1';
		end if;
	end process;

	-- Not entering an NMI: set by the reset and by RETI.
	no_nmi_latch : process (all)
	begin
		if nmi_entering = '1' then
			no_nmi <= '0';
		elsif sys_reset = '1' or c.reti_m3 = '1' then
			no_nmi <= '1';
		end if;
	end process;

	ime_ff : process (exec_phase)
	begin
		if falling_edge(exec_phase) then
			ime_i <= no_nmi and ime_state after tco;
		end if;
	end process;
	ime <= ime_i;

	----------------------------------------------------------------------
	-- The interrupt's entry: when IME is set and an interrupt is requested
	-- at the end of an instruction (but not during DI), the next M-cycle
	-- starts the entry (in_intr: the decoder's int_entry steps).
	-- @hw die: ZAIJ, ZAOC, ZZOM, ZKOG (srlatch), ZOWA, ZFEX, ZACW (dff),
	-- XYGB
	----------------------------------------------------------------------
	not_di    <= not (not opcode_i(3) and s.di_or_ei);
	int_start <= ime_i and data_phase and int_take and boundary and not_di;

	entering_latch : process (all)
	begin
		if s.int_entry_m6 = '1' or sys_reset = '1' then
			entering <= '0';
		elsif int_start = '1' then
			entering <= '1';
		end if;
	end process;

	entry_ff : process (clk)
	begin
		if rising_edge(clk) then
			int_entry_i <= entering or nmi_entering after tco;
			no_nmi_1    <= no_nmi after tco;
			nmi_low_1   <= not nmi after tco;
			nmi_entry_i <= nmi_entering after tco;
		end if;
	end process;
	int_entry <= int_entry_i;
	nmi_entry <= nmi_entry_i;

	in_intr <= test_freeze or idle or int_entry_i;

	----------------------------------------------------------------------
	-- NMI: a rise of nmi, at the end of an instruction, enters it. On the
	-- DMG, nmi is always low.
	-- @hw die: YEPJ, YDXA (dff), YODP (dlatch), YNIU, ZOJZ ZLOZ (srlatch),
	-- ZYOC, YNEU, ZRBY (dff)
	----------------------------------------------------------------------
	nmi_low_latch : process (all)
	begin
		if clk = '0' then
			nmi_low_2 <= nmi_low_1;
		end if;
	end process;
	nmi_rise  <= nmi_low_2 and nmi;
	nmi_clear <= (nmi_entry_i and s.int_entry_m6) or sys_reset;

	nmi_latches : process (all)
	begin
		if nmi_clear = '1' then
			nmi_req      <= '0';
			nmi_entering <= '0';
		else
			if nmi_rise = '1' then
				nmi_req <= '1';
			end if;
			if boundary = '1' and no_nmi_1 = '1' and nmi_req = '1' then
				nmi_entering <= '1';
			end if;
		end if;
	end process;
end architecture;
