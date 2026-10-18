-- SPDX-License-Identifier: MIT

-- The other end of the serial link: receives what the Game Boy sends with
-- its internal clock, a byte with the most significant bit first, sampled
-- on the rising edge of SCK. The text goes to SERIAL_FILE.
--
-- Blargg's tests print their result there: with BLARGG, "Passed" or
-- "Failed" ends the simulation.

library ieee;
use ieee.std_logic_1164.all;
use std.textio.all;
use work.tb_util.all;

entity serial_receiver is
  generic (
    SERIAL_FILE : string;
    BLARGG      : boolean;
    RESULT_FILE : string
  );
  port (sck, sout : in std_logic);
end entity;

architecture sim of serial_receiver is
  file serial : text open write_mode is file_name(SERIAL_FILE);
begin
  process is
    variable byte     : std_logic_vector(7 downto 0);
    variable received : string(1 to 4096);
    variable len      : natural := 0;
    variable l        : line;
    variable ch       : character;
  begin
    loop
      for i in 7 downto 0 loop
        wait until rising_edge(sck);
        byte(i) := to_x01(sout);
      end loop;
      if is_x(byte) then
        ch := '?';
      else
        ch := character'val(index(byte));
      end if;
      if not is_none(SERIAL_FILE) then
        if ch = LF then
          writeline(serial, l);
        else
          write(l, ch);
        end if;
      end if;
      if len < received'length then
        len := len + 1;
        received(len) := ch;
      end if;
      if BLARGG and len >= 6 then
        if received(len - 5 to len) = "Passed" then
          finish(RESULT_FILE, "pass serial: " & received(1 to len));
        elsif received(len - 5 to len) = "Failed" then
          finish(RESULT_FILE, "fail serial: " & received(1 to len));
        end if;
      end if;
    end loop;
  end process;
end architecture;
