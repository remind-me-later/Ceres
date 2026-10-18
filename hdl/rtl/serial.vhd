-- SPDX-License-Identifier: MIT
--
-- The serial port: SB (FF01) and SC (FF02), and the pins SIN, SOUT and SCK.
--
-- @hw die: dmg-schematics, category serial (sheet "FF01-FF02 serial");
-- checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity serial is
	port (
		reset2 : in std_ulogic;
		div    : in unsigned(15 downto 0);

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		-- Pins
		sin  : inout std_logic;   -- serial in, with a pull-up
		sout : out   std_ulogic;  -- serial out
		sck  : inout std_logic;   -- the clock: driven when it is internal,
		                          -- with a pull-up when it is external

		-- The test register FF60: bit 0 lets the joypad register drive SIN
		-- and SOUT (sin_drive: Z, 0 or 1; sout_test).
		test_pins : in std_ulogic;
		sin_drive : in std_ulogic;
		sout_test : in std_ulogic;

		int_serial : out std_ulogic  -- a byte is done: request the interrupt
	);
end entity;

architecture rtl of serial is
	signal write_sb, write_sc, read_sb, read_sc : std_ulogic;

	signal sb             : std_ulogic_vector(7 downto 0) := (others => '0');
	signal transfer       : std_ulogic := '0';  -- SC bit 7
	signal internal_clock : std_ulogic := '0';  -- SC bit 0
	signal half           : std_ulogic;         -- unknown at power-on
	signal sck_in         : std_ulogic := '0';
	signal serial_clock   : std_ulogic;
	signal count          : unsigned(3 downto 0) := (others => '0');
	signal ser_out        : std_ulogic := '0';
	signal int_serial_i   : std_ulogic;
begin
	-- @hw die: URYS UWAM UFEG UCOM, SANO SARE
	write_sb <= '1' when cpu_wr = '1' and addr = x"FF01" else '0';
	write_sc <= '1' when cpu_wr = '1' and addr = x"FF02" else '0';
	read_sb  <= '1' when cpu_rd = '1' and addr = x"FF01" else '0';
	read_sc  <= '1' when cpu_rd = '1' and addr = x"FF02" else '0';

	-- SC is written when the CPU's write ends. A byte done clears bit 7.
	-- @hw die: ETAF CULY (dffr), clocked by UWAM; CABY
	sc_ff : process (write_sc, reset2, int_serial_i)
	begin
		if reset2 = '1' then
			internal_clock <= '0';
			transfer       <= '0';
		else
			if int_serial_i = '1' then
				transfer <= '0';
			elsif falling_edge(write_sc) then
				transfer <= to_x01(data(7));
			end if;
			if falling_edge(write_sc) then
				internal_clock <= to_x01(data(0));
			end if;
		end if;
	end process;

	-- The internal clock is 8192 Hz: a flip-flop that toggles when bit 5 of
	-- the divider falls (16384 Hz). Writing SC clears it.
	-- @hw die: COTY (dffr), clocked by UVYN (16384hz)
	half_ff : process (div(5), write_sc)
	begin
		if write_sc = '1' then
			half <= '0';
		elsif falling_edge(div(5)) then
			half <= not half;
		end if;
	end process;

	-- The SCK pin's input holds its last level while nothing drives it.
	-- @hw die: SCK (pad_bidir_pu_latch)
	sck_in <= to_x01(sck) when to_x01(sck) /= 'X' else unaffected;

	-- The clock of the transfer: high while there is no transfer, and the
	-- shift register shifts when it rises.
	-- @hw die: CAVE (muxi), DAWA, EDYL EPYT DAWE
	serial_clock <= '1' when transfer = '0' else
	                not half when internal_clock = '1' else
	                sck_in;

	-- An internal clock goes out on SCK; an external one comes in, with
	-- the pull-up.
	-- @hw die: KEXU KUJO
	sck <= serial_clock when internal_clock = '1' else 'H';

	-- SB shifts in SIN at the rising edge of the clock, bit 7 first out. The
	-- CPU writes its bits directly, for as long as its write lasts.
	-- @hw die: CUBA DEGU DYRA DOJO DOVU EJAB EROD EDER (dffsr), clocked by
	-- DAWE/EPYT; CUFU DOCU DELA DYGE DOLA ELOK EDEL EFEF (set), COHY DUMO
	-- DYBO DAJU DYLY EHUJ EFAK EGUV (reset)
	shift : process (serial_clock, write_sb, data, reset2)
	begin
		for i in 0 to 7 loop
			if reset2 = '1' or (write_sb = '1' and to_x01(data(i)) = '0') then
				sb(i) <= '0';
			elsif write_sb = '1' and to_x01(data(i)) = '1' then
				sb(i) <= '1';
			elsif rising_edge(serial_clock) then
				if i = 0 then
					sb(i) <= to_x01(sin);
				else
					sb(i) <= sb(i - 1);
				end if;
			end if;
		end loop;
	end process;

	-- SOUT changes when the clock falls, to bit 7 of SB.
	-- @hw die: ELYS (dffr), clocked by EDYL
	out_ff : process (serial_clock, reset2)
	begin
		if reset2 = '1' then
			ser_out <= '0';
		elsif falling_edge(serial_clock) then
			ser_out <= sb(7);
		end if;
	end process;

	-- @hw die: KENA (muxi), SOUT (pad_out)
	sout <= sout_test when test_pins = '1' else ser_out;

	-- @hw die: SIN (pad_bidir_pu): KORE KYWE drive it in test mode
	sin <= sin_drive when test_pins = '1' else 'H';

	-- The clocks of the transfer are counted: the eighth ends it.
	-- @hw die: CAFA CYLO CYDE CALY (dffr), CARO
	counter : process (serial_clock, write_sc, reset2)
	begin
		if write_sc = '1' or reset2 = '1' then
			count <= (others => '0');
		elsif rising_edge(serial_clock) then
			count <= count + 1;
		end if;
	end process;
	int_serial_i <= count(3);
	int_serial   <= int_serial_i;

	-- @hw die: CUGY..ETAK (not_if1), CORE ELUV
	data <= sb when read_sb = '1' else
	        transfer & "ZZZZZZ" & internal_clock when read_sc = '1' else
	        (others => 'Z');
end architecture;
