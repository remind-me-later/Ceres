-- The cells of the DMG-CPU B that are not made of gates: the memories, the
-- boot ROM and the analog audio.
--
-- Ported by hand from dmg-sim (dmg_cpu_b/cells: simplified_sram.sv,
-- high_ram.sv, oam.sv, wave_ram.sv, boot_rom.sv, audio.sv; Copyright
-- Michael Singer, CC-BY-SA 4.0), which models them by behaviour. As in
-- dmg-sim's default build, OAM and wave RAM use the simplified SRAM, which
-- cannot reproduce the OAM corruption bug nor the wave RAM corruption of
-- Blargg's dmg_sound test 10.
--
-- Where dmg-sim computes addresses with 2-state integers, an unknown bit
-- (only seen while the chip powers up) makes the address invalid here.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.verilog.all;
use work.dmg_timing.all;

-- Drives one bit of a memory onto a bus: a pull-up for a 1 while oe_n is
-- low, a pull-down for a 0 while oe is high.
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

-------------------------------------------------------------------------------
-- Static RAM with precharged bit lines, as a behaviour (dmg-sim's
-- dmg_simplified_sram): N bytes, 4 columns per word line.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity dmg_simplified_sram is
  generic (N : positive; AW : positive);
  port (
    din         : in  std_logic_vector(7 downto 0);
    dout        : out std_logic_vector(7 downto 0);
    col         : in  std_logic_vector(3 downto 0);
    a, a_n      : in  std_logic_vector(AW - 1 downto 2);
    wr, bl_pch_n, wldrv_pch_n, wldrv_ena : in std_logic
  );
end entity;

architecture sim of dmg_simplified_sram is
  -- The address of the selected byte, or -1.
  function address(row, row_n : std_logic_vector; cols : std_logic_vector) return integer is
    variable wl, c : integer;
  begin
    if is_x(row) or is_x(row_n) or to_x01(row) /= not to_x01(row_n) then
      return -1;
    end if;
    wl := to_integer(unsigned(to_x01(row))) * 4;
    case to_x01(cols) is
      when "0001" => c := 0;
      when "0010" => c := 1;
      when "0100" => c := 2;
      when "1000" => c := 3;
      when others => return -1;
    end case;
    if wl < N then return wl + c; else return -1; end if;
  end function;
begin
  process (din, col, a, a_n, wr, bl_pch_n, wldrv_ena) is
    type mem_t is array (0 to N - 1) of std_logic_vector(7 downto 0);
    variable mem  : mem_t := (others => (others => '0'));
    variable last : std_logic_vector(7 downto 0) := (others => 'X');
    variable adr  : integer;
  begin
    adr := address(a, a_n, col);
    if to_x01(bl_pch_n) = '0' then
      last := (others => 'X');
    end if;
    if to_x01(wr) = '1' then
      if to_x01(bl_pch_n) = '1' then
        last := to_x01(din);
      end if;
      if adr /= -1 and to_x01(wldrv_ena) = '1' and not is_x(last) then
        mem(adr) := last;
      end if;
    end if;
    if to_x01(bl_pch_n) = '1' and to_x01(wldrv_ena) = '1' then
      if adr /= -1 then last := mem(adr); else last := (others => 'X'); end if;
    end if;
    if is_x(last) then dout <= (others => '1'); else dout <= last; end if;
  end process;
end architecture;

