-- SPDX-License-Identifier: MIT
--
-- A bus keeper: holds the last value driven onto each line of a bus, weakly,
-- while nobody drives it. The lines of the Game Boy's buses keep their
-- charge for a while when they float; dmg-sim puts keepers on the same pins
-- (its keeper.sv).

library ieee;
use ieee.std_logic_1164.all;

entity keeper is
	generic (
		WIDTH : positive := 1
	);
	port (
		lines : inout std_logic_vector(WIDTH - 1 downto 0)
	);
end entity;

architecture board of keeper is
	signal kept : std_ulogic_vector(WIDTH - 1 downto 0) := (others => 'U');
begin
	keep : process (lines)
	begin
		for i in lines'range loop
			if lines(i) = '0' or lines(i) = '1' then
				kept(i) <= lines(i);
			end if;
		end loop;
	end process;

	hold : for i in lines'range generate
		lines(i) <= 'H' when kept(i) = '1' else
		            'L' when kept(i) = '0' else
		            'Z';
	end generate;
end architecture;
