-- SPDX-License-Identifier: MIT

-- Writes every change of the pins of the chip to TRACE_FILE, one per line:
-- "<time> <pin> <value>", with the values 0, 1, z and x. The pins that
-- change at the same time are written in a fixed order, so that the trace
-- does not depend on the order the simulator runs the processes in.

library ieee;
use ieee.std_logic_1164.all;
use std.textio.all;
use work.tb_util.all;

entity pin_trace is
  generic (TRACE_FILE : string);
  port (
    a                     : in std_logic_vector(15 downto 0);
    d                     : in std_logic_vector(7 downto 0);
    ma                    : in std_logic_vector(12 downto 0);
    md                    : in std_logic_vector(7 downto 0);
    ld                    : in std_logic_vector(1 downto 0);
    phi, nrd, nwr, ncs    : in std_logic;
    nmrd, nmwr, nmcs      : in std_logic;
    cpg, cp, cpl, fr, st  : in std_logic;
    s, m1, sout           : in std_logic
  );
end entity;

architecture sim of pin_trace is
  file trace : text open write_mode is file_name(TRACE_FILE);
begin
  process is
    variable first : boolean := true;
    -- The values last written.
    variable last_a : std_logic_vector(a'range);
    variable last_d : std_logic_vector(d'range);
    variable last_ma : std_logic_vector(ma'range);
    variable last_md : std_logic_vector(md'range);
    variable last_ld : std_logic_vector(ld'range);
    variable last_pin : std_logic_vector(0 to 14);

    procedure pin(name : string; v : std_ulogic; last : inout std_ulogic) is
      variable l : line;
      variable c : character;
    begin
      if first or v /= last then
        case v is
          when '0' | 'L' => c := '0';
          when '1' | 'H' => c := '1';
          when 'Z'       => c := 'z';
          when others    => c := 'x';
        end case;
        write(l, to_string(now, ps) & " " & name & " " & c);
        writeline(trace, l);
        last := v;
      end if;
    end procedure;
  begin
    if is_none(TRACE_FILE) then
      wait;
    end if;
    loop
      for i in a'reverse_range loop pin("a_pad" & integer'image(i), a(i), last_a(i)); end loop;
      for i in d'reverse_range loop pin("d_pad" & integer'image(i), d(i), last_d(i)); end loop;
      for i in ma'reverse_range loop pin("ma_pad" & integer'image(i), ma(i), last_ma(i)); end loop;
      for i in md'reverse_range loop pin("md_pad" & integer'image(i), md(i), last_md(i)); end loop;
      for i in ld'reverse_range loop pin("ld_pad" & integer'image(i), ld(i), last_ld(i)); end loop;
      pin("phi_pad", phi, last_pin(0));
      pin("rd_n_pad", nrd, last_pin(1));
      pin("wr_n_pad", nwr, last_pin(2));
      pin("cs_n_pad", ncs, last_pin(3));
      pin("mrd_n_pad", nmrd, last_pin(4));
      pin("mwr_n_pad", nmwr, last_pin(5));
      pin("mcs_n_pad", nmcs, last_pin(6));
      pin("cpg_pad", cpg, last_pin(7));
      pin("cp_pad", cp, last_pin(8));
      pin("cpl_pad", cpl, last_pin(9));
      pin("fr_pad", fr, last_pin(10));
      pin("st_pad", st, last_pin(11));
      pin("s_pad", s, last_pin(12));
      pin("m1_pad", m1, last_pin(13));
      pin("sout_pad", sout, last_pin(14));
      first := false;
      wait on a, d, ma, md, ld, phi, nrd, nwr, ncs, nmrd, nmwr, nmcs, cpg, cp, cpl, fr, st, s, m1, sout;
    end loop;
  end process;
end architecture;
