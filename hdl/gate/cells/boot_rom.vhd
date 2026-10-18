-- SPDX-License-Identifier: CC-BY-SA-4.0

-- Boot ROM (0000-00FF while enabled): 256 bytes read from bootrom.bin in the
-- working directory. A row of 64 bytes is selected by the word line, which
-- must stay on one address while it is enabled; the precharged bit lines
-- then discharge for every 0 bit of the addressed byte.
--
-- Ported by hand from dmg-sim's dmg_cpu_b/cells/boot_rom.sv (Copyright Michael
-- Singer, CC-BY-SA 4.0), which models it by behaviour.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.dmg_timing.all;
use work.verilog.all;

entity dmg_boot_rom is
  generic (
    L_d0 : real := work.dmg_boot_rom_param.L_d0; L_d1 : real := work.dmg_boot_rom_param.L_d1;
    L_d2 : real := work.dmg_boot_rom_param.L_d2; L_d3 : real := work.dmg_boot_rom_param.L_d3;
    L_d4 : real := work.dmg_boot_rom_param.L_d4; L_d5 : real := work.dmg_boot_rom_param.L_d5;
    L_d6 : real := work.dmg_boot_rom_param.L_d6; L_d7 : real := work.dmg_boot_rom_param.L_d7;
    W_gate_d0 : real := 0.0; W_gate_d1 : real := 0.0; W_gate_d2 : real := 0.0; W_gate_d3 : real := 0.0;
    W_gate_d4 : real := 0.0; W_gate_d5 : real := 0.0; W_gate_d6 : real := 0.0; W_gate_d7 : real := 0.0
  );
  port (
    d0, d1, d2, d3, d4, d5, d6, d7 : out std_logic;
    row0_n, row1_n, row2_n, row3_n : in std_logic;
    col0, col1, col2, col3 : in std_logic;
    a2, a3, a6, a7, a2_n, a3_n, a6_n, a7_n : in std_logic;
    oe, oe_n, bl_pch_n, wl_pch, wl_ena_n : in std_logic
  );
end entity;

