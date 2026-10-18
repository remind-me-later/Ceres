-- SPDX-License-Identifier: MIT

-- The result of a gbmicrotest: the test writes its result to FF80, the
-- expected value to FF81, and 01 (passed) or FF (failed) to FF82, all in
-- high RAM.
--
-- It reads high RAM by an external name (<< signal ... >>), relative to the
-- design that instantiates it, which must call the chip `dut` and
-- instantiate it first.

library ieee;
use ieee.std_logic_1164.all;
use work.dmg_memory.all;
use work.tb_util.all;

entity gbmicrotest_result is
  generic (
    ENABLED     : boolean;
    RESULT_FILE : string
  );
end entity;

architecture sim of gbmicrotest_result is
  alias hram is << signal ^.dut.hram.u_high_ram.sram.mem_view : bytes(0 to 127) >>;
begin
  process is
    -- The byte at FF80 + adr. High RAM stores its bytes with the bits of the
    -- data bus reversed.
    impure function byte_at(adr : natural) return std_logic_vector is
      variable v : std_logic_vector(7 downto 0);
    begin
      for i in 0 to 7 loop
        v(i) := hram(adr)(7 - i);
      end loop;
      return v;
    end function;
  begin
    if not ENABLED then
      wait;
    end if;
    loop
      wait on hram;
      if byte_at(2) = x"01" or byte_at(2) = x"FF" then
        -- RAM is written as long as /WR is low, and the bus holds the
        -- precharged FF before the CPU drives the byte: read it again once
        -- the write is over.
        wait for 2 us;
      end if;
      if byte_at(2) = x"01" then
        finish(RESULT_FILE, "pass FF80=" & hex(byte_at(0)) & " FF81=" & hex(byte_at(1)));
      elsif byte_at(2) = x"FF" then
        finish(RESULT_FILE, "fail FF80=" & hex(byte_at(0)) & " FF81=" & hex(byte_at(1)));
      end if;
    end loop;
  end process;
end architecture;
