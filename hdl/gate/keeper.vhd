-- SPDX-License-Identifier: CC-BY-SA-4.0

-- Bus keeper: holds the last value driven on a tri-state net, weakly
-- (dmg-sim's keeper.sv).
--
-- Verilog has two strengths below strong: a pull-up (pull) wins over a
-- keeper (weak). std_logic has one, where the two conflict as W; the keeper
-- then lets go, and takes the value of the pull-up.

library ieee;
use ieee.std_logic_1164.all;

entity keeper is
  port (n : inout std_logic);
end entity;

architecture sim of keeper is
begin
  process (n) is
  begin
    case n is
      when 'Z'       => null;
      when '1' | 'H' => n <= 'H';
      when '0' | 'L' => n <= 'L';
      when 'W'       => n <= 'Z';
      when others    => n <= 'W';
    end case;
  end process;
end architecture;
