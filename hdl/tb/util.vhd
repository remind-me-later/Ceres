-- SPDX-License-Identifier: MIT

-- Helpers shared by the parts of the testbench: files, ROM images and the
-- result of a test.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use std.textio.all;
use work.dmg_memory.all;

package tb_util is
  -- The bytes of a ROM image, as numbers.
  type rom_image is array (natural range <>) of natural range 0 to 255;

  -- The content of a file; a single FF byte for "".
  impure function load_rom(name : string) return rom_image;

  -- Pseudo-random bytes, the initial content of a RAM (dmg-sim uses
  -- $random).
  function noise(n : natural) return bytes;

  -- "" or "-": no file (nvc cannot pass an empty string as a generic).
  function is_none(name : string) return boolean;

  -- The name to open for a file generic: /dev/null for none.
  function file_name(name : string) return string;

  -- A vector as a number, its unknown bits read as 0.
  function index(v : std_logic_vector) return natural;

  -- A byte as two hex digits, "xx" if a bit is unknown.
  function hex(v : std_logic_vector) return string;

  -- Ends the simulation with a result ("pass ...", "fail ...", "timeout"),
  -- which also goes to result_file unless it is none.
  procedure finish(result_file : string; result : string);
end package;

package body tb_util is
  impure function read_rom(name : string; n : positive) return rom_image is
    type bin_file is file of character;
    file f : bin_file;
    variable c : character;
    variable r : rom_image(0 to n - 1);
  begin
    file_open(f, name, read_mode);
    for i in r'range loop
      read(f, c);
      r(i) := character'pos(c);
    end loop;
    file_close(f);
    return r;
  end function;

  impure function load_rom(name : string) return rom_image is
    type bin_file is file of character;
    file f : bin_file;
    variable status : file_open_status;
    variable c : character;
    variable n : natural := 0;
  begin
    if name = "" then
      return (0 => 16#FF#);
    end if;
    -- Count the bytes first: the image is a constant of that size.
    file_open(status, f, name, read_mode);
    assert status = open_ok report "cannot open " & name severity failure;
    while not endfile(f) loop
      read(f, c);
      n := n + 1;
    end loop;
    file_close(f);
    return read_rom(name, n);
  end function;

  function noise(n : natural) return bytes is
    variable r : bytes(0 to n - 1);
    variable x : unsigned(31 downto 0) := x"2545F491";
  begin
    for i in r'range loop
      x := x xor shift_left(x, 13);
      x := x xor shift_right(x, 17);
      x := x xor shift_left(x, 5);
      r(i) := std_logic_vector(x(7 downto 0));
    end loop;
    return r;
  end function;

  function is_none(name : string) return boolean is
  begin
    return name = "" or name = "-";
  end function;

  function file_name(name : string) return string is
  begin
    if is_none(name) then
      return "/dev/null";
    else
      return name;
    end if;
  end function;

  function index(v : std_logic_vector) return natural is
  begin
    return to_integer(unsigned(to_x01(v)));
  end function;

  function hex(v : std_logic_vector) return string is
  begin
    if is_x(v) then
      return "xx";
    else
      return to_hstring(to_x01(v));
    end if;
  end function;

  procedure finish(result_file : string; result : string) is
    file f : text;
    variable l : line;
  begin
    report "result: " & result;
    if not is_none(result_file) then
      file_open(f, result_file, write_mode);
      write(l, result);
      writeline(f, l);
      file_close(f);
    end if;
    std.env.finish;
  end procedure;
end package body;
