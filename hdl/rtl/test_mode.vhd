-- SPDX-License-Identifier: MIT
--
-- Test mode: the pins T1 and T2, and the test register FF60. On the Game
-- Boy both pins are low, and FF60 cannot be written.
--
-- @hw die: dmg-schematics, category test; checked against dmg-sim
-- (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;

entity test_mode is
	port (
		t1, t2 : in std_ulogic;  -- pins
		reset  : in std_ulogic;  -- the /RESET pin
		reset2 : in std_ulogic;

		-- The CPU's bus
		addr   : in std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		cpu_wr : in std_ulogic;

		t1_only    : out std_ulogic;  -- T1 high, T2 low
		t2_only    : out std_ulogic;  -- T2 high, T1 low
		test_reset : out std_ulogic;  -- both high, and /RESET low
		ff60       : out std_ulogic_vector(1 downto 0)
	);
end entity;

architecture rtl of test_mode is
	signal write_ff60 : std_ulogic;
	signal ff60_i     : std_ulogic_vector(1 downto 0) := (others => '0');
begin
	-- @hw die: T1 T2 (pad_in), UBET UVAR, UNOR UMUT, UPOJ
	t1_only    <= t1 and not t2;
	t2_only    <= t2 and not t1;
	test_reset <= t1 and t2 and reset;

	-- FF60 can only be written with one of the test pins high, and is
	-- written when the CPU's write ends.
	-- @hw die: APER APET, BURO AMUT (dffr), clocked by APER
	write_ff60 <= '1' when cpu_wr = '1' and addr = x"FF60" and (t1 xor t2) = '1' else '0';

	ff60_ff : process (write_ff60, reset2)
	begin
		if reset2 = '1' then
			ff60_i <= (others => '0');
		elsif falling_edge(write_ff60) then
			ff60_i <= to_x01(data(1 downto 0));
		end if;
	end process;
	ff60 <= ff60_i;
end architecture;
