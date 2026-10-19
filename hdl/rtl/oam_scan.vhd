-- SPDX-License-Identifier: MIT
--
-- The OAM scan (mode 2): at the start of each line outside VBlank, the PPU
-- reads the 40 sprites' Y in OAM, one every two dots, and saves the first
-- ten that are on the line in the sprite store.
--
-- @hw die: dmg-schematics, categories ppu-objctl and ppu-ycomp (sheets
-- "Sprite control" and "Sprite Y comparator"); checked against dmg-sim
-- (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity oam_scan is
	port (
		clk       : in std_ulogic;
		ppu_reset : in std_ulogic;
		wuvu      : in std_ulogic;  -- the 2 MiHz clock
		wosu      : in std_ulogic;
		vclk      : in std_ulogic;  -- the end of a line
		ly        : in unsigned(7 downto 0);
		tall      : in std_ulogic;  -- LCDC bit 2: sprites of 16 lines
		dma_run   : in std_ulogic;

		sprite_y : in std_ulogic_vector(7 downto 0);  -- the Y of the sprite read from OAM

		line_start : out std_ulogic;  -- the line's rendering starts over
		mode2      : out std_ulogic;
		scanning   : out std_ulogic;  -- the scan runs (also during DMA)
		ceno       : out std_ulogic;  -- scanning, half a 2 MiHz period later
		stop_scan  : out std_ulogic;  -- the scan is done: mode 3 starts
		index      : out unsigned(5 downto 0);  -- the sprite being read

		y_match     : out std_ulogic;  -- the sprite is on the line
		sprite_line : out std_ulogic_vector(3 downto 0);  -- which of its lines
		save        : out std_ulogic_vector(9 downto 0)   -- save it in this slot
	);
end entity;

architecture rtl of oam_scan is
	signal start_scan       : std_ulogic := '0';
	signal anel             : std_ulogic := '0';
	signal line_start_i     : std_ulogic;
	signal scanning_i       : std_ulogic := '0';
	signal ceno_i           : std_ulogic := '0';
	signal index_i          : unsigned(5 downto 0) := (others => '0');
	signal last             : std_ulogic;
	signal scan_clock       : std_ulogic;
	signal byba, doba       : std_ulogic := '0';
	signal stop_scan_i      : std_ulogic;
	signal difference       : unsigned(8 downto 0);
	signal y_match_i        : std_ulogic;
	signal save_enable      : std_ulogic;
	signal not_saving       : std_ulogic := '0';
	signal count            : unsigned(3 downto 0) := (others => '0');
	signal count_clock      : std_ulogic;
begin
	-- The scan starts when wuvu rises at the end of a line (vclk), unless
	-- the next line is in VBlank (LY 144 to 153).
	-- @hw die: CATU (dffr), ABOV ALES XYVO; ANEL (dffr), AWOH
	start : process (wuvu, ppu_reset)
	begin
		if ppu_reset = '1' then
			start_scan <= '0';
			anel       <= '0';
		else
			if rising_edge(wuvu) then
				start_scan <= vclk and not (ly(7) and ly(4)) after tco;
			end if;
			if falling_edge(wuvu) then
				anel <= start_scan after tco;
			end if;
		end if;
	end process;

	-- From the start until anel follows, the line's rendering starts over:
	-- the scan's counters, the pixel counter and the sprite store are
	-- cleared.
	-- @hw die: BYHA ATEJ, ANOM
	line_start_i <= (start_scan and not anel) or ppu_reset;
	line_start   <= line_start_i;

	-- The stop clears the latch 1 ns before mode 3's latch sets
	-- (../rtl/render.vhd, XYMU): OAM is free for that moment, and a write
	-- of the CPU lands, as on dmg-sim (gbmicrotest's oam_write_l1_c).
	-- STAT does not see the moment (../rtl/lcd_timing.vhd).
	-- @hw die: BESU (nor_latch), ASEN; CENO (dffr)
	scan_latch : process (all)
	begin
		if ppu_reset = '1' or stop_scan_i = '1' then
			scanning_i <= '0';
		elsif start_scan = '1' then
			scanning_i <= '1';
		end if;
	end process;
	scanning <= scanning_i;

	ceno_ff : process (wuvu, ppu_reset)
	begin
		if ppu_reset = '1' then
			ceno_i <= '0';
		elsif rising_edge(wuvu) then
			ceno_i <= scanning_i after tco;
		end if;
	end process;
	ceno <= ceno_i;

	-- OAM DMA hides the scan from the OAM bus and from STAT, but it goes on.
	-- @hw die: ACYL BOGE
	mode2 <= scanning_i and not dma_run;

	-- The sprite's index counts at the rising edges of wuvu, up to 39.
	-- @hw die: YFEL WEWY GOSO ELYN FAHA FONY (dffr), GAVA; FETO
	last       <= index_i(5) and index_i(2) and index_i(1) and index_i(0);
	scan_clock <= last or wuvu;

	index_counter : process (scan_clock, line_start_i)
	begin
		if line_start_i = '1' then
			index_i <= (others => '0');
		elsif rising_edge(scan_clock) then
			index_i <= index_i + 1 after tco;
		end if;
	end process;
	index <= index_i;

	-- The scan stops when wuvu rises after the last sprite, until the clock
	-- rises: stop_scan is a pulse that starts mode 3.
	-- @hw die: BYBA (dffr), DOBA (dffr), BEBU AVAP
	stop_ff : process (wuvu, clk, line_start_i)
	begin
		if line_start_i = '1' then
			byba <= '0';
			doba <= '0';
		else
			if rising_edge(wuvu) then
				byba <= last after tco;
			end if;
			if rising_edge(clk) then
				doba <= byba after tco;
			end if;
		end if;
	end process;
	stop_scan_i <= byba and not doba and not line_start_i;
	stop_scan   <= stop_scan_i;

	-- A sprite is on the line when LY is in its 8 (or 16) lines, from Y - 16
	-- to Y - 9 (or Y - 1): the difference Y - LY - 1 is 8 to 15 (or 0 to
	-- 15). Its low four bits are the line within the sprite, from the
	-- bottom (15 for the first line of a sprite of 16 lines).
	-- @hw die: ERUC ENEF FECO GYKY GOPU FUWA GOJU WUHU (full_add), WOTA GESE
	difference  <= ('0' & unsigned(sprite_y)) + ('0' & not ly);
	y_match_i   <= '1' when difference(8) = '1' and difference(7 downto 4) = "0000"
	                    and (tall = '1' or difference(3) = '1') else '0';
	y_match     <= y_match_i;
	sprite_line <= std_ulogic_vector(difference(3 downto 0));

	-- A sprite on the line is saved while wosu is low, if the store has room.
	-- The count of saved sprites goes up at the end of each save, and stops
	-- at 10.
	-- @hw die: CARE DYTY, DEZY (dffr), CAKE BAKY, BESE CUXY BEGO DYBE (dffr),
	-- and the decoders of the slots
	save_enable <= not wosu and ceno_i and y_match_i;

	not_saving_ff : process (clk, ppu_reset)
	begin
		if ppu_reset = '1' then
			not_saving <= '0';
		elsif falling_edge(clk) then
			not_saving <= not save_enable after tco;
		end if;
	end process;

	count_clock <= (count(3) and count(1)) or not_saving;

	counter : process (count_clock, line_start_i)
	begin
		if line_start_i = '1' then
			count <= (others => '0');
		elsif rising_edge(count_clock) then
			count <= count + 1 after tco;
		end if;
	end process;

	slots : for i in 0 to 9 generate
		save(i) <= save_enable when count = i else '0';
	end generate;
end architecture;
