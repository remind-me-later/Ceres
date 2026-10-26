-- SPDX-License-Identifier: MIT
--
-- Clocks and resets (the schematic sheet "Clocks and reset").
--
-- @hw die: dmg-schematics, category clocks; checked against dmg-sim
-- (../check/chip_tb.vhd).
--
-- The CGB's crystal runs at 8 MiHz. Halved, it is the 4 MiHz clock of the
-- PPU and of the APU's dividers (clk4), and in the normal speed of the CPU
-- too; in double speed the CPU runs on the 8 MiHz clock itself, so that its
-- phases, and everything that counts them (the divider, the timer, the
-- serial port, OAM DMA, the cartridge's PHI), go twice as fast. The speed
-- changes only while the oscillator is stopped (by STOP, ../speed_switch.vhd).
-- The 4 MiHz clock has a gate of its own (ppu_ena), which holds it in a
-- switch for its own time: halved apart, it keeps its level while stopped.
-- @hw inferred: Pan Docs ("FF4D — KEY1", "Rendering": dots do not change
-- with the speed; "Audio": APU ticks neither); the clocks' circuit is a
-- guess.

library ieee;
use ieee.std_logic_1164.all;

use work.clocking.all;

entity clocks is
	port (
		-- Pins
		ck1         : in  std_ulogic;  -- the crystal, 8 MiHz
		reset_pin_n : in  std_ulogic;  -- /RESET, high on the Game Boy once powered
		t1, t2      : in  std_ulogic;  -- test pins, low on the Game Boy
		phi         : out std_ulogic;  -- PHI, the M-cycle clock for the cartridge

		-- From the CPU core
		osc_ena : in std_ulogic;  -- the oscillator runs (low in STOP)
		clk_ena : in std_ulogic;  -- the CPU core's clocks run
		mreq    : in std_ulogic;  -- the CPU accesses memory
		write   : in std_ulogic;  -- ... to write

		test_reset : in std_ulogic;  -- from the test register FF60
		div_16hz   : in std_ulogic;  -- the last bit of the divider

		double_speed : in std_ulogic;  -- KEY1 bit 7
		ppu_ena      : in std_ulogic;  -- the 4 MiHz clock runs
		core_hold    : in std_ulogic;  -- HDMA holds the CPU core (in phase A)

		clk   : out std_ulogic;  -- the CPU's clock: 4 MiHz, 8 MiHz in double speed
		phase : out phase_t;     -- the phase of the CPU's M-cycle
		clk4  : out std_ulogic;  -- the 4 MiHz clock, whatever the speed

		-- Clocks for the other blocks
		clk_1mhz      : out std_ulogic;  -- high in phase A: the M-cycle starts
		clk_t4        : out std_ulogic;  -- high in G and H
		data_phase    : out std_ulogic;  -- high in E to H, the second half
		exec_phase    : out std_ulogic;  -- high in B to H
		pch_phase     : out std_ulogic;  -- high in B to F
		cpu_wr_sync   : out std_ulogic;  -- a CPU write, in E to G
		cpu_ext_cs_en : out std_ulogic;  -- a CPU access may select the cartridge, C to H

		-- The same for the CPU core and its accesses, on the core's phases,
		-- which stop while HDMA holds the core
		phase_core      : out phase_t;
		clk_1mhz_core   : out std_ulogic;
		clk_t4_core     : out std_ulogic;
		data_phase_core : out std_ulogic;
		exec_phase_core : out std_ulogic;
		pch_phase_core  : out std_ulogic;

		-- Resets, all active high
		reset         : out std_ulogic;  -- the /RESET pin
		osc_stable    : out std_ulogic;  -- the power-on wait is over
		sys_reset     : out std_ulogic;  -- from /RESET until the power-on wait is over
		cpu_sys_reset : out std_ulogic;  -- sys_reset, one M-cycle later
		reset2        : out std_ulogic   -- both: the reset of most registers
	);
end entity;

architecture rtl of clocks is
	signal osc                    : std_ulogic;
	signal clk4_i                 : std_ulogic := '0';
	signal osc_ppu                : std_ulogic;  -- no rise at power-on, as osc
	signal clk4_ppu               : std_ulogic := '0';
	signal double_sel             : std_ulogic := '0';
	signal clk_in                 : std_ulogic := '0';
	signal afur, alef, apuk, adyk : std_ulogic := '0';
	signal clk_core               : std_ulogic;
	signal afur_c, alef_c, apuk_c, adyk_c : std_ulogic := '0';
	signal phase_c                : phase_t := H;
	signal clk_1mhz_c             : std_ulogic;
	signal phase_i                : phase_t := H;
	signal reset_i                : std_ulogic;
	signal waiting                : std_ulogic;
	signal osc_stable_i           : std_ulogic;
	signal sys_reset_i            : std_ulogic;
	signal cpu_sys_reset_i        : std_ulogic := '0';
	signal clk_1mhz_i             : std_ulogic;
	signal data_phase_i           : std_ulogic;
begin
	-- The oscillator's output, held high when it is stopped.
	-- @hw die: CK1_CK2 (pad_xtal), ANOS AVET ARYS, ATAL
	osc <= ck1 when osc_ena = '1' else '1';

	-- Halved: 4 MiHz.
	-- @hw inferred: guess
	halve : process (osc)
	begin
		if rising_edge(osc) then
			clk4_i <= not clk4_i;
		end if;
	end process;

	-- The CPU's clock takes the new speed while the oscillator is stopped.
	-- @hw inferred: guess
	speed_latch : process (all)
	begin
		if osc_ena = '0' then
			double_sel <= double_speed;
		end if;
	end process;

	clk_in <= '1' when osc_ena = '0' else
	          osc when double_sel = '1' else
	          clk4_i;
	clk    <= clk_in;

	-- The 4 MiHz clock of the PPU and the APU, halved from its own gate.
	-- @hw inferred: guess
	osc_ppu <= ck1 when ppu_ena = '1' else '1';

	halve_ppu : process (osc_ppu)
	begin
		if rising_edge(osc_ppu) then
			clk4_ppu <= not clk4_ppu;
		end if;
	end process;
	clk4 <= clk4_ppu;

	-- Four latches count the phases: AFUR and APUK are open while the clock
	-- is high, ALEF and ADYK while it is low, and each copies the one before
	-- it (AFUR the inverse of ADYK). One of them changes at every edge:
	--
	--   phase  A B C D E F G H
	--   AFUR   1 1 1 1 0 0 0 0
	--   ALEF   0 1 1 1 1 0 0 0
	--   APUK   0 0 1 1 1 1 0 0
	--   ADYK   0 0 0 1 1 1 1 0
	--
	-- The test register can hold them all at 0, in phase H.
	-- @hw die: AFUR ALEF APUK ADYK (drlatch_ee)
	counter : process (all)
	begin
		if test_reset = '1' then
			afur <= '0';
			alef <= '0';
			apuk <= '0';
			adyk <= '0';
		elsif clk_in = '1' then
			afur <= not adyk;
			apuk <= alef;
		else
			alef <= afur;
			adyk <= apuk;
		end if;
	end process;

	with std_ulogic_vector'(afur & alef & apuk & adyk) select phase_i <=
		A when "1000",
		B when "1100",
		C when "1110",
		D when "1111",
		E when "0111",
		F when "0011",
		G when "0001",
		H when others;
	phase <= phase_i;

	-- @hw die: BELE BYJU BALY BOGA (and BUTO, BANE, which do not matter
	-- while the oscillator runs)
	clk_1mhz_i <= '1' when phase_i = A or osc_ena = '0' else '0';
	clk_1mhz   <= clk_1mhz_i;

	-- @hw die: BAPY BERU BUFA BYLY
	clk_t4 <= clk_ena when phase_i = G or phase_i = H else '0';

	-- @hw die: BELU BYRY BUDE BEVA
	data_phase_i <= clk_ena when phase_i >= E else '0';
	data_phase   <= data_phase_i;

	-- @hw die: BUVU BYXO BEDO
	exec_phase <= clk_ena and not clk_1mhz_i;

	-- @hw die: BATE BASU BUKE
	pch_phase <= not clk_ena when phase_i = G or phase_i = H or phase_i = A else '1';

	-- The CPU core's phases: a copy of the latches above, on a clock that
	-- HDMA holds high, in phase A, for whole M-cycles (it rises and falls a
	-- moment after the clock's rise at the start of an M-cycle, while the
	-- clock is high). Outside HDMA they are the same as the others; the
	-- divider, the timer, the serial port, OAM DMA and the APU keep the
	-- others, and count on.
	-- @hw inferred: guess (how HDMA stops the CPU core is not known; it
	-- stops at an opcode fetch, ../hdma.vhd)
	clk_core <= '1' when core_hold = '1' else clk_in;

	core_counter : process (all)
	begin
		if test_reset = '1' then
			afur_c <= '0';
			alef_c <= '0';
			apuk_c <= '0';
			adyk_c <= '0';
		elsif clk_core = '1' then
			afur_c <= not adyk_c;
			apuk_c <= alef_c;
		else
			alef_c <= afur_c;
			adyk_c <= apuk_c;
		end if;
	end process;

	with std_ulogic_vector'(afur_c & alef_c & apuk_c & adyk_c) select phase_c <=
		A when "1000",
		B when "1100",
		C when "1110",
		D when "1111",
		E when "0111",
		F when "0011",
		G when "0001",
		H when others;
	phase_core <= phase_c;

	clk_1mhz_c      <= '1' when phase_c = A or osc_ena = '0' else '0';
	clk_1mhz_core   <= clk_1mhz_c;
	clk_t4_core     <= clk_ena when phase_c = G or phase_c = H else '0';
	data_phase_core <= clk_ena when phase_c >= E else '0';
	exec_phase_core <= clk_ena and not clk_1mhz_c;
	pch_phase_core  <= not clk_ena when phase_c = G or phase_c = H or phase_c = A else '1';

	-- A CPU access's strobes, on the core's phases.
	-- @hw die: AFAS AREV APOV
	cpu_wr_sync <= write when phase_c = E or phase_c = F or phase_c = G else '0';

	-- @hw die: AGUT AWOD ABUZ
	cpu_ext_cs_en <= '1' when (t1 = '1' and t2 = '0') or (mreq = '1' and phase_c >= C) else '0';

	-- @hw die: PHI (pad_out)
	phi <= not data_phase_i;

	-- @hw die: ~RESET (pad_in)
	reset_i <= not reset_pin_n;
	reset   <= reset_i;

	-- After /RESET, and while the oscillator is stopped, the chip waits until
	-- the CPU core starts its clocks. Until then, the power-on wait lasts
	-- until the last bit of the divider is set, 32768 M-cycles (31.25 ms) after
	-- the divider started from 0. The test pins skip it.
	-- @hw die: TUBO (nor_latch), UPYF UCOB, UNUT TABA
	wait_latch : process (all)
	begin
		if clk_ena = '1' or reset_i = '1' or osc_ena = '0' then
			waiting <= not clk_ena;
		end if;
	end process;

	osc_stable_i <= '1' when (t1 xor t2) = '1' or (waiting = '1' and div_16hz = '1') else '0';
	osc_stable   <= osc_stable_i;

	-- @hw die: ASOL (nor_latch), AFAR ALYP
	sys_reset_latch : process (all)
	begin
		if reset_i = '1' then
			sys_reset_i <= '1';
		elsif osc_stable_i = '1' then
			sys_reset_i <= '0';
		end if;
	end process;
	sys_reset <= sys_reset_i;

	-- @hw die: AFER (dffr_cc), clocked by BOGA
	cpu_sys_reset_ff : process (clk_in, test_reset)
	begin
		if test_reset = '1' then
			cpu_sys_reset_i <= '0';
		elsif starts(clk_in, phase_i, A) then
			cpu_sys_reset_i <= sys_reset_i after tco;
		end if;
	end process;
	cpu_sys_reset <= cpu_sys_reset_i;

	-- @hw die: AVOR ALUR
	reset2 <= cpu_sys_reset_i or sys_reset_i;
end architecture;
