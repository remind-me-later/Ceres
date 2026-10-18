-- SPDX-License-Identifier: CC-BY-SA-4.0

-- High RAM (FF80-FFFE): an SRAM on the CPU data bus.
--
-- Ported by hand from dmg-sim's dmg_cpu_b/cells/high_ram.sv (Copyright Michael
-- Singer, CC-BY-SA 4.0), which models it by behaviour.

library ieee;
use ieee.std_logic_1164.all;
use work.dmg_timing.all;
use work.verilog.all;

entity dmg_high_ram is
  generic (
    L_d0 : real := work.dmg_high_ram_param.L_d0; L_d1 : real := work.dmg_high_ram_param.L_d1;
    L_d2 : real := work.dmg_high_ram_param.L_d2; L_d3 : real := work.dmg_high_ram_param.L_d3;
    L_d4 : real := work.dmg_high_ram_param.L_d4; L_d5 : real := work.dmg_high_ram_param.L_d5;
    L_d6 : real := work.dmg_high_ram_param.L_d6; L_d7 : real := work.dmg_high_ram_param.L_d7;
    W_gate_d0 : real := work.dmg_high_ram_param.W_gate_d0; W_gate_d1 : real := work.dmg_high_ram_param.W_gate_d1;
    W_gate_d2 : real := work.dmg_high_ram_param.W_gate_d2; W_gate_d3 : real := work.dmg_high_ram_param.W_gate_d3;
    W_gate_d4 : real := work.dmg_high_ram_param.W_gate_d4; W_gate_d5 : real := work.dmg_high_ram_param.W_gate_d5;
    W_gate_d6 : real := work.dmg_high_ram_param.W_gate_d6; W_gate_d7 : real := work.dmg_high_ram_param.W_gate_d7
  );
  port (
    d0, d1, d2, d3, d4, d5, d6, d7 : inout std_logic;
    col0, col1, col2, col3 : in std_logic;
    a2, a3, a4, a5, a6, a2_n, a3_n, a4_n, a5_n, a6_n : in std_logic;
    oe, oe_n, wr, bl_pch_n, wldrv_pch_n, wldrv_ena : in std_logic
  );
end entity;

architecture sim of dmg_high_ram is
  -- The delays of a data line, from its length and the gates on it, as
  -- dmg-sim computes them for this cell.
  function t_rise(l, w : real) return delay_length is
  begin
    return to_t(tpd_elmore(l, 2.0 * R_pmos_ohm(120.0 * L_unit), C_gate_F(w)));
  end function;
  function t_fall(l, w : real) return delay_length is
  begin
    return to_t(tpd_elmore(l, 2.0 * R_nmos_ohm(80.0 * L_unit), C_gate_F(w)));
  end function;
  signal dout : std_logic_vector(7 downto 0);
begin
  sram : entity work.dmg_simplified_sram
    generic map (N => 128, AW => 7)
    port map (
      din => (d7, d6, d5, d4, d3, d2, d1, d0), dout => dout,
      col => (col3, col2, col1, col0),
      a => (a6, a5, a4, a3, a2), a_n => (a6_n, a5_n, a4_n, a3_n, a2_n),
      wr => wr, bl_pch_n => bl_pch_n, wldrv_pch_n => wldrv_pch_n, wldrv_ena => wldrv_ena
    );
  -- Each data line is driven by a pair of tri-state gates.
  drive_d0 : entity work.dmg_bus_bit
    generic map (t_rise(L_d0, W_gate_d0), t_fall(L_d0, W_gate_d0))
    port map (dout(0), oe, oe_n, d0);
  drive_d1 : entity work.dmg_bus_bit
    generic map (t_rise(L_d1, W_gate_d1), t_fall(L_d1, W_gate_d1))
    port map (dout(1), oe, oe_n, d1);
  drive_d2 : entity work.dmg_bus_bit
    generic map (t_rise(L_d2, W_gate_d2), t_fall(L_d2, W_gate_d2))
    port map (dout(2), oe, oe_n, d2);
  drive_d3 : entity work.dmg_bus_bit
    generic map (t_rise(L_d3, W_gate_d3), t_fall(L_d3, W_gate_d3))
    port map (dout(3), oe, oe_n, d3);
  drive_d4 : entity work.dmg_bus_bit
    generic map (t_rise(L_d4, W_gate_d4), t_fall(L_d4, W_gate_d4))
    port map (dout(4), oe, oe_n, d4);
  drive_d5 : entity work.dmg_bus_bit
    generic map (t_rise(L_d5, W_gate_d5), t_fall(L_d5, W_gate_d5))
    port map (dout(5), oe, oe_n, d5);
  drive_d6 : entity work.dmg_bus_bit
    generic map (t_rise(L_d6, W_gate_d6), t_fall(L_d6, W_gate_d6))
    port map (dout(6), oe, oe_n, d6);
  drive_d7 : entity work.dmg_bus_bit
    generic map (t_rise(L_d7, W_gate_d7), t_fall(L_d7, W_gate_d7))
    port map (dout(7), oe, oe_n, d7);
end architecture;
