-- SPDX-License-Identifier: CC-BY-SA-4.0

-- The video RAM of the Game Boy, an 8 KiB static RAM (LH5164LN-10). /WE
-- has precedence over /OE, and a write lasts as long as /CS and /WE are low.
--
-- As in dmg-sim's testbench (dmg_cpu_b_gameboy.sv; Copyright Michael
-- Singer, CC-BY-SA 4.0).

library ieee;
use ieee.std_logic_1164.all;
use work.dmg_memory.all;
use work.tb_util.all;

entity video_ram is
  port (
    ncs, nwe, noe : in std_logic;
    a             : in std_logic_vector(12 downto 0);
    d             : inout std_logic_vector(7 downto 0)
  );
end entity;

architecture sim of video_ram is
begin
  process (ncs, nwe, noe, a, d) is
    variable mem : bytes(0 to 8191) := noise(8192);
  begin
    if to_x01(ncs) = '0' and to_x01(nwe) = '0' and not is_x(a) then
      if is_x(d) then
        mem(index(a)) := x"A5";
      else
        mem(index(a)) := to_x01(d);
      end if;
    end if;
    if to_x01(ncs) = '0' and to_x01(noe) = '0' and to_x01(nwe) = '1' then
      if is_x(a) then
        d <= (others => 'X');
      else
        d <= mem(index(a));
      end if;
    else
      d <= (others => 'Z');
    end if;
  end process;
end architecture;
