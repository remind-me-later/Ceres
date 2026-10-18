-- SPDX-License-Identifier: CC-BY-SA-4.0

-- The bytes of a memory, for the testbench to look at.

library ieee;
use ieee.std_logic_1164.all;

package dmg_memory is
  type bytes is array (natural range <>) of std_logic_vector(7 downto 0);
end package;
