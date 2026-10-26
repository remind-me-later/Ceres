-- SPDX-License-Identifier: MIT
--
-- The timing of the lines and frames: the PPU's clocks, the line position
-- LX (0 to 113, in M-cycles), LY (FF44), LYC (FF45), STAT (FF41) and the
-- LCD interrupts, and the LCD's sync pins.
--
-- @hw die: dmg-schematics, categories ppu-lcd and ppu-stat (sheets "LCD"
-- and "FF41 STAT, FF44 LY, FF45 LYC"); checked against dmg-sim
-- (../check/chip_tb.vhd).
--
-- The CGB keeps the DMG's counters and sources, and moves some of what the
-- CPU sees of them, and when its writes take effect, by a few half dots
-- (the clock's edges): see "The CGB's timing" below.
-- @hw inferred: ceres (ceres-core/src/ppu/display/timing.rs), fitted to
-- Gambatte's, SameSuite's, Mooneye's and AGE's CGB tests
-- (doc/cgb-hardware-tests.md, entries 1 to 6). No circuit is known: the
-- half-dot delays here only reproduce ceres's.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;
use work.revisions.all;

entity lcd_timing is
	generic (
		REVISION : revision_t := CGB_E
	);
	port (
		clk        : in std_ulogic;
		ppu_reset  : in std_ulogic;  -- the LCD is off
		hard_reset : in std_ulogic;
		lcd_on     : in std_ulogic;  -- LCDC bit 7
		div        : in unsigned(15 downto 0);

		double_speed : in std_ulogic;  -- KEY1 bit 7

		-- From the rendering
		mode3      : in std_ulogic;  -- pixels
		mode0      : in std_ulogic;  -- HBlank
		scanning   : in std_ulogic;  -- the OAM scan runs (mode 2, whatever OAM DMA does)
		scan_index : in unsigned(5 downto 0);  -- the scan's sprite
		dma_run    : in std_ulogic;  -- OAM DMA copies

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
		start_line : out std_ulogic;  -- the first line after the LCD is turned on

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
	signal ly_i                 : unsigned(7 downto 0) := (others => '0');
	signal line_153             : std_ulogic := '0';
	signal ly_reset             : std_ulogic;
	signal first_line           : std_ulogic := '0';
	signal start_line_i         : std_ulogic := '0';
	signal frame                : std_ulogic := '0';
	signal line_parity          : std_ulogic := '0';
	signal cpg_lx               : std_ulogic := '0';

	signal lyc                  : std_ulogic_vector(7 downto 0) := (others => '0');
	signal lyc_match            : std_ulogic := '0';
	signal stat_written         : std_ulogic_vector(6 downto 3) := (others => '0');
	signal write_stat, write_lyc : std_ulogic;

	-- The CGB's timing
	type enables_t is array (natural range <>) of std_ulogic_vector(6 downto 3);
	type bytes_t is array (natural range <>) of std_ulogic_vector(7 downto 0);
	signal stat_late            : enables_t(1 to 3) := (others => (others => '0'));
	signal stat_enable          : std_ulogic_vector(6 downto 3) := (others => '0');
	signal lyc_late             : bytes_t(1 to 5) := (others => (others => '0'));
	signal lyc_compared         : std_ulogic_vector(7 downto 0);
	signal flag_now             : std_ulogic;
	signal flag_late            : std_ulogic_vector(1 to 3) := (others => '0');
	signal mode0_late           : std_ulogic := '0';
	signal hblank               : std_ulogic;
	signal ly_late              : std_ulogic_vector(7 downto 0) := (others => '0');
	signal line_153_late        : std_ulogic_vector(1 to 2) := (others => '0');
	signal mode1_late           : std_ulogic_vector(1 to 2) := (others => '0');
	signal stat_scan, stat_scan_done : std_ulogic := '0';
	signal vblank_shown         : std_ulogic := '0';
	signal stat_m1, stat_m2, stat_m3 : std_ulogic;

	-- What a read of STAT and LY sees
	signal mode_read            : std_ulogic_vector(1 downto 0);
	signal flag_read            : std_ulogic;
	signal ly_read              : std_ulogic_vector(7 downto 0);
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
	-- until nype falls: then LY reads 0 for the rest of the line (on the
	-- CGB-E, from a dot later).
	-- @hw die: MUWY MYRO LEXA LYDO LOVU LEMA MATO LAFO (dffr), LAMA; NOKO
	-- MYTA (dffr)
	ly_reset <= line_153_late(2) or ppu_reset when REVISION = CGB_E else
	            line_153 or ppu_reset;

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

	-- The first line after the LCD is turned on, which has no OAM scan,
	-- lasts until vclk first rises.
	start_line_ff : process (vclk_i, ppu_reset)
	begin
		if ppu_reset = '1' then
			start_line_i <= '1';
		elsif rising_edge(vclk_i) then
			start_line_i <= '0';
		end if;
	end process;
	start_line <= start_line_i;

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
	-- checked at the rising edge of hclk, with the LYC that the CGB's
	-- comparison sees (below).
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
			lyc_match <= '1' after tco when std_ulogic_vector(ly_i) = lyc_compared else '0' after tco;
		end if;
	end process;

	-- STAT's interrupt enables: latches, open while the CPU writes STAT.
	-- @hw die: ROXE RUFO REFE RUGU (drlatch_ee), SEPA RYVE PUPU
	write_stat <= '1' when cpu_wr = '1' and addr = x"FF41" else '0';

	stat_latch : process (all)
	begin
		if hard_reset = '1' then
			stat_written <= (others => '0');
		elsif write_stat = '1' then
			stat_written <= to_x01(data(6 downto 3));
		end if;
	end process;

	----------------------------------------------------------------------
	-- The CGB's timing. Each rule is ceres's, at the half dot (the clock's
	-- edge) ceres has it. A CPU access here begins (the strobe, phase E) a
	-- half dot after ceres's, and a read takes the byte as phase H ends,
	-- 4 half dots on: it sees the signals as ceres's reads do, 4 half dots
	-- ahead, without help. What moves:
	--
	-- - STAT's written enables reach the interrupt 4 half dots after ceres's
	--   write (3 after the strobe opens), so that the DMG's glitch of a
	--   write (every source enabled for a moment) does not happen; at the
	--   edge where VBlank's latch (POPU) changes, they change with it, and
	--   the interrupt sees both at once.
	-- - A written LYC reaches the comparison at its first sample (the rise
	--   of hclk) 6 half dots or more after ceres's write.
	-- - The interrupt's HBlank source falls a half dot after mode 0's.
	-- - A read sees the LY = LYC flag 3 half dots late (ceres reads it 1
	--   ahead); on the CGB-C it drops as LY changes, as the DMG's, but
	--   holds until the next sample on the CGB-E.
	-- - A read sees mode 2 from h = 909 to 157 (then mode 3), mode 1 from
	--   h = 911 of line 143 to VBlank's end (2 half dots later on the
	--   CGB-E).
	-- - A read of LY when it has changed at the last edge sees old and new
	--   ANDed (the bits that fall have fallen, the one that rises has not
	--   risen yet).
	-- - On the CGB-E, LY becomes 0 on line 153 a dot later.
	--
	-- In double speed these are the same for now: the double speed's own
	-- rules are not modelled yet.
	----------------------------------------------------------------------
	half_dots : process (clk)
	begin
		if clk'event then
			stat_late     <= stat_written & stat_late(1 to 2);
			lyc_late      <= lyc & lyc_late(1 to 4);
			flag_late     <= flag_now & flag_late(1 to 2);
			mode0_late    <= mode0;
			ly_late       <= std_ulogic_vector(ly_i);
			line_153_late <= line_153 & line_153_late(1);
			mode1_late    <= mode1_i & mode1_late(1);
		end if;
	end process;

	-- The enables land 3 ns after the edge, as POPU does (its clock comes
	-- through vena and nype).
	stat_enable <= stat_late(3) after 3 * tco;

	lyc_compared <= lyc_late(5);

	flag_now  <= lyc_match when REVISION = CGB_E or double_speed = '1' else
	             lyc_match and (ppu_reset or (std_ulogic_vector(ly_i) ?= lyc_compared));
	flag_read <= flag_late(3);

	hblank <= mode0 or mode0_late;

	-- Mode 2 as a read sees it: from the rise of the clock after nype's
	-- (vclk high, wuvu low; the next line is a visible one) to the rise
	-- before the scan stops (the last sprite, wuvu low); then mode 3, until
	-- mode 3 itself begins.
	scan_read : process (clk, ppu_reset)
	begin
		if ppu_reset = '1' then
			stat_scan      <= '0';
			stat_scan_done <= '0';
		elsif rising_edge(clk) then
			if vclk_i = '1' and nype = '1' and wuvu_i = '0' and (ly_i(7) and ly_i(4)) = '0' then
				stat_scan <= '1';
			elsif stat_scan = '1' and scanning = '1' and wuvu_i = '0' and scan_index = 39 then
				stat_scan      <= '0';
				stat_scan_done <= '1';
			elsif mode3 = '1' then
				stat_scan_done <= '0';
			end if;
		end if;
	end process;

	-- Mode 1 as a read sees it begins at the rise of the clock at h = 911
	-- (vclk, nype and wuvu high, LY 144).
	vblank_read : process (clk, ppu_reset)
	begin
		if ppu_reset = '1' then
			vblank_shown <= '0';
		elsif rising_edge(clk) then
			if vclk_i = '1' and nype = '1' and wuvu_i = '1' and ly_i(7) = '1' and ly_i(4) = '1' then
				vblank_shown <= '1';
			elsif mode1_i = '0' and mode1_late = "00" then
				vblank_shown <= '0';
			end if;
		end if;
	end process;

	int_stat <= (stat_enable(6) and lyc_match) or (stat_enable(5) and vclk_i and not mode1_i) or
	            (stat_enable(4) and mode1_i) or (stat_enable(3) and hblank);
	int_vbl  <= mode1_i;

	-- STAT's mode bits: 0 HBlank, 1 VBlank, 2 OAM scan, 3 pixels. (The
	-- latch RUPO between ROPO and bit 2 is held open outside reset.) On the
	-- CGB, mode 2 and mode 1 come from the reads' own signals above; mode 3
	-- shows from the end of mode 2's, while the scan finishes, so that a
	-- read never sees 0 between them (on the DMG, mode 2 reaches
	-- bit 1 through slower gates than OAM's, to the same effect: gbmicrotest's
	-- oam_write_l1_a, lcdon_to_stat3_c); and mode 1 wins over the others, as
	-- its read can now overlap the next scan's. OAM DMA hides the scan, as
	-- on the DMG.
	-- @hw die: SADU TEBY, XATY WUGA, RUPO SEGO; the enables PUZO POFO SASY
	-- POTE; LY: VEGA..WEZE; LYC: RETU..PUFY (not_if0)
	stat_m1 <= vblank_shown and (mode1_i or mode1_late(2)) when REVISION = CGB_E else
	           vblank_shown and mode1_i;
	stat_m2 <= stat_scan and not dma_run and not stat_m1;
	stat_m3 <= (mode3 or stat_scan_done) and not stat_m1;
	mode_read <= (stat_m2 or stat_m3) & (stat_m3 or stat_m1);

	ly_read <= std_ulogic_vector(ly_i) and ly_late;

	data_out <= 'Z' & stat_written & flag_read & mode_read when cpu_rd = '1' and addr = x"FF41" else
	        std_logic_vector(ly_read) when cpu_rd = '1' and addr = x"FF44" else
	        lyc when cpu_rd = '1' and addr = x"FF45" else
	        (others => 'Z');
end architecture;
