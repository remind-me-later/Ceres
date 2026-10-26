-- SPDX-License-Identifier: MIT
--
-- The Game Boy Color's board around the chip (../rtl/cgb_cpu.vhd), so that
-- the model runs a ROM on its own: the crystal, the reset at power-on, the
-- cartridge, the work RAM, the buttons, and keepers on the buses. It is the
-- DMG's board as dmg-sim has it (dmg_cpu_b_gameboy.sv), with the CGB's work
-- RAM of 32 KiB on the bank lines, and without the VRAM chip (the CGB's VRAM
-- is inside its chip).
--
-- The link port, the LCD's pins and the sound outputs are the board's
-- ports (../board/gameboy_tb.vhd reads them).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.revisions.all;

entity gameboy is
	generic (
		ROM     : string;  -- the cartridge's ROM; "" for none
		BOOTROM : string;  -- the boot ROM's 2304 bytes

		REVISION : revision_t := CGB_E;

		-- Half a period of the crystal. The Game Boy Color's runs at 8 MiHz (a
		-- half period of 59.6 ns); this one runs at 59.5 ns, half of the
		-- DMG model's 119 ns (dmg-sim's half period of the DMG's 4 MiHz,
		-- rounded to its unit of time), so that the two keep the same time.
		T_HALF : time := 59500 ps;

		-- Whether the chip has its four sound channels (see dmg_cpu_b)
		WITH_CHANNELS : boolean := true;

		-- Whether the VRAM starts with zeros, as the boot ROM leaves it,
		-- instead of arbitrary bytes (hdl/boot/tinyboot.asm)
		VRAM_ZERO : boolean := false
	);
	port (
		-- The buttons held down: Start, Select, B, A, Down, Up, Left, Right
		-- (bits 7 to 0)
		buttons : in std_ulogic_vector(7 downto 0) := (others => '0');

		-- The link port
		sin  : inout std_logic;
		sout : out   std_ulogic;
		sck  : inout std_logic;

		-- The LCD
		cpg, cp, cpl, fr, st, s : out std_ulogic;
		rgb                     : out std_ulogic_vector(14 downto 0);

		-- Sound: right and left, in volts
		so1, so2 : out real
	);
end entity;

architecture board of gameboy is
	type bytes_t is array (natural range <>) of std_ulogic_vector(7 downto 0);

	-- RAM chips hold arbitrary bytes at power-on; dmg-sim fills them with
	-- random values. These come from a fixed generator (a 32-bit xorshift),
	-- so that runs repeat.
	-- @hw inferred: the contents are unknown
	function arbitrary(length : positive; seed : positive) return bytes_t is
		variable r : bytes_t(0 to length - 1);
		variable x : unsigned(31 downto 0) := to_unsigned(seed, 32);
	begin
		for i in r'range loop
			x := x xor shift_left(x, 13);
			x := x xor shift_right(x, 17);
			x := x xor shift_left(x, 5);
			r(i) := std_ulogic_vector(x(7 downto 0));
		end loop;
		return r;
	end function;

	signal ck1     : std_ulogic := '0';
	signal ck2     : std_ulogic;
	signal reset_n : std_ulogic := '0';

	-- The cartridge bus
	signal phi        : std_ulogic;
	signal a          : std_logic_vector(15 downto 0);
	signal d          : std_logic_vector(7 downto 0);
	signal rd_n, wr_n : std_logic;
	signal cs_n       : std_ulogic;
	signal wram_bank  : std_ulogic_vector(2 downto 0);
	signal wram_addr  : std_ulogic_vector(14 downto 0);

	-- The buttons' pins
	signal p10_13   : std_logic_vector(3 downto 0);
	signal p14, p15 : std_logic;

	signal work_ram : bytes_t(0 to 32767) := arbitrary(32768, 1);
begin
	-- The crystal, between CK1 and CK2 (the chip's inverter): CK1 takes the
	-- value of CK2 half a period later. The reset holds for 128 periods (the
	-- DMG model's 64 of its 4 MiHz crystal).
	crystal : process
	begin
		wait for T_HALF;
		ck1 <= ck2;
	end process;

	reset_n <= '1' after 256 * T_HALF;

	u_chip : entity work.cgb_cpu
		generic map (
			BOOTROM => BOOTROM, REVISION => REVISION, VRAM_ZERO => VRAM_ZERO,
			WITH_CHANNELS => WITH_CHANNELS
		)
		port map (
			ck1 => ck1, ck2 => ck2, reset_n => reset_n, t1 => '0', t2 => '0',
			phi => phi, a => a, d => d, rd_n => rd_n, wr_n => wr_n, cs_n => cs_n,
			wram_bank => wram_bank,
			sin => sin, sout => sout, sck => sck, p10_13 => p10_13, p14 => p14, p15 => p15,
			cpg => cpg, cp => cp, cpl => cpl, fr => fr, st => st, s => s, rgb => rgb,
			vin => 0.0, so1 => so1, so2 => so2
		);

	u_cartridge : entity work.cartridge
		generic map (
			ROM => ROM
		)
		port map (
			reset_n => reset_n, a => a, d => d, rd_n => rd_n, wr_n => wr_n, cs_n => cs_n
		);

	-- The work RAM: C000 to DFFF (and its echo up to FDFF), selected by /CS
	-- and A14, its bank by the chip's bank lines. It takes the data bus at the
	-- rise of /WR.
	wram_addr <= wram_bank & to_x01(a(11 downto 0));

	work_ram_write : process (wr_n)
	begin
		if rising_edge(wr_n) and cs_n = '0' and a(14) = '1' and not is_x(wram_addr) then
			work_ram(to_integer(unsigned(wram_addr))) <= to_x01(d);
		end if;
	end process;

	d <= (others => 'Z') when not (cs_n = '0' and a(14) = '1' and rd_n = '0' and wr_n = '1') else
	     (others => 'X') when is_x(wram_addr) else
	     std_logic_vector(work_ram(to_integer(unsigned(wram_addr))));

	-- The buttons: P14 low reads the directions on P10 to P13, P15 low
	-- reads the others, through diodes; the chip pulls the pins up.
	buttons_pins : for i in 0 to 3 generate
		p10_13(i) <= '0' when (buttons(i) = '1' and to_x01(p14) = '0')
		                   or (buttons(4 + i) = '1' and to_x01(p15) = '0') else 'Z';
	end generate;

	a_keeper : entity work.keeper generic map (WIDTH => 16) port map (lines => a);
	d_keeper : entity work.keeper generic map (WIDTH => 8) port map (lines => d);
	controls_keeper : entity work.keeper
		generic map (
			WIDTH => 2
		)
		port map (
			lines(0) => rd_n, lines(1) => wr_n
		);
end architecture;
