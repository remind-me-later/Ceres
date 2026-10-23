-- SPDX-License-Identifier: MIT
--
-- The Game Boy's board around the chip (../rtl/dmg_cpu_b.vhd), as dmg-sim's
-- board has it (dmg_cpu_b_gameboy.sv), so that the model runs a ROM on its
-- own: the 4 MiHz crystal, the reset at power-on, the cartridge, the work
-- RAM, the VRAM chip, the buttons, and keepers on the buses.
--
-- The link port, the LCD's pins and the sound outputs are the board's
-- ports (../board/gameboy_tb.vhd reads them).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity gameboy is
	generic (
		ROM     : string;  -- the cartridge's ROM; "" for none
		BOOTROM : string;  -- the boot ROM's 256 bytes

		-- Half a period of the crystal. The Game Boy's runs at 4 MiHz (a
		-- half period of 119.2 ns); dmg-sim's runs at 119 ns, the half period
		-- rounded to its unit of time, and so does this one by default, so
		-- that the two keep the same time.
		T_HALF : time := 119 ns;

		-- Whether the chip has its four sound channels (see dmg_cpu_b)
		WITH_CHANNELS : boolean := true;

		-- Whether the VRAM chip starts with zeros, as dmg-sim's quickboot
		-- leaves it, instead of arbitrary bytes (hdl/boot/tinyboot.asm)
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
		ld                      : out std_ulogic_vector(1 downto 0);

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

	-- The VRAM at power-on: arbitrary, or zeros (VRAM_ZERO).
	function vram_power_on(zero : boolean) return bytes_t is
	begin
		if zero then
			return (0 to 8191 => (others => '0'));
		end if;
		return arbitrary(8192, 2);
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

	-- The VRAM bus
	signal ma                  : std_ulogic_vector(12 downto 0);
	signal md                  : std_logic_vector(7 downto 0);
	signal mcs_n, mrd_n, mwr_n : std_logic;

	-- The buttons' pins
	signal p10_13   : std_logic_vector(3 downto 0);
	signal p14, p15 : std_logic;

	signal work_ram  : bytes_t(0 to 8191) := arbitrary(8192, 1);
	signal video_ram : bytes_t(0 to 8191) := vram_power_on(VRAM_ZERO);
begin
	-- The crystal, between CK1 and CK2 (the chip's inverter): CK1 takes the
	-- value of CK2 half a period later. The reset holds for 64 periods.
	crystal : process
	begin
		wait for T_HALF;
		ck1 <= ck2;
	end process;

	reset_n <= '1' after 128 * T_HALF;

	u_chip : entity work.dmg_cpu_b
		generic map (
			BOOTROM => BOOTROM, WITH_CHANNELS => WITH_CHANNELS
		)
		port map (
			ck1 => ck1, ck2 => ck2, reset_n => reset_n, t1 => '0', t2 => '0',
			phi => phi, a => a, d => d, rd_n => rd_n, wr_n => wr_n, cs_n => cs_n,
			ma => ma, md => md, mcs_n => mcs_n, mrd_n => mrd_n, mwr_n => mwr_n,
			sin => sin, sout => sout, sck => sck, p10_13 => p10_13, p14 => p14, p15 => p15,
			cpg => cpg, cp => cp, cpl => cpl, fr => fr, st => st, s => s, ld => ld,
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
	-- and A14. It takes the data bus at the rise of /WR.
	work_ram_write : process (wr_n)
	begin
		if rising_edge(wr_n) and cs_n = '0' and a(14) = '1' and not is_x(a(12 downto 0)) then
			work_ram(to_integer(unsigned(a(12 downto 0)))) <= to_x01(d);
		end if;
	end process;

	d <= (others => 'Z') when not (cs_n = '0' and a(14) = '1' and rd_n = '0' and wr_n = '1') else
	     (others => 'X') when is_x(a(12 downto 0)) else
	     std_logic_vector(work_ram(to_integer(unsigned(a(12 downto 0)))));

	-- The VRAM chip (an LH5164): written for as long as /MCS and /MWR are
	-- low, which /MRD does not override. dmg-sim needs the continuous write
	-- for Mooneye's lcdon_write_timing.
	video_ram_write : process (all)
	begin
		if mcs_n = '0' and mwr_n = '0' and not is_x(ma) then
			video_ram(to_integer(unsigned(ma))) <= to_x01(md);
		end if;
	end process;

	md <= (others => 'Z') when not (mcs_n = '0' and mrd_n = '0' and mwr_n = '1') else
	      (others => 'X') when is_x(ma) else
	      std_logic_vector(video_ram(to_integer(unsigned(ma))));

	-- The buttons: P14 low reads the directions on P10 to P13, P15 low
	-- reads the others, through diodes; the chip pulls the pins up.
	buttons_pins : for i in 0 to 3 generate
		p10_13(i) <= '0' when (buttons(i) = '1' and to_x01(p14) = '0')
		                   or (buttons(4 + i) = '1' and to_x01(p15) = '0') else 'Z';
	end generate;

	a_keeper : entity work.keeper generic map (WIDTH => 16) port map (lines => a);
	d_keeper : entity work.keeper generic map (WIDTH => 8) port map (lines => d);
	md_keeper : entity work.keeper generic map (WIDTH => 8) port map (lines => md);
	controls_keeper : entity work.keeper
		generic map (
			WIDTH => 5
		)
		port map (
			lines(0) => rd_n, lines(1) => wr_n, lines(2) => mrd_n, lines(3) => mwr_n, lines(4) => mcs_n
		);
end architecture;
