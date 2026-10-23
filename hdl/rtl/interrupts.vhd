-- SPDX-License-Identifier: MIT
--
-- The interrupt flags, IF (FF0F): one flag per source, which the CPU core
-- sees as its interrupt requests. The enable bits (IE, FFFF) are in the
-- CPU core.
--
-- @hw die: dmg-schematics, category int (sheet "FF0F interrupts"); checked
-- against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;

use work.clocking.all;

entity interrupts is
	port (
		reset2 : in std_ulogic;

		-- The sources: VBlank, STAT, timer, serial, joypad, in IF's order.
		-- A flag is set when its source rises.
		requests : in std_ulogic_vector(4 downto 0);

		-- The CPU core takes the interrupt: it clears the flag.
		acks : in std_ulogic_vector(4 downto 0);

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		flags : out std_ulogic_vector(4 downto 0)  -- to the CPU core
	);
end entity;

architecture rtl of interrupts is
	signal write_if, read_if : std_ulogic;
	signal flags_i           : std_ulogic_vector(4 downto 0) := (others => '0');
	signal read_value        : std_ulogic_vector(4 downto 0) := (others => '0');
begin
	-- @hw die: REFA ROLO, SEMY SAPA
	write_if <= '1' when cpu_wr = '1' and addr = x"FF0F" else '0';
	read_if  <= '1' when cpu_rd = '1' and addr = x"FF0F" else '0';

	-- Each flag is a flip-flop set by the rising edge of its source, and
	-- also set or cleared directly, for as long as the CPU writes IF with
	-- the bit set or clear. The CPU's acknowledge and the reset clear it.
	-- Clearing wins over setting.
	-- @hw die: LOPE LALU NYBO UBUL ULAK (dffsr), MYZU MODY PYHU TOME TOGA
	-- (set), LYTA MOVU PYGA TUNY TYME (reset)
	flag : for i in 0 to 4 generate
		ff : process (requests(i), write_if, data(i), acks(i), reset2)
		begin
			if reset2 = '1' or acks(i) = '1' or (write_if = '1' and to_x01(data(i)) = '0') then
				flags_i(i) <= '0';
			elsif write_if = '1' and to_x01(data(i)) = '1' then
				flags_i(i) <= '1';
			elsif rising_edge(requests(i)) then
				flags_i(i) <= '1' after tco;
			end if;
		end process;
	end generate;
	flags <= flags_i;

	-- A read sees the flags as they were when it started.
	-- @hw die: MATY MOPO PAVY NEJY NUTY (dlatch), POLA, NELA..PEGY (not_if1)
	read_latch : process (all)
	begin
		if read_if = '0' then
			read_value <= flags_i;
		end if;
	end process;

	data_out <= "ZZZ" & read_value when read_if = '1' else (others => 'Z');
end architecture;
