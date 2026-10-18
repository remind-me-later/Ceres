-- SPDX-License-Identifier: CC-BY-SA-4.0

-- Wave RAM (FF30-FF3F): an SRAM with separate data in and out.
--
-- Ported by hand from dmg-sim's dmg_cpu_b/cells/wave_ram.sv (Copyright Michael
-- Singer, CC-BY-SA 4.0), which models it by behaviour.

library ieee;
use ieee.std_logic_1164.all;
use work.dmg_timing.all;
use work.verilog.all;

entity dmg_wave_ram is
  generic (
    L_dout0 : real := work.dmg_wave_ram_param.L_dout0; L_dout1 : real := work.dmg_wave_ram_param.L_dout1;
    L_dout2 : real := work.dmg_wave_ram_param.L_dout2; L_dout3 : real := work.dmg_wave_ram_param.L_dout3;
    L_dout4 : real := work.dmg_wave_ram_param.L_dout4; L_dout5 : real := work.dmg_wave_ram_param.L_dout5;
    L_dout6 : real := work.dmg_wave_ram_param.L_dout6; L_dout7 : real := work.dmg_wave_ram_param.L_dout7;
    W_gate_dout0 : real := 0.0; W_gate_dout1 : real := 0.0; W_gate_dout2 : real := 0.0; W_gate_dout3 : real := 0.0;
    W_gate_dout4 : real := 0.0; W_gate_dout5 : real := 0.0; W_gate_dout6 : real := 0.0; W_gate_dout7 : real := 0.0
  );
  port (
    din0, din1, din2, din3, din4, din5, din6, din7 : in std_logic;
    dout0, dout1, dout2, dout3, dout4, dout5, dout6, dout7 : out std_logic;
    col0, col1, col2, col3 : in std_logic;
    a2, a3, a2_n, a3_n : in std_logic;
    wr, bl_pch_n, wldrv_pch_n : in std_logic
  );
end entity;

architecture sim of dmg_wave_ram is
  type reals is array (0 to 7) of real;
  constant L : reals := (L_dout0, L_dout1, L_dout2, L_dout3, L_dout4, L_dout5, L_dout6, L_dout7);
  constant W : reals := (W_gate_dout0, W_gate_dout1, W_gate_dout2, W_gate_dout3,
                         W_gate_dout4, W_gate_dout5, W_gate_dout6, W_gate_dout7);
  signal dout, q : std_logic_vector(7 downto 0);
begin
  sram : entity work.dmg_simplified_sram
    generic map (N => 16, AW => 4)
    port map (
      din => (din7, din6, din5, din4, din3, din2, din1, din0), dout => dout,
      col => (col3, col2, col1, col0),
      a => (a3, a2), a_n => (a3_n, a2_n),
      wr => wr, bl_pch_n => bl_pch_n, wldrv_pch_n => wldrv_pch_n, wldrv_ena => wldrv_pch_n
    );
  -- assign #(T_rise_dout, T_fall_dout) dout = dout[i];
  bits : for i in 0 to 7 generate
    constant T_rise : delay_length := to_t(tpd_elmore(L(i), R_pmos_ohm(120.0 * L_unit), C_gate_F(W(i))));
    constant T_fall : delay_length := to_t(tpd_elmore(L(i), R_nmos_ohm(80.0 * L_unit), C_gate_F(W(i))));
  begin
    process (dout(i)) is
      variable v : std_ulogic;
      variable p : std_ulogic := 'U';
    begin
      v := to_x01z(dout(i));
      if v /= p then
        q(i) <= v after gdly(v, T_rise, T_fall, minimum(T_rise, T_fall));
        p := v;
      end if;
    end process;
  end generate;
  (dout7, dout6, dout5, dout4, dout3, dout2, dout1, dout0) <= q;
end architecture;
