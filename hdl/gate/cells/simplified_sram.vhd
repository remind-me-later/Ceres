-- SPDX-License-Identifier: CC-BY-SA-4.0

-- Static RAM with precharged bit lines, as a behaviour (dmg-sim's
-- dmg_simplified_sram): N bytes, 4 columns per word line.
--
-- Ported by hand from dmg-sim's dmg_cpu_b/cells/simplified_sram.sv (Copyright Michael
-- Singer, CC-BY-SA 4.0), which models it by behaviour.
--
-- As in dmg-sim's default build, OAM and wave RAM use this simplified SRAM
-- too, which cannot reproduce the OAM corruption bug nor the wave RAM
-- corruption of Blargg's dmg_sound test 10. Where dmg-sim computes
-- addresses with 2-state integers, an unknown bit (only seen while the chip
-- powers up) makes the address invalid here.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.dmg_memory.all;

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
  -- A copy of the content, which the testbench reads by an external name
  -- (the memory itself is a variable, as dmg-sim writes and reads it in
  -- the same step).
  signal mem_view : bytes(0 to N - 1) := (others => (others => '0'));
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
        mem_view(adr) <= last;
      end if;
    end if;
    if to_x01(bl_pch_n) = '1' and to_x01(wldrv_ena) = '1' then
      if adr /= -1 then last := mem(adr); else last := (others => 'X'); end if;
    end if;
    if is_x(last) then dout <= (others => '1'); else dout <= last; end if;
  end process;
end architecture;