-------------------------------------------------------------------------------
-- High RAM (FF80-FFFE) and OAM: SRAMs on the CPU data bus.

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
  type reals is array (0 to 7) of real;
  constant L : reals := (L_d0, L_d1, L_d2, L_d3, L_d4, L_d5, L_d6, L_d7);
  constant W : reals := (W_gate_d0, W_gate_d1, W_gate_d2, W_gate_d3, W_gate_d4, W_gate_d5, W_gate_d6, W_gate_d7);
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
  bits : for i in 0 to 7 generate
    constant T_rise : delay_length := to_t(tpd_elmore(L(i), 2.0 * R_pmos_ohm(120.0 * L_unit), C_gate_F(W(i))));
    constant T_fall : delay_length := to_t(tpd_elmore(L(i), 2.0 * R_nmos_ohm(80.0 * L_unit), C_gate_F(W(i))));
  begin
    b0 : if i = 0 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(0), oe, oe_n, d0); end generate;
    b1 : if i = 1 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(1), oe, oe_n, d1); end generate;
    b2 : if i = 2 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(2), oe, oe_n, d2); end generate;
    b3 : if i = 3 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(3), oe, oe_n, d3); end generate;
    b4 : if i = 4 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(4), oe, oe_n, d4); end generate;
    b5 : if i = 5 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(5), oe, oe_n, d5); end generate;
    b6 : if i = 6 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(6), oe, oe_n, d6); end generate;
    b7 : if i = 7 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(7), oe, oe_n, d7); end generate;
  end generate;
end architecture;

library ieee;
use ieee.std_logic_1164.all;
use work.dmg_timing.all;
use work.verilog.all;

entity dmg_oam is
  generic (
    L_d0 : real := work.dmg_oam_param.L_d0; L_d1 : real := work.dmg_oam_param.L_d1;
    L_d2 : real := work.dmg_oam_param.L_d2; L_d3 : real := work.dmg_oam_param.L_d3;
    L_d4 : real := work.dmg_oam_param.L_d4; L_d5 : real := work.dmg_oam_param.L_d5;
    L_d6 : real := work.dmg_oam_param.L_d6; L_d7 : real := work.dmg_oam_param.L_d7;
    W_gate_d0 : real := work.dmg_oam_param.W_gate_d0; W_gate_d1 : real := work.dmg_oam_param.W_gate_d1;
    W_gate_d2 : real := work.dmg_oam_param.W_gate_d2; W_gate_d3 : real := work.dmg_oam_param.W_gate_d3;
    W_gate_d4 : real := work.dmg_oam_param.W_gate_d4; W_gate_d5 : real := work.dmg_oam_param.W_gate_d5;
    W_gate_d6 : real := work.dmg_oam_param.W_gate_d6; W_gate_d7 : real := work.dmg_oam_param.W_gate_d7
  );
  port (
    d0, d1, d2, d3, d4, d5, d6, d7 : inout std_logic;
    col0, col1, col2, col3 : in std_logic;
    a2, a3, a4, a5, a6, a2_n, a3_n, a4_n, a5_n, a6_n : in std_logic;
    oe, oe_n, wr, bl_pch_n, wldrv_pch_n : in std_logic
  );
end entity;

architecture sim of dmg_oam is
  type reals is array (0 to 7) of real;
  constant L : reals := (L_d0, L_d1, L_d2, L_d3, L_d4, L_d5, L_d6, L_d7);
  constant W : reals := (W_gate_d0, W_gate_d1, W_gate_d2, W_gate_d3, W_gate_d4, W_gate_d5, W_gate_d6, W_gate_d7);
  signal dout : std_logic_vector(7 downto 0);
begin
  sram : entity work.dmg_simplified_sram
    generic map (N => 80, AW => 7)
    port map (
      din => (d7, d6, d5, d4, d3, d2, d1, d0), dout => dout,
      col => (col3, col2, col1, col0),
      a => (a6, a5, a4, a3, a2), a_n => (a6_n, a5_n, a4_n, a3_n, a2_n),
      wr => wr, bl_pch_n => bl_pch_n, wldrv_pch_n => wldrv_pch_n, wldrv_ena => wldrv_pch_n
    );
  bits : for i in 0 to 7 generate
    constant T_rise : delay_length := to_t(tpd_elmore(L(i), 2.0 * R_pmos_ohm(120.0 * L_unit), C_gate_F(W(i))));
    constant T_fall : delay_length := to_t(tpd_elmore(L(i), 2.0 * R_nmos_ohm(80.0 * L_unit), C_gate_F(W(i))));
  begin
    b0 : if i = 0 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(0), oe, oe_n, d0); end generate;
    b1 : if i = 1 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(1), oe, oe_n, d1); end generate;
    b2 : if i = 2 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(2), oe, oe_n, d2); end generate;
    b3 : if i = 3 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(3), oe, oe_n, d3); end generate;
    b4 : if i = 4 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(4), oe, oe_n, d4); end generate;
    b5 : if i = 5 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(5), oe, oe_n, d5); end generate;
    b6 : if i = 6 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(6), oe, oe_n, d6); end generate;
    b7 : if i = 7 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (dout(7), oe, oe_n, d7); end generate;
  end generate;