architecture sim of dmg_boot_rom is
  type rom_t is array (0 to 255) of std_logic_vector(7 downto 0);

  impure function load return rom_t is
    type bin_file is file of character;
    file f : bin_file;
    variable status : file_open_status;
    variable c : character;
    variable rom : rom_t := (others => (others => '0'));
  begin
    file_open(status, f, "bootrom.bin", read_mode);
    if status /= open_ok then
      report "cannot open bootrom.bin, using an all-zero boot ROM" severity warning;
      return rom;
    end if;
    for i in rom'range loop
      exit when endfile(f);
      read(f, c);
      rom(i) := std_logic_vector(to_unsigned(character'pos(c), 8));
    end loop;
    file_close(f);
    return rom;
  end function;

  constant mem : rom_t := load;

  -- A 2-bit field selected when a = not a_n, as an integer; -1 otherwise.
  function field(hi, lo, hi_n, lo_n : std_logic) return integer is
  begin
    if is_x(std_logic_vector'(hi & lo & hi_n & lo_n)) or to_x01(hi) = to_x01(hi_n) or to_x01(lo) = to_x01(lo_n) then
      return -1;
    end if;
    return to_integer(unsigned'(to_x01(hi) & to_x01(lo)));
  end function;

  -- The delays of a data line, from its length and the gates on it, as
  -- dmg-sim computes them for this cell.
  function t_rise(l, w : real) return delay_length is
  begin
    return to_t(tpd_elmore(l, 2.0 * R_pmos_ohm(136.0 * L_unit), C_gate_F(w)));
  end function;
  function t_fall(l, w : real) return delay_length is
  begin
    return to_t(tpd_elmore(l, 2.0 * R_nmos_ohm(100.0 * L_unit), C_gate_F(w)));
  end function;

  signal wl_adr_valid : std_logic := '0';
  signal wl_adr, adr : integer := -1;
  signal bl_data : std_logic_vector(7 downto 0) := (others => '0');
begin
  -- wl_adr = (({a3, a2} == ~{a3_n, a2_n}) && wl_adr_valid) ? {a3, a2, 2'b00} : -1;
  process (a2, a3, a2_n, a3_n, wl_adr_valid) is
    variable f : integer;
  begin
    f := field(a3, a2, a3_n, a2_n);
    if f /= -1 and to_x01(wl_adr_valid) = '1' then wl_adr <= f * 4; else wl_adr <= -1; end if;
  end process;

  -- adr = cm_adr | col_adr | wl_adr | row_adr;
  process (a6, a7, a6_n, a7_n, row0_n, row1_n, row2_n, row3_n, col0, col1, col2, col3, wl_adr) is
    variable cm, row, col : integer;
  begin
    cm := field(a7, a6, a7_n, a6_n);
    case std_logic_vector'(to_x01(row3_n) & to_x01(row2_n) & to_x01(row1_n) & to_x01(row0_n)) is
      when "1110" => row := 0;
      when "1101" => row := 1;
      when "1011" => row := 2;
      when "0111" => row := 3;
      when others => row := -1;
    end case;
    case std_logic_vector'(to_x01(col3) & to_x01(col2) & to_x01(col1) & to_x01(col0)) is
      when "0001" => col := 16#00#;
      when "0010" => col := 16#10#;
      when "0100" => col := 16#20#;
      when "1000" => col := 16#30#;
      when others => col := -1;
    end case;
    if cm = -1 or row = -1 or col = -1 or wl_adr = -1 then
      adr <= -1;
    else
      adr <= cm * 64 + col + wl_adr + row;
    end if;
  end process;

  -- The word line holds the first address it sees until precharged.
  process (wl_pch, wl_ena_n, wl_adr) is
    variable last_wl_adr : integer := -1;
  begin
    if to_x01(wl_pch) = '1' then
      last_wl_adr := -1;
      wl_adr_valid <= '1';
    elsif to_x01(wl_ena_n) = '0' then
      if last_wl_adr = -1 and wl_adr /= -1 then
        last_wl_adr := wl_adr;
      elsif wl_adr = -1 or last_wl_adr /= wl_adr then
        wl_adr_valid <= '0';
      end if;
    end if;
  end process;

  -- always_latch if (!bl_pch_n) bl_data = '1; else bl_data &= mem[adr] (or 'x);
  process (bl_pch_n, adr) is
    variable data : std_logic_vector(7 downto 0) := (others => '0');
  begin
    if to_x01(bl_pch_n) = '0' then
      data := (others => '1');
    elsif to_x01(bl_pch_n) = '1' then
      if adr /= -1 then data := data and mem(adr); else data := data and "XXXXXXXX"; end if;
    end if;
    bl_data <= data;
  end process;

  -- Each data line is driven by a pair of tri-state gates.
  drive_d0 : entity work.dmg_bus_bit
    generic map (t_rise(L_d0, W_gate_d0), t_fall(L_d0, W_gate_d0))
    port map (bl_data(0), oe, oe_n, d0);
  drive_d1 : entity work.dmg_bus_bit
    generic map (t_rise(L_d1, W_gate_d1), t_fall(L_d1, W_gate_d1))
    port map (bl_data(1), oe, oe_n, d1);
  drive_d2 : entity work.dmg_bus_bit
    generic map (t_rise(L_d2, W_gate_d2), t_fall(L_d2, W_gate_d2))
    port map (bl_data(2), oe, oe_n, d2);
  drive_d3 : entity work.dmg_bus_bit
    generic map (t_rise(L_d3, W_gate_d3), t_fall(L_d3, W_gate_d3))
    port map (bl_data(3), oe, oe_n, d3);
  drive_d4 : entity work.dmg_bus_bit
    generic map (t_rise(L_d4, W_gate_d4), t_fall(L_d4, W_gate_d4))
    port map (bl_data(4), oe, oe_n, d4);
  drive_d5 : entity work.dmg_bus_bit
    generic map (t_rise(L_d5, W_gate_d5), t_fall(L_d5, W_gate_d5))
    port map (bl_data(5), oe, oe_n, d5);
  drive_d6 : entity work.dmg_bus_bit
    generic map (t_rise(L_d6, W_gate_d6), t_fall(L_d6, W_gate_d6))
    port map (bl_data(6), oe, oe_n, d6);
  drive_d7 : entity work.dmg_bus_bit
    generic map (t_rise(L_d7, W_gate_d7), t_fall(L_d7, W_gate_d7))
    port map (bl_data(7), oe, oe_n, d7);
end architecture;
