-- SPDX-License-Identifier: CC-BY-SA-4.0

-- Drives one bit of a memory onto a bus: a pull-up for a 1 while oe_n is
-- low, a pull-down for a 0 while oe is high.
--
-- The pair of bufif gates of each data bit in dmg-sim's memories
-- (dmg_cpu_b/cells/high_ram.sv and others; Copyright Michael Singer,
-- CC-BY-SA 4.0).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.verilog.all;
use work.dmg_timing.all;

entity dmg_bus_bit is
  generic (T_rise, T_fall : delay_length);
  port (data, oe, oe_n : in std_logic; d : out std_logic := 'Z');
end entity;

architecture sim of dmg_bus_bit is
  constant T_z : delay_length := minimum(T_rise, T_fall);
begin
  -- bufif0 (strong1, highz0) #(T_rise, T_fall) (d, data, oe_n);
  process (data, oe_n) is
    variable v : std_ulogic;
    variable p : std_ulogic := 'U';
  begin
    v := drive(bufif0_v(data, oe_n), strong, highz);
    if v /= p then
      d <= v after gdly(v, T_rise, T_fall, T_z);
      p := v;
    end if;
  end process;
  -- bufif1 (highz1, strong0) #(T_rise, T_fall) (d, data, oe);
  process (data, oe) is
    variable v : std_ulogic;
    variable p : std_ulogic := 'U';
  begin
    v := drive(bufif1_v(data, oe), highz, strong);
    if v /= p then
      d <= v after gdly(v, T_rise, T_fall, T_z);
      p := v;
    end if;
  end process;
end architecture;