end architecture;

-------------------------------------------------------------------------------
-- Wave RAM (FF30-FF3F): an SRAM with separate data in and out.

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

-------------------------------------------------------------------------------
-- Boot ROM (0000-00FF while enabled): 256 bytes read from bootrom.bin in the
-- working directory. A row of 64 bytes is selected by the word line, which
-- must stay on one address while it is enabled; the precharged bit lines
-- then discharge for every 0 bit of the addressed byte.

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

  type reals is array (0 to 7) of real;
  constant L : reals := (L_d0, L_d1, L_d2, L_d3, L_d4, L_d5, L_d6, L_d7);
  constant W : reals := (W_gate_d0, W_gate_d1, W_gate_d2, W_gate_d3, W_gate_d4, W_gate_d5, W_gate_d6, W_gate_d7);

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

  bits : for i in 0 to 7 generate
    constant T_rise : delay_length := to_t(tpd_elmore(L(i), 2.0 * R_pmos_ohm(136.0 * L_unit), C_gate_F(W(i))));
    constant T_fall : delay_length := to_t(tpd_elmore(L(i), 2.0 * R_nmos_ohm(100.0 * L_unit), C_gate_F(W(i))));
  begin
    b0 : if i = 0 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (bl_data(0), oe, oe_n, d0); end generate;
    b1 : if i = 1 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (bl_data(1), oe, oe_n, d1); end generate;
    b2 : if i = 2 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (bl_data(2), oe, oe_n, d2); end generate;
    b3 : if i = 3 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (bl_data(3), oe, oe_n, d3); end generate;
    b4 : if i = 4 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (bl_data(4), oe, oe_n, d4); end generate;
    b5 : if i = 5 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (bl_data(5), oe, oe_n, d5); end generate;
    b6 : if i = 6 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (bl_data(6), oe, oe_n, d6); end generate;
    b7 : if i = 7 generate drv : entity work.dmg_bus_bit generic map (T_rise, T_fall) port map (bl_data(7), oe, oe_n, d7); end generate;
  end generate;
end architecture;

-------------------------------------------------------------------------------
-- Analog audio: the four DACs, the mixers and the volume amplifiers, by
-- behaviour (voltages in V).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity dmg_audio is
  generic (
    L_rout : real := work.dmg_audio_param.L_rout; L_lout : real := work.dmg_audio_param.L_lout;
    W_gate_rout : real := 0.0; W_gate_lout : real := 0.0
  );
  port (
    ch1_0, ch1_1, ch1_2, ch1_3, ch2_0, ch2_1, ch2_2, ch2_3 : in std_logic;
    ch3_0, ch3_1, ch3_2, ch3_3, ch4_0, ch4_1, ch4_2, ch4_3 : in std_logic;
    ena, ena_n, ch1_ena_n, ch2_ena_n, ch3_ena_n, ch4_ena_n : in std_logic;
    r_vin_ena, rmixer0, rmixer1, rmixer2, rmixer3 : in std_logic;
    l_vin_ena, lmixer0, lmixer1, lmixer2, lmixer3 : in std_logic;
    rvolume_n0, rvolume_n1, rvolume_n2, lvolume_n0, lvolume_n1, lvolume_n2 : in std_logic;
    rout, lout : out real;
    vin : in real
  );
