-- SPDX-License-Identifier: MIT
--
-- The rendering of a line (mode 3): the pixel counter, the fetcher of the
-- background and the window, the fine scroll, the window, the fetch of
-- sprites, the two pixel shifters, the mixing of their pixels, the
-- palettes, and the LCD's pixel pins.
--
-- The pixels move at the PPU's dot clock, clkpipe, which stops while the
-- fetcher is busy and while a sprite is fetched. The fetcher counts 0 to 5:
-- 0 and 1 read the tile map, 2 and 3 the tile's low byte, 4 and 5 its high
-- byte; then the background shifter loads the tile's eight pixels.
--
-- @hw die: dmg-schematics, categories ppu-cycles, ppu-bgscroll,
-- ppu-window, ppu-bgfifo, ppu-objfifo, ppu-mux, ppu-pal, the sprite fetch
-- of ppu-ycomp, the sprite shifters of ppu-xcomp, and the pixel counter of
-- ppu-stat; checked against dmg-sim (../check/chip_tb.vhd).
--
-- The CGB adds the colours (@hw inferred, each where it is; REVISION picks
-- the chip, and the DMG's code is the netlist's):
-- - the tile map's attributes, in VRAM's bank 1 at the map's address, read
--   with the tile number (VRAM has a second data bus for bank 1): the tile's
--   bank, its flips in X and Y, its palette and its priority over sprites;
-- - a sprite's CGB attributes in OAM: its bank and its palette (in the
--   CGB's mode only: the DMG's mode keeps the DMG's bits);
-- - priority between sprites by their index in OAM, unless OPRI says by X
--   (as the DMG, where the sprite fetched first wins);
-- - LCDC bit 0, in the CGB's mode, turns off the priorities over sprites
--   instead of the background;
-- - the pixel goes out as a palette and a colour, which ../colour_palettes.vhd
--   turns into 15 bits: in the CGB's mode the attributes' palette and the
--   pixel's colour; in the DMG's, BGP's, OBP0's and OBP1's shade, in the
--   background's palette 0 and the sprites' palettes 0 and 1.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;
use work.revisions.all;

entity render is
	generic (
		REVISION : revision_t := DMG_B
	);
	port (
		clk        : in std_ulogic;
		ppu_reset  : in std_ulogic;
		hard_reset : in std_ulogic;
		hclk       : in std_ulogic;
		mode1      : in std_ulogic;
		ly         : in unsigned(7 downto 0);
		lcdc       : in std_ulogic_vector(7 downto 0);

		cgb_mode     : in std_ulogic := '0';  -- the CGB's mode, not the DMG's
		dmg_priority : in std_ulogic := '1';  -- sprites take priority by X (the CGB's OPRI)

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		-- From the OAM scan and the sprite store
		line_start : in std_ulogic;
		stop_scan  : in std_ulogic;
		ceno       : in std_ulogic;
		x_match    : in std_ulogic;
		sprite_line : in std_logic_vector(3 downto 0);
		tile       : in std_ulogic_vector(7 downto 0);  -- OAM's even byte: the sprite's tile
		attributes : in std_ulogic_vector(7 downto 0);  -- OAM's odd byte
		sprite_index : in std_logic_vector(5 downto 0) := (others => '0');  -- the fetched sprite's index in OAM (the CGB's)

		md      : in std_logic_vector(7 downto 0);   -- VRAM's data
		md_attr : in std_ulogic_vector(7 downto 0) := (others => '0');  -- VRAM's bank 1 at the same address (the CGB's)

		mode3 : out std_ulogic;
		mode0 : out std_ulogic;
		mode0_early : out std_ulogic;  -- mode 0 without the DMG's delay (for the CGB)
		px    : out unsigned(7 downto 0);  -- the pixel counter

		-- To OAM and the sprite store
		rendering     : out std_ulogic;  -- mode 3, after the scan's last read
		fetch_read    : out std_ulogic;  -- the sprite fetch reads OAM
		fetch_hold    : out std_ulogic;
		sprite_loaded : out std_ulogic;  -- the fetched sprite's pixels are loaded

		-- The VRAM addresses of the fetches, and when they apply
		ma          : out std_logic_vector(12 downto 0);
		bank        : out std_ulogic;  -- VRAM's bank, for a tile's bytes (the CGB's)
		fetching    : out std_ulogic;  -- the background or the window
		sprite_read : out std_ulogic;  -- a sprite
		sprite_mrd  : out std_ulogic;  -- ... and reads VRAM now

		-- LCD pins
		cp : out std_ulogic;  -- the pixel clock
		st : out std_ulogic;  -- the line's start
		ld : out std_ulogic_vector(1 downto 0);  -- the DMG's pixel

		-- The CGB's pixel, for its palettes
		pixel_obj     : out std_ulogic;                     -- a sprite's
		pixel_palette : out std_ulogic_vector(2 downto 0);
		pixel_colour  : out std_ulogic_vector(1 downto 0)
	);
end entity;

