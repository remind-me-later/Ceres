-- SPDX-License-Identifier: CC-BY-SA-4.0

-- The work RAM of the Game Boy, 8 KiB at C000-DFFF on the cartridge bus,
-- written on the rising edge of /WR.
--
-- As in dmg-sim's testbench (dmg_cpu_b_gameboy.sv; Copyright Michael
-- Singer, CC-BY-SA 4.0).

library ieee;
use ieee.std_logic_1164.all;
use work.dmg_memory.all;
use work.tb_util.all;

entity work_ram is
  port (
    ncs, nrd, nwr : in std_logic;
    a             : in std_logic_vector(15 downto 0);
    d             : inout std_logic_vector(7 downto 0)
  );
end entity;

architecture sim of work_ram is
begin
  process (ncs, nrd, nwr, a) is
    variable mem : bytes(0 to 8191) := noise(8192);
  begin
    -- Selected by /CS and A14.
    if rising_edge(nwr) and to_x01(ncs) = '0' and to_x01(a(14)) = '1' and not is_x(a(12 downto 0)) then
      if is_x(d) then
        mem(index(a(12 downto 0))) := x"5A";
      else
        mem(index(a(12 downto 0))) := to_x01(d);
      end if;
    end if;
    if to_x01(ncs) = '0' and to_x01(a(14)) = '1' and to_x01(nrd) = '0' and to_x01(nwr) = '1' then
      if is_x(a(12 downto 0)) then
        d <= (others => 'X');
      else
        d <= mem(index(a(12 downto 0)));
      end if;
    else
      d <= (others => 'Z');
    end if;
  end process;
end architecture;
