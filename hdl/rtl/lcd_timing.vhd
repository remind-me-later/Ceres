-- SPDX-License-Identifier: MIT
--
-- The timing of the lines and frames: the PPU's clocks, the line position
-- LX (0 to 113, in M-cycles), LY (FF44), LYC (FF45), STAT (FF41) and the
-- LCD interrupts, and the LCD's sync pins.
--
-- @hw die: dmg-schematics, categories ppu-lcd and ppu-stat (sheets "LCD"
-- and "FF41 STAT, FF44 LY, FF45 LYC"); checked against dmg-sim
-- (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity lcd_timing is
	port (
		clk        : in std_ulogic;
		ppu_reset  : in std_ulogic;  -- the LCD is off
		hard_reset : in std_ulogic;
		lcd_on     : in std_ulogic;  -- LCDC bit 7
		div        : in unsigned(15 downto 0);

		-- From the rendering
		mode2 : in std_ulogic;  -- OAM scan
		mode3 : in std_ulogic;  -- pixels
		mode0 : in std_ulogic;  -- HBlank

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		-- The PPU's clocks: hclk is 1 MiHz, vclk rises at the end of each
		-- line, and wuvu, the 2 MiHz clock, and wosu help the OAM scan.
		wuvu  : out std_ulogic;
		wosu  : out std_ulogic;
		hclk  : out std_ulogic;
		vclk  : out std_ulogic;
		lx    : out unsigned(6 downto 0);
		ly    : out unsigned(7 downto 0);
		mode1 : out std_ulogic;  -- VBlank

		int_vbl  : out std_ulogic;
		int_stat : out std_ulogic;

		-- LCD pins
		cpl : out std_ulogic;  -- the line clock
		cpg : out std_ulogic;
		fr  : out std_ulogic;  -- the polarity, which alternates
		s   : out std_ulogic   -- the frame sync
	);
end entity;

architecture rtl of lcd_timing is
	signal wuvu_i, vena, wosu_i : std_ulogic := '0';
	signal hclk_i               : std_ulogic;
	signal lx_i                 : unsigned(6 downto 0) := (others => '0');
	signal vclk_i               : std_ulogic := '0';
	signal nype                 : std_ulogic := '0';
	signal mode1_i              : std_ulogic := '0';
	signal stat_mode2           : std_ulogic;  -- mode 2, as STAT sees it
	signal ly_i                 : unsigned(7 downto 0) := (others => '0');
	signal line_153             : std_ulogic := '0';
	signal ly_reset             : std_ulogic;
	signal first_line           : std_ulogic := '0';
	signal frame                : std_ulogic := '0';
	signal line_parity          : std_ulogic := '0';
	signal cpg_lx               : std_ulogic := '0';

	signal lyc                  : std_ulogic_vector(7 downto 0) := (others => '0');
	signal lyc_match            : std_ulogic := '0';
	signal stat_enable          : std_ulogic_vector(6 downto 3) := (others => '0');
	signal write_stat, write_lyc : std_ulogic;
	signal int_oam              : std_ulogic;
begin
	-- The PPU's clock is the chip's, inverted. wuvu halves it on its rising
	-- edge (the chip's falling edge), and vena halves that again: hclk.
	-- The LCD's being off stops them both.
	-- @hw die: WUVU VENA (dffr), XYVA XOTA, TALU
	dividers : process (clk, ppu_reset)
	begin
		if ppu_reset = '1' then
			wuvu_i <= '0';
			vena   <= '0';
		elsif falling_edge(clk) then
			wuvu_i <= not wuvu_i after tco;
			if wuvu_i = '1' then
				vena <= not vena after tco;
			end if;
		end if;
	end process;
	hclk_i <= vena;
	hclk   <= hclk_i;
	wuvu   <= wuvu_i;

	-- @hw die: WOSU (dffr), XYFY
	wosu_ff : process (clk, ppu_reset)
	begin
		if ppu_reset = '1' then
			wosu_i <= '0';
		elsif rising_edge(clk) then
			wosu_i <= not wuvu_i after tco;
		end if;
	end process;
	wosu <= wosu_i;

	-- LX counts hclk, and vclk ends the line when it has reached 113: vclk
	-- is high for one hclk period, and holds LX at 0.
	-- @hw die: SAXO TYPO VYZO TELU SUDE TAHA TYRY (dffr), MUDE; SANU, RUTU
	-- (dffr), SONO
	lx_counter : process (hclk_i, vclk_i, ppu_reset)
	begin
		if vclk_i = '1' or ppu_reset = '1' then
			lx_i <= (others => '0');
		elsif rising_edge(hclk_i) then
			lx_i <= lx_i + 1 after tco;
		end if;
	end process;
	lx <= lx_i;

	end_of_line : process (hclk_i, ppu_reset)
	begin
		if ppu_reset = '1' then
			vclk_i <= '0';
		elsif falling_edge(hclk_i) then
			vclk_i <= '1' after tco when lx_i = 113 else '0' after tco;
		end if;
	end process;
	vclk <= vclk_i;

	-- nype is vclk half an hclk period later.
	-- @hw die: NYPE (dffr)
	nype_ff : process (hclk_i, ppu_reset)
	begin
		if ppu_reset = '1' then
			nype <= '0';
		elsif rising_edge(hclk_i) then
			nype <= vclk_i after tco;
		end if;
	end process;

	-- LY counts the lines at the rising edge of vclk. Line 153 lasts only
	-- until nype falls: then LY reads 0 for the rest of the line.
	-- @hw die: MUWY MYRO LEXA LYDO LOVU LEMA MATO LAFO (dffr), LAMA; NOKO
	-- MYTA (dffr)
	ly_reset <= line_153 or ppu_reset;

	ly_counter : process (vclk_i, ly_reset)
	begin
		if ly_reset = '1' then
			ly_i <= (others => '0');
		elsif rising_edge(vclk_i) then
			ly_i <= ly_i + 1 after tco;
		end if;
	end process;
	ly <= ly_i;

	frame_end : process (nype, ppu_reset)
	begin
		if ppu_reset = '1' then
			line_153   <= '0';
			first_line <= '0';
		elsif falling_edge(nype) then
			line_153   <= '1' after tco when ly_i = 153 else '0' after tco;
			first_line <= '1' after tco when ly_i = 0 else '0' after tco;
		end if;
	end process;

	-- VBlank: from line 144, at the rising edge of nype.
	-- @hw die: XYVO, POPU (dffr), PARU
	vblank : process (nype, ppu_reset)
	begin
		if ppu_reset = '1' then
			mode1_i <= '0';
		elsif rising_edge(nype) then
			mode1_i <= ly_i(7) and ly_i(4) after tco;
		end if;
	end process;
	mode1 <= mode1_i;

	-- The LCD's pins. While the LCD is off, CPL and FR follow the divider.
	-- @hw die: NAPO LUCA (dffr), MAGU MECO KEBO, KUPA KOFO FR; KAHE KYMO CPL;
	-- MEDA (dffr) MURE S; TEGY SYGU (dffr) RYNO POGU CPG. (LEBE, a flip-flop
	-- behind LUCA, drives nothing.)
	polarity : process (mode1_i, ppu_reset)
	begin
		if ppu_reset = '1' then
			frame <= '0';
		elsif rising_edge(mode1_i) then
			frame <= not frame after tco;
		end if;
	end process;

	line_polarity : process (vclk_i, ppu_reset)
	begin
		if ppu_reset = '1' then
			line_parity <= '0';
		elsif falling_edge(vclk_i) then
			line_parity <= not line_parity after tco;
		end if;
	end process;

	fr  <= frame xor not line_parity when lcd_on = '1' else div(7);
	cpl <= vclk_i when lcd_on = '1' else div(6);
	s   <= first_line;

	cpg_ff : process (hclk_i, ppu_reset)
	begin
		if ppu_reset = '1' then
			cpg_lx <= '0';
		elsif falling_edge(hclk_i) then
			cpg_lx <= '1' after tco when lx_i = 0 or lx_i = 7 or lx_i = 45 or lx_i = 83 else '0' after tco;
		end if;
	end process;
	cpg <= cpg_lx or vclk_i;

	-- LYC is a set of latches, open while the CPU writes it. LY = LYC is
	-- checked at the rising edge of hclk.
	-- @hw die: SYRY VUCE SEDY SALO SOTA VAFA VEVO RAHA (drlatch_ee), XUFA
	-- WANE VOZE; RAPE PALY, ROPO (dffr)
	write_lyc <= '1' when cpu_wr = '1' and addr = x"FF45" else '0';

	lyc_latch : process (all)
	begin
		if hard_reset = '1' then
			lyc <= (others => '0');
		elsif write_lyc = '1' then
			lyc <= to_x01(data);
		end if;
	end process;

	lyc_compare : process (hclk_i, hard_reset)
	begin
		if hard_reset = '1' then
			lyc_match <= '0';
		elsif rising_edge(hclk_i) then
			lyc_match <= '1' after tco when std_ulogic_vector(ly_i) = lyc else '0' after tco;
		end if;
	end process;

	-- STAT's interrupt enables: latches, open while the CPU writes STAT.
	-- @hw die: ROXE RUFO REFE RUGU (drlatch_ee), SEPA RYVE PUPU
	write_stat <= '1' when cpu_wr = '1' and addr = x"FF41" else '0';

	stat_latch : process (all)
	begin
		if hard_reset = '1' then
			stat_enable <= (others => '0');
		elsif write_stat = '1' then
			stat_enable <= to_x01(data(6 downto 3));
		end if;
	end process;

	-- The interrupts. The STAT interrupt is the OR of its enabled sources;
	-- IF's flag is set when it rises. The OAM source is the end of each line
	-- outside VBlank (vclk), when the next line's OAM scan begins.
	-- @hw die: TAPA, SUKO TUVA VOTY; VYPU
	int_oam  <= vclk_i and not mode1_i;
	int_stat <= (stat_enable(6) and lyc_match) or (stat_enable(5) and int_oam) or
	            (stat_enable(4) and mode1_i) or (stat_enable(3) and mode0);
	int_vbl  <= mode1_i;

	-- STAT's mode bits: 0 HBlank, 1 VBlank, 2 OAM scan, 3 pixels. (The
	-- latch RUPO between ROPO and bit 2 is held open outside reset.) Mode 2
	-- reaches bit 1 through slower gates than OAM's: it ends there 2 ns
	-- after the scan's flag falls (../rtl/oam_scan.vhd), after mode 3 has
	-- begun, and a read then sees mode 2 or 3, never 0, as on dmg-sim
	-- (gbmicrotest's oam_write_l1_a, lcdon_to_stat3_c).
	-- @hw die: SADU TEBY, XATY WUGA, RUPO SEGO; the enables PUZO POFO SASY
	-- POTE; LY: VEGA..WEZE; LYC: RETU..PUFY (not_if0)
	stat_mode2 <= '1' when mode2 = '1' else '0' after 2 ns;
	data_out <= 'Z' & stat_enable & lyc_match & (stat_mode2 or mode3) & (mode3 or mode1_i)
	            when cpu_rd = '1' and addr = x"FF41" else
	        std_logic_vector(ly_i) when cpu_rd = '1' and addr = x"FF44" else
	        lyc when cpu_rd = '1' and addr = x"FF45" else
	        (others => 'Z');
end architecture;