architecture rtl of render is
	-- Registers
	signal scy, scx, wy, wx    : std_ulogic_vector(7 downto 0) := (others => '0');
	signal bgp, obp0, obp1     : std_ulogic_vector(7 downto 0) := (others => '0');

	-- Mode 3 and the pixel counter
	signal mode3_i, mode0_i    : std_ulogic;
	signal voga                : std_ulogic := '0';  -- mode 3's end, a clock later
	signal stop_mode3          : std_ulogic;
	signal px_i                : unsigned(7 downto 0) := (others => '0');
	signal px9                 : std_ulogic := '0';
	signal clkpipe             : std_ulogic;
	signal dot                 : std_ulogic;  -- tyfa: a dot passes
	signal segu                : std_ulogic;         -- not dot
	signal bg_tile_clock       : std_ulogic;
	signal wx_clk              : std_ulogic;
	signal discard             : std_ulogic := '0';  -- roxy: the fine scroll discards

	-- The fetcher
	signal fetch_count         : unsigned(2 downto 0) := (others => '0');
	signal fetch_clock         : std_ulogic;
	signal fetch_reset         : std_ulogic;
	signal fetch_done          : std_ulogic;
	signal lyzu                : std_ulogic := '0';  -- count bit 0, at the clock's rise
	signal lovy                : std_ulogic := '0';  -- fetch_done, at the clock's fall
	signal fetching_i          : std_ulogic := '0';
	signal nyka, pory, pygo    : std_ulogic := '0';  -- fetch_done, half a clock later each
	signal started             : std_ulogic := '0';  -- poky
	signal myso                : std_ulogic;
	signal tile_map_cycle      : std_ulogic;
	signal bit_plane_cycle     : std_ulogic;
	signal bg_tile             : std_ulogic_vector(7 downto 0) := (others => '0');
	signal bg_low              : std_ulogic_vector(7 downto 0) := (others => '0');
	signal bg_shift_a          : std_ulogic_vector(7 downto 0) := (others => '0');
	signal bg_shift_b          : std_ulogic_vector(7 downto 0) := (others => '0');
	signal bg_attr             : std_ulogic_vector(7 downto 0) := (others => '0');
	signal bg_palette          : std_ulogic_vector(2 downto 0) := (others => '0');
	signal bg_priority         : std_ulogic := '0';

	-- The window
	signal wy_match, wxy_match : std_ulogic;
	signal sary                : std_ulogic := '0';  -- wy_match, at hclk's rise
	signal wy_latch            : std_ulogic := '0';  -- WY matched in this frame
	signal pyco, nunu          : std_ulogic := '0';  -- wxy_match, at a dot, then the clock's fall
	signal nopa                : std_ulogic := '0';  -- in_window, at the clock's rise
	signal in_window           : std_ulogic := '0';
	signal win_start           : std_ulogic;
	signal win_reset           : std_ulogic;
	signal win_x               : unsigned(4 downto 0) := (others => '0');
	signal win_y               : unsigned(7 downto 0) := (others => '0');
	signal rydy                : std_ulogic := '0';  -- the window's first tile is not fetched yet
	signal sovy                : std_ulogic := '0';  -- rydy, at the clock's rise

	-- The fine scroll
	signal fine                : unsigned(2 downto 0) := (others => '0');
	signal fine_reset          : std_ulogic;
	signal puxa, nyze          : std_ulogic := '0';  -- the fine count matches SCX, and later
	signal paho                : std_ulogic := '0';  -- px bit 3, at a dot
	signal ryfa, rene          : std_ulogic := '0';  -- a tile's last dot, and a clock later
	signal fine_done           : std_ulogic;  -- pova
	signal restart             : std_ulogic;  -- tevo
	signal line_started        : std_ulogic := '0';  -- ruju

	-- The sprite fetch
	signal sprite_step         : unsigned(2 downto 0) := (others => '0');  -- tese tuly toxe
	signal step_reset          : std_ulogic;
	signal sobu, suda          : std_ulogic := '0';  -- a sprite fetch requested, and later
	signal fetch_start         : std_ulogic;  -- ryce
	signal busy                : std_ulogic := '0';  -- taka
	signal tobu, vonu, seba    : std_ulogic := '0';  -- step bit 1, delayed by clocks
	signal tyfo                : std_ulogic := '0';  -- step bit 0, at the clock's fall
	signal sycu                : std_ulogic;
	signal loaded              : std_ulogic;  -- wuty
	signal sprite_cycles       : std_ulogic;  -- sp_bp_cys
	signal sp_low_a, sp_low_b  : std_ulogic_vector(7 downto 0) := (others => '0');
	signal sp_data             : std_ulogic_vector(7 downto 0);
	signal sp_a, sp_b          : std_ulogic_vector(7 downto 0) := (others => '0');
	signal sp_palette          : std_ulogic_vector(7 downto 0) := (others => '0');
	signal sp_priority         : std_ulogic_vector(7 downto 0) := (others => '0');
	type palettes_t is array (0 to 7) of std_ulogic_vector(2 downto 0);
	type indices_t is array (0 to 7) of unsigned(5 downto 0);
	signal sp_cgb_palette      : palettes_t := (others => (others => '0'));
	signal sp_index            : indices_t := (others => (others => '0'));
	signal cgb_palette         : std_ulogic_vector(2 downto 0);
	signal sprite_on           : std_ulogic_vector(7 downto 0);
	signal before              : std_ulogic_vector(7 downto 0);  -- the fetched sprite comes first in OAM

	-- The pixel
	signal bg_a, bg_b          : std_ulogic;
	signal obj_a, obj_b        : std_ulogic;
	signal bg_shown            : std_ulogic;
	signal show_sprite         : std_ulogic;
	signal pixel               : std_ulogic_vector(1 downto 0);
	signal palette             : std_ulogic_vector(7 downto 0);
	signal shade               : std_ulogic_vector(1 downto 0);