end entity;

architecture sim of dmg_audio is
  -- A quarter cycle of delay against glitches between the bits of a value.
  constant dl : delay_length := 60 ns;

  function num(v : std_logic_vector) return natural is
  begin
    if is_x(v) then return 0; else return to_integer(unsigned(to_x01(v))); end if;
  end function;

  signal ch1, ch2, ch3, ch4 : natural := 0;
  signal on_dl : boolean := false;
  signal rmix, lmix : std_logic_vector(1 to 5) := (others => '0');
  signal rvol, lvol : natural := 1;
begin
  ch1 <= num(ch1_3 & ch1_2 & ch1_1 & ch1_0) after dl;
  ch2 <= num(ch2_3 & ch2_2 & ch2_1 & ch2_0) after dl;
  ch3 <= num(ch3_3 & ch3_2 & ch3_1 & ch3_0) after dl;
  ch4 <= num(ch4_3 & ch4_2 & ch4_1 & ch4_0) after dl;
  on_dl <= (to_x01(ena) and not to_x01(ena_n)) = '1' after dl;
  rmix <= (rmixer3 and not ch4_ena_n and ena and not ena_n) &
          (rmixer2 and not ch3_ena_n and ena and not ena_n) &
          (rmixer1 and not ch2_ena_n and ena and not ena_n) &
          (rmixer0 and not ch1_ena_n and ena and not ena_n) &
          (r_vin_ena and ena and not ena_n) after dl;
  lmix <= (lmixer3 and not ch4_ena_n and ena and not ena_n) &
          (lmixer2 and not ch3_ena_n and ena and not ena_n) &
          (lmixer1 and not ch2_ena_n and ena and not ena_n) &
          (lmixer0 and not ch1_ena_n and ena and not ena_n) &
          (l_vin_ena and ena and not ena_n) after dl;
  rvol <= 7 - num(rvolume_n2 & rvolume_n1 & rvolume_n0) + 1 after dl;
  lvol <= 7 - num(lvolume_n2 & lvolume_n1 & lvolume_n0) + 1 after dl;

  process (ch1, ch2, ch3, ch4, on_dl, rmix, lmix, rvol, lvol, vin) is
    -- The DACs output 0.7 V to 3.7 V in 0.2 V steps.
    function dac(n : natural) return real is
    begin
      return real(n) / 5.0 + 0.7;
    end function;
    -- Mixing and volume invert and scale around 2.2 V; the mixer resistors
    -- are four times the maximum of the volume rheostat.
    function mix(m : std_logic_vector(1 to 5); c1, c2, c3, c4 : natural; v_in : real; vol : natural) return real is
      variable s : real := 0.0;
    begin
      if m(1) = '1' then s := s + (dac(c4) - 2.2) * 0.25; end if;
      if m(2) = '1' then s := s + (dac(c3) - 2.2) * 0.25; end if;
      if m(3) = '1' then s := s + (dac(c2) - 2.2) * 0.25; end if;
      if m(4) = '1' then s := s + (dac(c1) - 2.2) * 0.25; end if;
      if m(5) = '1' then s := s + (v_in - 2.2) * 0.25; end if;
      return s * (-(real(vol) / 8.0)) + 2.2;
    end function;
    -- The output is capped at 0.3 V and 4.3 V.
    function cap(v : real) return real is
    begin
      if v > 4.3 then return 4.3; elsif v < 0.3 then return 0.3; else return v; end if;
    end function;
  begin
    if on_dl then
      rout <= cap(mix(rmix, ch1, ch2, ch3, ch4, vin, rvol));
      lout <= cap(mix(lmix, ch1, ch2, ch3, ch4, vin, lvol));
    else
      rout <= 2.2;
      lout <= 2.2;
    end if;
  end process;
end architecture;
