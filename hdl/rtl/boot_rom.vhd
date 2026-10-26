-- SPDX-License-Identifier: MIT
--
-- The boot ROM, 2304 bytes at 0000 to 00FF and 0200 to 08FF (the cartridge's
-- header shows through at 0100 to 01FF), and the register FF50 that unmaps
-- it.
--
-- @hw die: dmg-schematics, category bootrom (the DMG's 256 bytes at 0000 to
-- 00FF); checked against dmg-sim (../check/chip_tb.vhd).
-- @hw inferred: the CGB's second range, from the size of its boot ROM and
-- SameBoy's and Pan Docs' memory maps.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity boot_rom is
	generic (
		IMAGE : string  -- the file with the boot ROM's 2304 bytes
	);
	port (
		reset2  : in std_ulogic;
		t2_only : in std_ulogic;  -- test mode that unmaps the boot ROM

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		boot_sel : out std_ulogic;  -- the boot ROM answers, not the cartridge
		boot_on  : out std_ulogic   -- the boot ROM is mapped
	);
end entity;

architecture rtl of boot_rom is
	type rom_t is array (0 to 2303) of std_ulogic_vector(7 downto 0);

	impure function load(path : string) return rom_t is
		type byte_file is file of character;
		file f     : byte_file open read_mode is path;
		variable c : character;
		variable r : rom_t := (others => (others => '0'));
	begin
		for i in rom_t'range loop
			exit when endfile(f);
			read(f, c);
			r(i) := std_ulogic_vector(to_unsigned(character'pos(c), 8));
		end loop;
		return r;
	end function;

	constant rom : rom_t := load(IMAGE);

	signal unmapped   : std_ulogic := '0';
	signal write_ff50 : std_ulogic;
	signal boot_sel_i : std_ulogic;
begin
	-- Writing FF50 with bit 0 set unmaps the boot ROM until the next reset.
	-- @hw die: TEPU (dffr), clocked by TUGE; SATO
	write_ff50 <= '1' when cpu_wr = '1' and addr = x"FF50" else '0';

	ff50 : process (write_ff50, reset2)
	begin
		if reset2 = '1' then
			unmapped <= '0';
		elsif falling_edge(write_ff50) then
			unmapped <= unmapped or to_x01(data(0)) after tco;
		end if;
	end process;

	-- @hw die: TULO TERA TUTU (0000 to 00FF)
	-- @hw inferred: 0200 to 08FF
	boot_sel_i <= '1' when unmapped = '0' and (addr(15 downto 8) = x"00"
	                       or (addr(15 downto 12) = x"0" and unsigned(addr(11 downto 8)) >= 2
	                           and unsigned(addr(11 downto 8)) <= 8)) else '0';
	boot_sel   <= boot_sel_i;
	boot_on    <= not unmapped;

	-- @hw die: YULA YAZA, ZUFA ZADO, boot_rom
	data_out <= rom(to_integer(to_01(unsigned(addr(11 downto 0))))) when cpu_rd = '1' and boot_sel_i = '1' and t2_only = '0' else
	        "ZZZZZZZ" & unmapped when cpu_rd = '1' and addr = x"FF50" else
	        (others => 'Z');
end architecture;
