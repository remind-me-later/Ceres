-- SPDX-License-Identifier: CC-BY-SA-4.0

-- A cartridge: a ROM, and a RAM if the header asks for one, with no memory
-- bank controller, an MBC1 or an MBC5. The header (0147, 0149) says which.
-- Without a ROM file, nothing is on the bus.
--
-- The controllers are dmg-sim's (mbc/mbc1.sv and mbc/mbc5.sv; Copyright
-- Michael Singer, CC-BY-SA 4.0).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use work.dmg_memory.all;
use work.tb_util.all;

entity cartridge is
  generic (ROM_FILE : string);
  port (
    nreset        : in std_logic;
    ncs, nrd, nwr : in std_logic;
    a             : in std_logic_vector(15 downto 0);
    d             : inout std_logic_vector(7 downto 0)
  );
end entity;

architecture sim of cartridge is
  constant rom     : rom_image := load_rom(ROM_FILE);
  constant has_rom : boolean := ROM_FILE /= "";

  type mbc_t is (none, mbc1, mbc5);

  function mbc_of(image : rom_image) return mbc_t is
  begin
    if image'length <= 16#147# then
      return none;
    end if;
    case image(16#147#) is
      when 16#00# | 16#08# | 16#09# => return none;
      when 16#01# | 16#02# | 16#03# => return mbc1;
      when 16#19# to 16#1E#         => return mbc5;
      when others =>
        report "unsupported cartridge type " & integer'image(image(16#147#)) severity failure;
        return none;
    end case;
  end function;

  function ram_size_of(image : rom_image) return natural is
  begin
    if image'length <= 16#149# then
      return 0;
    end if;
    case image(16#149#) is
      when 0      => return 0;
      when 1      => return 2048;
      when 2      => return 8192;
      when 3      => return 32768;
      when 4      => return 131072;
      when 5      => return 65536;
      when others => return 0;
    end case;
  end function;

  constant mbc      : mbc_t   := mbc_of(rom);
  constant ram_size : natural := ram_size_of(rom);

  -- The registers of the memory bank controller.
  signal ram_ena      : boolean := false;
  signal rom_bank_reg : natural range 0 to 511 := 0;
  signal ram_bank_reg : natural range 0 to 15 := 0;
  signal mbc1_mode    : std_ulogic := '0';
begin
  -- The registers, written on the rising edge of /WR, cleared by the reset.
  registers : process (nreset, nwr) is
    variable dv : natural;
  begin
    if to_x01(nreset) = '0' then
      ram_ena      <= false;
      rom_bank_reg <= 0;
      ram_bank_reg <= 0;
      mbc1_mode    <= '0';
    elsif rising_edge(nwr) and not is_x(a(15 downto 12)) and not is_x(d) then
      dv := index(d);
      case mbc is
        when mbc1 =>
          case index(a(15 downto 13)) is
            when 0      => ram_ena <= dv mod 16 = 16#A#;
            when 1      => rom_bank_reg <= dv mod 32;
            when 2      => ram_bank_reg <= dv mod 4;
            when 3      => mbc1_mode <= d(0);
            when others => null;
          end case;
        when mbc5 =>
          case index(a(15 downto 12)) is
            when 0      => ram_ena <= dv mod 16 = 16#A#;
            when 2      => rom_bank_reg <= (rom_bank_reg / 256) * 256 + dv;
            when 3      => rom_bank_reg <= (dv mod 2) * 256 + rom_bank_reg mod 256;
            when 4      => ram_bank_reg <= dv mod 16;
            when others => null;
          end case;
        when none => null;
      end case;
    end if;
  end process;

  -- The ROM and the RAM on the bus.
  memory : process (a, nrd, nwr, ncs, nreset, ram_ena, rom_bank_reg, ram_bank_reg, mbc1_mode) is
    variable ram      : bytes(0 to 131071) := (others => (others => '0'));
    variable rom_cs   : boolean;
    variable ram_cs   : boolean;
    variable rom_bank : natural;
    variable ram_bank : natural;
    variable adr      : natural;
    variable known    : boolean;
  begin
    known := not is_x(a);
    rom_bank := 0;
    ram_bank := 0;
    case mbc is
      when none =>
        rom_cs := to_x01(a(15)) = '0';
        ram_cs := to_x01(ncs) = '0' and to_x01(a(13)) = '1';
      when mbc1 =>
        rom_cs := to_x01(nreset) = '0' or (to_x01(a(15)) = '0' and to_x01(nrd) = '0');
        ram_cs := ram_ena and to_x01(ncs) = '0' and to_x01(a(14)) = '0';
        if to_x01(nreset) = '1' and to_x01(a(14)) = '1' then
          rom_bank := rom_bank_reg;
          if rom_bank = 0 then rom_bank := 1; end if;
        end if;
        if mbc1_mode = '1' or to_x01(a(14)) = '1' then ram_bank := ram_bank_reg; end if;
      when mbc5 =>
        rom_cs := to_x01(a(15)) = '0';
        ram_cs := ram_ena and to_x01(ncs) = '0' and to_x01(a(14)) = '0';
        if to_x01(nreset) = '1' and to_x01(a(14)) = '1' then rom_bank := rom_bank_reg; end if;
        if to_x01(a(14)) = '1' then ram_bank := ram_bank_reg; end if;
    end case;
    ram_cs := ram_cs and ram_size /= 0;

    -- The RAM is written on the rising edge of /WR.
    if nwr'event and to_x01(nwr) = '1' and ram_cs and known then
      adr := (ram_bank * 8192 + index(a(12 downto 0))) mod ram_size;
      if is_x(d) then ram(adr) := x"5A"; else ram(adr) := to_x01(d); end if;
    end if;

    if has_rom and rom_cs and to_x01(nrd) = '0' then
      if known then
        adr := (rom_bank * 16384 + index(a(13 downto 0))) mod rom'length;
        if mbc = none then
          adr := index(a(14 downto 0)) mod rom'length;
        end if;
        d <= std_logic_vector(to_unsigned(rom(adr), 8));
      else
        d <= (others => 'X');
      end if;
    elsif ram_cs and to_x01(nrd) = '0' and to_x01(nwr) = '1' then
      if known then
        d <= ram((ram_bank * 8192 + index(a(12 downto 0))) mod ram_size);
      else
        d <= (others => 'X');
      end if;
    else
      d <= (others => 'Z');
    end if;
  end process;
end architecture;
