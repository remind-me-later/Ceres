-- SPDX-License-Identifier: MIT
--
-- A cartridge: its ROM, its RAM, and the MBC that banks them, as dmg-sim's
-- board has them (dmg_cpu_b_gameboy.sv, mbc/mbc1.sv, mbc/mbc5.sv). The
-- header says which (0147: none, MBC1 or MBC5; 0149: whether there is RAM).
--
-- The ROM answers whenever /RD is low and it is selected, the RAM only when
-- /WR is high too; RAM and the MBC's registers take the data bus at the
-- rise of /WR.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity cartridge is
	generic (
		ROM : string  -- the file with the ROM's bytes; "" for no cartridge
	);
	port (
		reset_n : in    std_ulogic;
		a       : in    std_logic_vector(15 downto 0);
		d       : inout std_logic_vector(7 downto 0);
		rd_n    : in    std_logic;
		wr_n    : in    std_logic;
		cs_n    : in    std_ulogic  -- the chip selects A000 to FDFF
	);
end entity;

architecture board of cartridge is
	type bytes_t is array (natural range <>) of std_ulogic_vector(7 downto 0);
	type bytes_ptr is access bytes_t;

	type mbc_t is (none, mbc1, mbc5);

	-- The ROM's bytes, as many as the file has, in a shared variable: a
	-- ROM can have 8 MiB.
	type rom_store is protected
		procedure load(path : string);
		impure function size return natural;
		impure function byte(i : natural) return std_ulogic_vector;
	end protected;

	type rom_store is protected body
		variable bytes : bytes_ptr := null;

		procedure load(path : string) is
			type byte_file is file of character;
			file f      : byte_file;
			variable st : file_open_status;
			variable c  : character;
			variable n  : natural := 0;
			variable grown : bytes_ptr;
		begin
			file_open(st, f, path, read_mode);
			assert st = open_ok report "cannot open the ROM " & path severity failure;
			bytes := new bytes_t(0 to 32767);
			while not endfile(f) loop
				if n > bytes'high then
					grown := new bytes_t(0 to 2 * bytes'length - 1);
					grown(bytes'range) := bytes.all;
					deallocate(bytes);
					bytes := grown;
				end if;
				read(f, c);
				bytes(n) := std_ulogic_vector(to_unsigned(character'pos(c), 8));
				n := n + 1;
			end loop;
			file_close(f);
		end procedure;

		impure function size return natural is
		begin
			if bytes = null then
				return 0;
			end if;
			return bytes'length;
		end function;

		impure function byte(i : natural) return std_ulogic_vector is
		begin
			return bytes(i mod bytes'length);
		end function;
	end protected body;

	shared variable rom_bytes : rom_store;

	impure function header(i : natural) return natural is
	begin
		return to_integer(unsigned(rom_bytes.byte(i)));
	end function;

	-- dmg-sim's RAM has 256 KiB, cleared at power-on.
	signal ram : bytes_t(0 to 262143) := (others => (others => '0'));

	signal mbc     : mbc_t := none;
	signal has_rom : boolean := false;
	signal has_ram : boolean := false;

	-- The MBC's registers
	signal enabled  : std_ulogic := '0';
	signal rom_bank : unsigned(8 downto 0) := (others => '0');
	signal ram_bank : unsigned(3 downto 0) := (others => '0');
	signal mode     : std_ulogic := '0';

	signal rom_addr : natural;
	signal ram_addr : natural range 0 to 262143;
	signal rom_cs, ram_cs : boolean;
begin
	load : process
	begin
		if ROM /= "" then
			rom_bytes.load(ROM);
			has_rom <= true;
			case header(16#147#) is
				when 16#00# | 16#08# | 16#09# => mbc <= none;
				when 16#01# | 16#02# | 16#03# => mbc <= mbc1;
				when 16#19# to 16#1E# => mbc <= mbc5;
				when others =>
					report "the cartridge's MBC (" & integer'image(header(16#147#)) & ") is not modelled"
						severity failure;
			end case;
			has_ram <= header(16#149#) /= 0;
		end if;
		wait;
	end process;

	-- The MBC's registers, written in 0000 to 7FFF.
	-- @hw measured: dmg-sim's mbc1.sv and mbc5.sv
	registers : process (wr_n, reset_n)
		variable region : natural range 0 to 15;
	begin
		if reset_n = '0' then
			enabled  <= '0';
			rom_bank <= (others => '0');
			ram_bank <= (others => '0');
			mode     <= '0';
		elsif rising_edge(wr_n) and not is_x(a(15 downto 12)) then
			region := to_integer(unsigned(a(15 downto 12)));
			case mbc is
				when mbc1 =>
					case region / 2 is
						when 0 => enabled  <= '1' when d(3 downto 0) = x"A" else '0';
						when 1 => rom_bank <= "0000" & unsigned(to_x01(d(4 downto 0)));
						when 2 => ram_bank <= "00" & unsigned(to_x01(d(1 downto 0)));
						when 3 => mode     <= to_x01(d(0));
						when others => null;
					end case;
				when mbc5 =>
					-- dmg-sim decodes A12 too: 1000 to 1FFF and 5000 to 5FFF
					-- write nothing.
					case region is
						when 0 => enabled <= '1' when d(3 downto 0) = x"A" else '0';
						when 2 => rom_bank(7 downto 0) <= unsigned(to_x01(d));
						when 3 => rom_bank(8) <= to_x01(d(0));
						when 4 => ram_bank <= unsigned(to_x01(d(3 downto 0)));
						when others => null;
					end case;
				when none =>
					null;
			end case;
		end if;
	end process;

	-- Where the ROM and the RAM are read: the banks the MBC selects, and the
	-- address pins.
	decode : process (all)
		variable bank : natural;
		variable low  : natural;
	begin
		rom_addr <= 0;
		ram_addr <= 0;
		rom_cs   <= false;
		ram_cs   <= false;
		if not is_x(a) then
			low := to_integer(unsigned(a(13 downto 0)));
			case mbc is
				when none =>
					rom_addr <= to_integer(unsigned(a(14 downto 0)));
					ram_addr <= to_integer(unsigned(a(12 downto 0)));
					rom_cs   <= a(15) = '0';
					ram_cs   <= cs_n = '0' and a(13) = '1';
				when mbc1 =>
					-- Bank 0 reads as bank 1 in 4000 to 7FFF.
					if reset_n = '0' or a(14) = '0' then
						bank := 0;
					elsif rom_bank(4 downto 0) = 0 then
						bank := 1;
					else
						bank := to_integer(rom_bank(4 downto 0));
					end if;
					rom_addr <= bank * 16384 + low;
					if mode = '1' or a(14) = '1' then
						ram_addr <= to_integer(ram_bank(1 downto 0)) * 8192 + to_integer(unsigned(a(12 downto 0)));
					else
						ram_addr <= to_integer(unsigned(a(12 downto 0)));
					end if;
					rom_cs <= reset_n = '0' or (a(15) = '0' and rd_n = '0');
					ram_cs <= enabled = '1' and cs_n = '0' and a(14) = '0';
				when mbc5 =>
					if reset_n = '0' or a(14) = '0' then
						bank := 0;
					else
						bank := to_integer(rom_bank);
					end if;
					rom_addr <= bank * 16384 + low;
					if a(14) = '1' then
						ram_addr <= to_integer(ram_bank) * 8192 + to_integer(unsigned(a(12 downto 0)));
					else
						ram_addr <= to_integer(unsigned(a(12 downto 0)));
					end if;
					rom_cs <= a(15) = '0';
					ram_cs <= enabled = '1' and cs_n = '0' and a(14) = '0';
			end case;
		end if;
	end process;

	ram_write : process (wr_n)
	begin
		if rising_edge(wr_n) and has_ram and ram_cs then
			ram(ram_addr) <= to_x01(d);
		end if;
	end process;

	read : process (all)
	begin
		d <= (others => 'Z');
		if has_rom and rom_cs and rd_n = '0' then
			if is_x(a) then
				d <= (others => 'X');
			else
				d <= std_logic_vector(rom_bytes.byte(rom_addr));
			end if;
		end if;
		if has_ram and ram_cs and rd_n = '0' and wr_n = '1' then
			d <= std_logic_vector(ram(ram_addr));
		end if;
	end process;
end architecture;