begin
	----------------------------------------------------------------------
	-- The registers: latches, open while the CPU writes them.
	-- @hw die: GAVE..FUNY (SCY), DATY..BAKE (SCX), NESO..NAFU (WY), MYPA..NUKU
	-- (WX), PAVO..MENA (BGP), XUFU..XANA (OBP0), MOXY..LUXO (OBP1)
	-- (drlatch_ee, dlatch_ee)
	----------------------------------------------------------------------
	registers : process (all)
	begin
		if hard_reset = '1' then
			scy <= (others => '0');
			scx <= (others => '0');
			wy  <= (others => '0');
			wx  <= (others => '0');
		elsif cpu_wr = '1' then
			case addr is
				when x"FF42" => scy <= to_x01(data);
				when x"FF43" => scx <= to_x01(data);
				when x"FF4A" => wy  <= to_x01(data);
				when x"FF4B" => wx  <= to_x01(data);
				when others  => null;
			end case;
		end if;
		if cpu_wr = '1' then
			case addr is
				when x"FF47" => bgp  <= to_x01(data);
				when x"FF48" => obp0 <= to_x01(data);
				when x"FF49" => obp1 <= to_x01(data);
				when others  => null;
			end case;
		end if;
	end process;

	data_out <= scy  when cpu_rd = '1' and addr = x"FF42" else
	        scx  when cpu_rd = '1' and addr = x"FF43" else
	        bgp  when cpu_rd = '1' and addr = x"FF47" else
	        obp0 when cpu_rd = '1' and addr = x"FF48" else
	        obp1 when cpu_rd = '1' and addr = x"FF49" else
	        wy   when cpu_rd = '1' and addr = x"FF4A" else
	        wx   when cpu_rd = '1' and addr = x"FF4B" else
	        (others => 'Z');

	----------------------------------------------------------------------
	-- Mode 3 starts when the scan stops, and ends when the pixel counter
	-- reaches 167 with no sprite to fetch there.
	-- @hw die: XYMU (nor_latch), WEGO, VOGA (dffr), WODU XUGU XENA, TARU
	----------------------------------------------------------------------
	-- (The latch sets a little after the scan's stop also clears the
	-- fetcher: when mode 3 starts, the fetcher's count is already 0. For
	-- that moment OAM is free to the CPU, and a write reaches it: gbmicrotest's
	-- oam_write_l1_c.)
	-- @rule oam-free-between-scan-and-mode-3
	mode3_latch : process (all)
	begin
		if ppu_reset = '1' or voga = '1' then
			mode3_i <= '0';
		elsif stop_scan = '1' then
			mode3_i <= '1' after 1 ns;
		end if;
	end process;
	mode3 <= mode3_i;

	stop_mode3 <= '1' when px_i = 167 and x_match = '0' else '0';
	mode0_i    <= stop_mode3 and not mode1;
	-- On the chip, the pixel counter's clock comes 8 ns after hclk, and mode
	-- 0 rises 14 ns after it, when LY = LYC's flag has long changed at that
	-- edge. Here the counter's clock comes first, so mode 0's rise waits:
	-- a write to LYC that clears the flag at the end of mode 3 drops STAT's
	-- interrupt line for a moment, and the rise sets IF's flag (Gambatte's
	-- lycdisable_ff45_3, measured in dmg-sim).
	-- @fact mode-0-rises-after-lyc-flag, @fact lcd-off-in-vblank-mode-0-source
	-- @rule mode-0-rises-after-lyc-flag
	mode0      <= '1' after 14 ns when mode0_i = '1' else '0';
	mode0_early <= mode0_i;

	voga_ff : process (clk, line_start)
	begin
		if line_start = '1' then
			voga <= '0';
		elsif rising_edge(clk) then
			voga <= stop_mode3 after tco;
		end if;
	end process;

	-- A dot passes while the clock is high, unless the fetcher has not
	-- finished its first tile, a sprite is to be fetched, or mode 3 is over.
	-- clkpipe rises at the end of a dot; the fine scroll holds it high while
	-- it discards pixels.
	-- @hw die: TYFA SOCY VYBO SEGU, ROXY (nor_latch), SACU
	dot     <= not rydy and started and not x_match and not stop_mode3 and clk;
	segu    <= not dot;
	clkpipe <= segu or discard;

	-- The pixel counter counts the dots. Its upper four bits count when
	-- bit 3 falls.
	-- @hw die: XEHO SAVY XODU XYDO (dffr), clocked by SACU; TUHU TUKY TAKO
	-- SYBE (dffr), clocked by TOCA; TADY
	px_low : process (clkpipe, line_start)
	begin
		if line_start = '1' then
			px_i(3 downto 0) <= (others => '0');
		elsif rising_edge(clkpipe) then
			px_i(3 downto 0) <= px_i(3 downto 0) + 1 after tco;
		end if;
	end process;

	px_high : process (px_i(3), line_start)
	begin
		if line_start = '1' then
			px_i(7 downto 4) <= (others => '0');
		elsif falling_edge(px_i(3)) then
			px_i(7 downto 4) <= px_i(7 downto 4) + 1 after tco;
		end if;
	end process;
	px <= px_i;

	-- @hw die: WUSA (nor_latch), XAJO
	px9_latch : process (all)
	begin
		if ppu_reset = '1' or voga = '1' then
			px9 <= '0';
		elsif px_i(0) = '1' and px_i(3) = '1' then
			px9 <= '1';
		end if;
	end process;

	----------------------------------------------------------------------
	-- The fetcher counts at the falling edges of the clock, up to 5, and
	-- starts over when restarted (bg_cy_rst).
	-- @hw die: LAXU MESU NYVA (dffr), LEBO MOCE, NYXU
	----------------------------------------------------------------------
	fetch_reset <= stop_scan or win_start or restart;
	fetch_done  <= fetch_count(2) and fetch_count(0) and not fetch_reset;
	fetch_clock <= not (clk and not fetch_done);

	fetch_counter : process (fetch_clock, fetch_reset)
	begin
		if fetch_reset = '1' then
			fetch_count <= (others => '0');
		elsif rising_edge(fetch_clock) then
			fetch_count <= fetch_count + 1 after tco;
		end if;
	end process;

	-- @hw die: LYZU LOVY (dffr), LURY, LONY (nand_latch), LENA
	fetch_ff : process (clk, mode3_i, fetch_reset)
	begin
		if mode3_i = '0' then
			lyzu <= '0';
		elsif rising_edge(clk) then
			lyzu <= fetch_count(0) after tco;
		end if;
		if fetch_reset = '1' then
			lovy <= '0';
		elsif falling_edge(clk) then
			lovy <= fetch_done after tco;
		end if;
	end process;

	fetching_latch : process (all)
	begin
		if fetch_reset = '1' then
			fetching_i <= '1';
		elsif lovy = '1' or mode3_i = '0' then
			fetching_i <= '0';
		end if;
	end process;
	fetching <= fetching_i;

	-- The first fetch of the line done: the pixels can start.
	-- @hw die: NYKA PORY PYGO (dffr), NAFY, POKY (nor_latch)
	first_fetch : process (clk, mode3_i, win_start)
	begin
		if mode3_i = '0' or win_start = '1' then
			nyka <= '0';
			pory <= '0';
		else
			if rising_edge(clk) then
				nyka <= fetch_done after tco;
			end if;
			if falling_edge(clk) then
				pory <= nyka after tco;
			end if;
		end if;
		if mode3_i = '0' then
			pygo <= '0';
		elsif rising_edge(clk) then
			pygo <= pory after tco;
		end if;
	end process;

	started_latch : process (all)
	begin
		if mode3_i = '0' then
			started <= '0';
		elsif pygo = '1' then
			started <= '1';
		end if;
	end process;

	-- @hw die: MYSO, NOGU NETA POTU, XUHA
	myso            <= mode3_i and fetch_count(0) and not lyzu;
	bit_plane_cycle <= fetching_i and (fetch_count(1) or fetch_count(2));
	tile_map_cycle  <= fetching_i and not (fetch_count(1) or fetch_count(2));

	-- The tile register takes VRAM's data at the end of count 1 (the tile
	-- map) and of count 5 (the tile's high byte, which the shifter loads);
	-- the low byte is latched in count 3.
	-- @hw die: RAWA..PYJU (dffr_cc_q), MOFU LEBO LUVE LABU; LEGU..NEFO
	-- (dlatch_ee_q), NYDY
	-- When the window starts while the counter is at 3, it is reset to 0 and
	-- its bit 1 falls a moment before MYSO does: this clock pulses for no
	-- time, and the register takes the bus, which is still precharged.
	-- dmg-sim, whose gates are slower than that pulse is wide, does not
	-- show it (build/check on m3_window_timing differs there); the screens
	-- of the real DMG do: with this pulse the model draws
	-- m3_lcdc_win_en_change_multiple_wx and m3_scy_change as the hardware
	-- does, and without it (a delay on the clock) it does not.
	-- @fact window-start-tile-register-pulse
	-- @rule tile-register-takes-the-pulse
	bg_tile_clock <= myso and not fetch_count(1);

	tile_ff : process (bg_tile_clock)
	begin
		if falling_edge(bg_tile_clock) then
			bg_tile <= to_x01(md) after tco;
		end if;
	end process;

	low_latch : process (all)
	begin
		if myso = '1' and fetch_count = 3 then
			bg_low <= to_x01(md);
		end if;
	end process;

	-- The tile's attributes, from VRAM's bank 1, latched with the tile
	-- number.
	-- @hw inferred: SameBoy reads them in the same T-cycle as the tile
	-- number on any CGB ("the CGB has a 16-bit data bus for the VRAM"); the
	-- latch is a guess, the low byte's latch in count 1
	cgb_attributes : if REVISION /= DMG_B generate
		attr_latch : process (all)
		begin
			if myso = '1' and fetch_count = 1 then
				bg_attr <= to_x01(md_attr);
			end if;
		end process;
	end generate;

	-- The background shifter: two shift registers, loaded with a tile's
	-- bit planes while the fetcher restarts, and shifted at clkpipe. A tile
	-- flipped in X loads mirrored; its palette and priority load with it
	-- (the shifter holds one tile at a time).
	-- @hw die: MYDE NOZO MOJU MACU NEPO MODU NEDA PYBO, TOMY TACA SADY RYSA
	-- SOBO SETU RALU SOHU (dffsr), LUXA LOZE
	-- @hw inferred: the flip, the palette and the priority (Pan Docs "BG Map
	-- Attributes"; where they load is a guess)
	bg_shifter : process (clkpipe, fetch_reset, bg_low, bg_tile, bg_attr)
	begin
		if fetch_reset = '1' then
			if bg_attr(5) = '1' then
				for i in 0 to 7 loop
					bg_shift_a(i) <= bg_low(7 - i);
					bg_shift_b(i) <= bg_tile(7 - i);
				end loop;
			else
				bg_shift_a <= bg_low;
				bg_shift_b <= bg_tile;
			end if;
			bg_palette  <= bg_attr(2 downto 0);
			bg_priority <= bg_attr(7);
		elsif rising_edge(clkpipe) then
			bg_shift_a <= bg_shift_a(6 downto 0) & '0' after tco;
			bg_shift_b <= bg_shift_b(6 downto 0) & '0' after tco;
		end if;
	end process;

	----------------------------------------------------------------------
	-- The window.
	----------------------------------------------------------------------
	-- WY matches LY at the rising edge of hclk; then the window is armed for
	-- the rest of the frame.
	-- @hw die: PAFU ROGE, SARY (dffr), REJO (nor_latch), REPU
	wy_match <= '1' when lcdc(5) = '1' and wy = std_ulogic_vector(ly) else '0';

	sary_ff : process (hclk, ppu_reset)
	begin
		if ppu_reset = '1' then
			sary <= '0';
		elsif rising_edge(hclk) then
			sary <= wy_match after tco;
		end if;
	end process;

	wy_latch_p : process (all)
	begin
		if mode1 = '1' or ppu_reset = '1' then
			wy_latch <= '0';
		elsif sary = '1' then
			wy_latch <= '1';
		end if;
	end process;

	-- WX matches the pixel counter. On the CGB only while the window is on:
	-- on the DMG, WY's latch is enough, and a match with the window off
	-- still holds the fetcher's next tile back for a dot (RYFA, below).
	-- @hw die: NOGY NUKO
	-- @hw inferred: cgb-acid2's picture of a real CGB (the footer, under WX
	-- 5F with the window turned off: no dot lost), ceres (the window's start
	-- needs LCDC bit 5)
	wxy_match <= '1' when (REVISION = DMG_B or lcdc(5) = '1') and wy_latch = '1'
	                      and wx = std_ulogic_vector(px_i) else '0';

	-- The window starts at a dot where WX matches, after two clocks.
	-- @hw die: PYCO (dffr), clocked by ROCO; NUNU (dffr); PYNU (nor_latch),
	-- XOFO; NOPA (dffr), NUNY
	win_reset <= not lcdc(5) or line_start;

	window_start : process (dot, clk, ppu_reset)
	begin
		if ppu_reset = '1' then
			pyco <= '0';
			nunu <= '0';
			nopa <= '0';
		else
			if rising_edge(dot) then
				pyco <= wxy_match after tco;
			end if;
			if falling_edge(clk) then
				nunu <= pyco after tco;
			end if;
			if rising_edge(clk) then
				nopa <= in_window after tco;
			end if;
		end if;
	end process;

	in_window_latch : process (all)
	begin
		if win_reset = '1' then
			in_window <= '0';
		elsif nunu = '1' then
			in_window <= '1';
		end if;
	end process;

	win_start <= in_window and not nopa;

	-- The window's tile column counts the window's tiles; its line counts
	-- the lines on which the window was drawn.
	-- @hw die: WYKA WODY WOBO WYKO XOLO (dffr), VETU; VYNO VUJO VYMU TUFU
	-- TAXA TOZO TATE TEKE (dffr), WAZY, SYNY
	wx_clk <= restart and in_window;

	win_x_counter : process (wx_clk, win_reset)
	begin
		if win_reset = '1' then
			win_x <= (others => '0');
		elsif rising_edge(wx_clk) then
			win_x <= win_x + 1 after tco;
		end if;
	end process;

	win_y_counter : process (in_window, mode1, ppu_reset)
	begin
		if mode1 = '1' or ppu_reset = '1' then
			win_y <= (others => '0');
		elsif falling_edge(in_window) then
			win_y <= win_y + 1 after tco;
		end if;
	end process;

	-- After the window starts, rydy holds the pixels until the fetcher has
	-- fetched the window's first tile.
	-- @hw die: RYDY PUKU (nor latch), SOVY (dffr), TUXY SUZU
	rydy_latch : process (all)
	begin
		if pory = '1' or ppu_reset = '1' then
			rydy <= '0';
		elsif win_start = '1' then
			rydy <= '1';
		end if;
	end process;

	sovy_ff : process (clk, ppu_reset)
	begin
		if ppu_reset = '1' then
			sovy <= '0';
		elsif rising_edge(clk) then
			sovy <= rydy after tco;
		end if;
	end process;

	----------------------------------------------------------------------
	-- The fine scroll: at the start of the line, the first SCX & 7 dots are
	-- discarded; a counter of the dots stops at 7.
	-- @hw die: RYKU ROGA RUBU (dffr), PECU ROZE, PASO; SUHA SYBY SOZU RONE
	-- POHU; PUXA NYZE (dffr), POVA; ROXY (nor_latch)
	----------------------------------------------------------------------
	fine_reset <= not mode3_i or restart;

	fine_counter : process (dot, fine_reset)
	begin
		if fine_reset = '1' then
			fine <= (others => '0');
		elsif falling_edge(dot) and fine /= 7 then
			fine <= fine + 1 after tco;
		end if;
	end process;

	fine_ff : process (dot, clk, mode3_i)
	begin
		if mode3_i = '0' then
			puxa <= '0';
			nyze <= '0';
			paho <= '0';
		else
			if rising_edge(dot) then
				puxa <= '1' after tco when discard = '1' and std_ulogic_vector(fine) = scx(2 downto 0) else '0' after tco;
				paho <= px_i(3) after tco;
			end if;
			if falling_edge(clk) then
				nyze <= puxa after tco;
			end if;
		end if;
	end process;
	fine_done <= puxa and not nyze;

	discard_latch : process (all)
	begin
		if fine_done = '1' then
			discard <= '0';
		elsif mode3_i = '0' then
			discard <= '1';
		end if;
	end process;

	-- The fetcher restarts when the fine counter reaches 7 without a window
	-- (the next tile), when the window's first tile is fetched, and once at
	-- the start of the line.
	-- @hw die: RYFA (dffr), clocked by SEGU; RENE (dffr), SEKO; SUZU; SUVU
	-- TAVE; TEVO
	ryfa_ff : process (segu, clk, mode3_i)
	begin
		if mode3_i = '0' then
			ryfa <= '0';
			rene <= '0';
		else
			if rising_edge(segu) then
				ryfa <= '1' after tco when fine = 7 and wxy_match = '0' else '0' after tco;
			end if;
			if rising_edge(clk) then
				rene <= ryfa after tco;
			end if;
		end if;
	end process;
	restart <= (ryfa and not rene) or (sovy and not rydy)
	           or (mode3_i and not started and nyka and pory);

	-- ST, the start of the line's pixels.
	-- @hw die: RUJU POME POFY RUZE ST
	st_latch : process (all)
	begin
		if paho = '1' or ppu_reset = '1' then
			line_started <= '1';
		elsif stop_scan = '1' then
			line_started <= '0';
		end if;
	end process;
	st <= not line_started;

	-- CP, the pixel clock: from the tenth pixel on, the dots; and the end of
	-- the fine scroll.
	-- @hw die: TOBA SEMU RYPO CP
	cp <= (px9 and clkpipe) or fine_done;

	----------------------------------------------------------------------
	-- The sprite fetch: when a sprite matches, the fetch waits for the
	-- fetcher to finish, then reads the sprite's tile and attributes in
	-- OAM, and its two bytes in VRAM, in six steps of the clock.
	-- @hw die: SOBU SUDA (dffr), RYCE TEKY, SECA; TOXE TULY TESE (dffr),
	-- SABE TAME; TAKA (nand_latch), VEKU; TOBU VONU SEBA TYFO (dffr)
	----------------------------------------------------------------------
	start_ff : process (clk)
	begin
		if rising_edge(clk) then
			sobu <= x_match and not rydy and fetch_done and not busy after tco;
		end if;
		if falling_edge(clk) then
			suda <= sobu after tco;
		end if;
	end process;
	fetch_start <= sobu and not suda;
	step_reset  <= fetch_start or ppu_reset or line_start;

	steps : process (clk, step_reset)
	begin
		if step_reset = '1' then
			sprite_step <= (others => '0');
		elsif rising_edge(clk) and sprite_step /= 5 then
			sprite_step <= sprite_step + 1 after tco;
		end if;
	end process;

	busy_latch : process (all)
	begin
		if step_reset = '1' then
			busy <= '1';
		elsif loaded = '1' or (mode3_i and not started and nyka and pory) = '1' then
			busy <= '0';
		end if;
	end process;

	sprite_ff : process (clk, mode3_i)
	begin
		if mode3_i = '0' then
			tobu <= '0';
			vonu <= '0';
		elsif rising_edge(clk) then
			tobu <= sprite_step(1) after tco;
			vonu <= tobu after tco;
		end if;
	end process;

	sprite_ff2 : process (clk, mode3_i)
	begin
		if falling_edge(clk) then
			tyfo <= sprite_step(0) after tco;
			if mode3_i = '0' then
				seba <= '0' after tco;
			else
				seba <= vonu after tco;
			end if;
		end if;
	end process;

	-- @hw die: SYCU TUVO WEFY TACU VAPE, TYNO VUSA WUTY, SAKY TYSO TEXY,
	-- RACA ROZO TOPU SUSE
	sycu          <= sprite_step(0) and mode3_i and not tyfo;
	fetch_read    <= mode3_i and not sprite_step(1) and not sprite_step(2) and not tyfo;
	fetch_hold    <= mode3_i and not sprite_step(1) and not sprite_step(2) and not (not sprite_step(0) and tyfo);
	loaded        <= tyfo and sprite_step(0) and seba and vonu;
	sprite_loaded <= loaded;
	sprite_cycles <= mode3_i and (sprite_step(1) or vonu);
	sprite_read   <= sprite_cycles;
	sprite_mrd    <= sprite_cycles and not (not sprite_step(0) and tyfo);
	rendering     <= mode3_i and not ceno;

	-- The sprite's two bytes, mirrored when the sprite is flipped in X.
	-- @hw die: PUTE..PULY (mux), XONO; PEFO..RYDU, REWO..SEGA (dlatch_ee_q)
	sp_data <= md(0) & md(1) & md(2) & md(3) & md(4) & md(5) & md(6) & md(7)
	               when attributes(5) = '1' and sprite_cycles = '1' else to_x01(md);

	sprite_latches : process (all)
	begin
		if vonu = '1' and sycu = '1' then
			sp_low_a <= sp_data;
		end if;
		if sprite_step(1) = '1' and sycu = '1' then
			sp_low_b <= sp_data;
		end if;
	end process;

	-- The sprite shifter: the fetched sprite's pixels load where the shifter
	-- has transparent pixels, so that the sprites fetched earlier win.
	-- @hw die: NYLU PEFU NATY PYJO VARE WEBA VANU VUPY, NURO MASO LEFE LESU
	-- WYHO WORA VAFO WUFY (dffsr); the palette and priority shifters (ppu-
	-- xcomp); MEFU..WOPE LESY..: sprite_on
	--
	-- With priority by OAM index, a pixel of the fetched sprite that is not
	-- transparent also loads over one of a sprite with a greater index; each
	-- pixel keeps its sprite's index for that, and its CGB palette.
	-- @hw inferred: SameBoy (fifo_overlay_object_row); Pan Docs ("Object
	-- Priority and Conflicts")
	dmg_sprites : if REVISION = DMG_B generate
		free_pixels : for i in 0 to 7 generate
			sprite_on(i) <= loaded and not sp_a(i) and not sp_b(i);
		end generate;

		sprite_shifter : process (clkpipe, sprite_on, sp_low_a, sp_low_b, attributes)
		begin
			for i in 0 to 7 loop
				if sprite_on(i) = '1' then
					sp_a(i)        <= sp_low_a(i);
					sp_b(i)        <= sp_low_b(i);
					sp_palette(i)  <= attributes(4);
					sp_priority(i) <= attributes(7);
				elsif rising_edge(clkpipe) then
					if i = 0 then
						sp_a(i)        <= '0' after tco;
						sp_b(i)        <= '0' after tco;
						sp_palette(i)  <= '0' after tco;
						sp_priority(i) <= '1' after tco;
					else
						sp_a(i)        <= sp_a(i - 1) after tco;
						sp_b(i)        <= sp_b(i - 1) after tco;
						sp_palette(i)  <= sp_palette(i - 1) after tco;
						sp_priority(i) <= sp_priority(i - 1) after tco;
					end if;
				end if;
			end loop;
		end process;
	else generate
		free_pixels : for i in 0 to 7 generate
			before(i)    <= '1' when to_x01(unsigned(sprite_index)) < sp_index(i) else '0';
			sprite_on(i) <= loaded and ((not sp_a(i) and not sp_b(i))
			                            or (not dmg_priority and (sp_low_a(i) or sp_low_b(i)) and before(i)));
		end generate;

		-- The CGB's palette: in the CGB's mode only.
		cgb_palette <= attributes(2 downto 0) when cgb_mode = '1' else "000";

		sprite_shifter : process (clkpipe, sprite_on, sp_low_a, sp_low_b, attributes, cgb_palette, sprite_index)
		begin
			for i in 0 to 7 loop
				if sprite_on(i) = '1' then
					sp_a(i)           <= sp_low_a(i);
					sp_b(i)           <= sp_low_b(i);
					sp_palette(i)     <= attributes(4);
					sp_priority(i)    <= attributes(7);
					sp_cgb_palette(i) <= cgb_palette;
					sp_index(i)       <= to_x01(unsigned(sprite_index));
				elsif rising_edge(clkpipe) then
					if i = 0 then
						sp_a(i)           <= '0' after tco;
						sp_b(i)           <= '0' after tco;
						sp_palette(i)     <= '0' after tco;
						sp_priority(i)    <= '1' after tco;
						sp_cgb_palette(i) <= "000" after tco;
						sp_index(i)       <= (others => '1') after tco;
					else
						sp_a(i)           <= sp_a(i - 1) after tco;
						sp_b(i)           <= sp_b(i - 1) after tco;
						sp_palette(i)     <= sp_palette(i - 1) after tco;
						sp_priority(i)    <= sp_priority(i - 1) after tco;
						sp_cgb_palette(i) <= sp_cgb_palette(i - 1) after tco;
						sp_index(i)       <= sp_index(i - 1) after tco;
					end if;
				end if;
			end loop;
		end process;
	end generate;

	----------------------------------------------------------------------
	-- The pixel: the sprite's, unless it is transparent, or behind a
	-- background pixel that is not 0; then the palette.
	-- @hw die: WOXA XULA NULY, RAJY TADE, RUTA RYFU POKA; NYPO NUMA NUXO
	-- POBU NELO NURA, VYRO VATA VOLO VUGO WALY WUFU, LEDO LARU LYKY LOPU MUFA
	-- MOKA; PERO PATY REMY RAVO LD0 LD1
	----------------------------------------------------------------------
	-- In the CGB's mode, LCDC bit 0 does not hide the background.
	-- @hw inferred: Pan Docs ("LCDC.0 — BG and Window enable/priority");
	-- SameBoy
	bg_shown <= lcdc(0) or cgb_mode;
	bg_a  <= bg_shift_a(7) and lcdc(0) when REVISION = DMG_B else bg_shift_a(7) and bg_shown;
	bg_b  <= bg_shift_b(7) and lcdc(0) when REVISION = DMG_B else bg_shift_b(7) and bg_shown;
	obj_a <= sp_a(7) and lcdc(1);
	obj_b <= sp_b(7) and lcdc(1);

	-- A sprite's A plane is the tile's high byte (read second) and B the low
	-- one (read first), the other way round from the background's A and B:
	-- the palette picks 2*A + B (the cells of OBP0 and OBP1).
	-- A sprite is behind a background pixel that is not 0 when its priority
	-- bit says so. On the CGB, also when the tile's says so, and in the
	-- CGB's mode, not while LCDC bit 0 is clear.
	-- @hw inferred: the tile's priority and LCDC bit 0, Pan Docs ("BG-to-OBJ
	-- Priority in CGB Mode"); SameBoy
	show_sprite <= (obj_a or obj_b) and not (sp_priority(7) and (bg_a or bg_b)) when REVISION = DMG_B else
	               (obj_a or obj_b) and not ((sp_priority(7) or bg_priority) and lcdc(0) and (bg_a or bg_b));
	pixel       <= obj_a & obj_b when show_sprite = '1' else bg_b & bg_a;

	palette <= bgp when show_sprite = '0' else
	           obp1 when sp_palette(7) = '1' else
	           obp0;

	dmg_pixel : if REVISION = DMG_B generate
		with pixel select ld <=
			palette(1 downto 0) when "00",
			palette(3 downto 2) when "01",
			palette(5 downto 4) when "10",
			palette(7 downto 6) when others;
	else generate
		with pixel select shade <=
			palette(1 downto 0) when "00",
			palette(3 downto 2) when "01",
			palette(5 downto 4) when "10",
			palette(7 downto 6) when others;

		-- To the colour palettes: in the CGB's mode, the pixel and its
		-- palette; in the DMG's, the DMG palettes' shade, in the background's
		-- palette 0 or the sprites' palette 0 or 1.
		-- @hw inferred: Pan Docs ("Compatibility palettes": BGP, OBP0 and
		-- OBP1 index into the CGB palettes); SameBoy
		pixel_obj     <= show_sprite;
		pixel_colour  <= pixel when cgb_mode = '1' else shade;
		pixel_palette <= sp_cgb_palette(7) when cgb_mode = '1' and show_sprite = '1' else
		                 bg_palette when cgb_mode = '1' else
		                 "00" & sp_palette(7) when show_sprite = '1' else
		                 "000";
	end generate;

	----------------------------------------------------------------------
	-- The VRAM addresses of the fetches.
	-- @hw die: the full adders of ppu-bgscroll and their not_if0 onto ~ma;
	-- the window's (ppu-window); the tile's (ppu-bgfifo); the sprite's
	-- (ppu-ycomp)
	----------------------------------------------------------------------
	addresses : process (all)
		variable map_x  : unsigned(7 downto 0);
		variable map_y  : unsigned(7 downto 0);
		variable row    : std_ulogic_vector(3 downto 0);
		variable sprite : std_ulogic_vector(7 downto 0);
	begin
		map_x := px_i + unsigned(scx);
		map_y := ly + unsigned(scy);
		ma    <= (others => 'Z');
		bank  <= '0';
		if tile_map_cycle = '1' and in_window = '0' then
			ma <= "11" & lcdc(3) & std_logic_vector(map_y(7 downto 3)) & std_logic_vector(map_x(7 downto 3));
		elsif tile_map_cycle = '1' and in_window = '1' then
			ma <= "11" & lcdc(6) & std_logic_vector(win_y(7 downto 3)) & std_logic_vector(win_x);
		elsif bit_plane_cycle = '1' and in_window = '0' then
			-- A tile flipped in Y reads its rows from the bottom; its bank is
			-- the attributes'.
			-- @hw inferred: Pan Docs ("BG Map Attributes")
			row(2 downto 0) := std_ulogic_vector(map_y(2 downto 0)) xor (2 downto 0 => bg_attr(6));
			ma   <= (bg_tile(7) nor lcdc(4)) & bg_tile & std_logic_vector(row(2 downto 0)) & fetch_count(2);
			bank <= bg_attr(3);
		elsif bit_plane_cycle = '1' and in_window = '1' then
			row(2 downto 0) := std_ulogic_vector(win_y(2 downto 0)) xor (2 downto 0 => bg_attr(6));
			ma   <= (bg_tile(7) nor lcdc(4)) & bg_tile & std_logic_vector(row(2 downto 0)) & fetch_count(2);
			bank <= bg_attr(3);
		elsif sprite_cycles = '1' then
			-- The stored line counts from the bottom: flipped in Y, it is the
			-- row; otherwise its inverse is.
			row := to_x01(sprite_line) xor (3 downto 0 => not attributes(6));
			sprite := tile;
			if lcdc(2) = '1' then
				sprite(0) := row(3);
			end if;
			ma <= '0' & sprite & row(2 downto 0) & vonu;
			-- The CGB's mode only.
			-- @hw inferred: Pan Docs ("Byte 3 — Attributes/Flags"); ceres
			bank <= attributes(3) and cgb_mode;
		end if;
	end process;
end architecture;
