-- SPDX-License-Identifier: MIT
--
-- The board around the chip, so that the model runs a ROM on its own: the
-- crystal, the reset at power-on, the cartridge, the work RAM, the buttons,
-- and keepers on the buses. REVISION picks the chip:
--
-- - DMG_B: the Game Boy's board around the DMG-CPU B (../rtl/dmg_cpu_b.vhd),
--   as dmg-sim's board has it (dmg_cpu_b_gameboy.sv): the 4 MiHz crystal,
--   8 KiB of work RAM and the VRAM chip.
-- - CGB_C, CGB_E: the Game Boy Color's around the CPU CGB
--   (../rtl/cgb_cpu.vhd): the 8 MiHz crystal, 32 KiB of work RAM on the
--   chip's bank lines, and no VRAM chip (the CGB's VRAM is inside its chip).
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
		BOOTROM : string;  -- the boot ROM's 256 bytes (the CGB's 2304)

		REVISION : revision_t := DMG_B;

		-- Whether the chip has its four sound channels (see dmg_cpu_b)
		WITH_CHANNELS : boolean := true;

		-- Whether the VRAM starts with zeros, as dmg-sim's quickboot (and the
		-- CGB's boot ROM) leaves it, instead of arbitrary bytes
		-- (hdl/boot/tinyboot.asm)
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

		-- The LCD: the DMG's pixel (ld), the CGB's (rgb)
		cpg, cp, cpl, fr, st, s : out std_ulogic;
		ld                      : out std_ulogic_vector(1 downto 0);
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

	-- The VRAM at power-on: arbitrary, or zeros (VRAM_ZERO).
	function vram_power_on(zero : boolean) return bytes_t is
	begin
		if zero then
			return (0 to 8191 => (others => '0'));
		end if;
		return arbitrary(8192, 2);
	end function;

	-- Half a period of the crystal. The Game Boy's runs at 4 MiHz (a half
	-- period of 119.2 ns); dmg-sim's runs at 119 ns, the half period rounded
	-- to its unit of time, and so does this one, so that the two keep the
	-- same time. The Game Boy Color's runs at 8 MiHz (59.6 ns): here 59.5 ns,
	-- half the DMG's.
	function half_period(revision : revision_t) return time is
	begin
		if revision = DMG_B then
			return 119 ns;
		end if;
		return 59500 ps;
	end function;

	constant T_HALF : time := half_period(REVISION);

	signal ck1     : std_ulogic := '0';
	signal ck2     : std_ulogic;
	signal reset_n : std_ulogic := '0';

	-- The cartridge bus
	signal phi        : std_ulogic;
	signal a          : std_logic_vector(15 downto 0);
	signal d          : std_logic_vector(7 downto 0);
	signal rd_n, wr_n : std_logic;
	signal cs_n       : std_ulogic;

	-- The buttons' pins
	signal p10_13   : std_logic_vector(3 downto 0);
	signal p14, p15 : std_logic;
begin
	-- The crystal, between CK1 and CK2 (the chip's inverter): CK1 takes the
	-- value of CK2 half a period later. The reset holds for 64 periods of
	-- the DMG's crystal (128 of the CGB's).
	crystal : process
	begin
		wait for T_HALF;
		ck1 <= ck2;
	end process;

	reset_n <= '1' after 128 * 119 ns;

	-- Both alternatives are labelled as the statement, so that the path to
	-- the chip is .gameboy.chip.u_chip for either (nvc's external names go
	-- by the alternative's label, the LRM's by the statement's).
	chip : if chip : REVISION = DMG_B generate
		-- The VRAM bus
		signal ma                  : std_ulogic_vector(12 downto 0);
		signal md                  : std_logic_vector(7 downto 0);
		signal mcs_n, mrd_n, mwr_n : std_logic;

		signal work_ram  : bytes_t(0 to 8191) := arbitrary(8192, 1);
		signal video_ram : bytes_t(0 to 8191) := vram_power_on(VRAM_ZERO);
	begin
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
		rgb <= (others => '0');

		-- The work RAM: C000 to DFFF (and its echo up to FDFF), selected by
		-- /CS and A14. It takes the data bus at the rise of /WR.
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
		-- low, which /MRD does not override. dmg-sim needs the continuous
		-- write for Mooneye's lcdon_write_timing.
		video_ram_write : process (all)
		begin
			if mcs_n = '0' and mwr_n = '0' and not is_x(ma) then
				video_ram(to_integer(unsigned(ma))) <= to_x01(md);
			end if;
		end process;

		md <= (others => 'Z') when not (mcs_n = '0' and mrd_n = '0' and mwr_n = '1') else
		      (others => 'X') when is_x(ma) else
		      std_logic_vector(video_ram(to_integer(unsigned(ma))));

		md_keeper : entity work.keeper generic map (WIDTH => 8) port map (lines => md);
		controls_keeper : entity work.keeper
			generic map (
				WIDTH => 5
			)
			port map (
				lines(0) => rd_n, lines(1) => wr_n, lines(2) => mrd_n, lines(3) => mwr_n, lines(4) => mcs_n
			);
	else chip : generate
		signal wram_bank : std_ulogic_vector(2 downto 0);
		signal wram_addr : std_ulogic_vector(14 downto 0);

		signal work_ram : bytes_t(0 to 32767) := arbitrary(32768, 1);
	begin
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
		ld <= (others => '0');

		-- The work RAM: C000 to DFFF (and its echo up to FDFF), selected by
		-- /CS and A14, its bank by the chip's bank lines. It takes the data
		-- bus at the rise of /WR.
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

		controls_keeper : entity work.keeper
			generic map (
				WIDTH => 2
			)
			port map (
				lines(0) => rd_n, lines(1) => wr_n
			);
	end generate;

	u_cartridge : entity work.cartridge
		generic map (
			ROM => ROM
		)
		port map (
			reset_n => reset_n, a => a, d => d, rd_n => rd_n, wr_n => wr_n, cs_n => cs_n
		);

	-- The buttons: P14 low reads the directions on P10 to P13, P15 low
	-- reads the others, through diodes; the chip pulls the pins up.
	buttons_pins : for i in 0 to 3 generate
		p10_13(i) <= '0' when (buttons(i) = '1' and to_x01(p14) = '0')
		                   or (buttons(4 + i) = '1' and to_x01(p15) = '0') else 'Z';
	end generate;

	a_keeper : entity work.keeper generic map (WIDTH => 16) port map (lines => a);
	d_keeper : entity work.keeper generic map (WIDTH => 8) port map (lines => d);
end architecture;
