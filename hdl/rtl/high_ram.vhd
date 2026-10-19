-- SPDX-License-Identifier: MIT
--
-- High RAM: 127 bytes at FF80 to FFFE.
--
-- @hw die: dmg-schematics, category hram; checked against dmg-sim
-- (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;

package high_ram_types is
	type ram_t is array (0 to 127) of std_ulogic_vector(7 downto 0);
end package;

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.high_ram_types.all;

entity high_ram is
	port (
		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic
	);
end entity;

architecture rtl of high_ram is
	signal ram      : ram_t := (others => (others => '0'));
	signal selected : std_ulogic;
	signal index    : natural range 0 to 127;
begin
	-- @hw die: WALE WOLY WUTA
	selected <= '1' when addr(15 downto 7) = "111111111" and addr /= x"FFFF" else '0';
	index    <= to_integer(to_01(unsigned(addr(6 downto 0))));

	-- A byte takes the data bus for as long as the CPU writes it.
	-- @hw die: ATEF APUH, high_ram
	write : process (all)
	begin
		if selected = '1' and cpu_wr = '1' then
			ram(index) <= to_x01(data);
		end if;
	end process;

	-- @hw die: ABUF APOW, high_ram
	data <= ram(index) when selected = '1' and cpu_rd = '1' else (others => 'Z');
end architecture;
