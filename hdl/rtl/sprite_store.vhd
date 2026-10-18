-- SPDX-License-Identifier: MIT
--
-- The sprite store: the ten sprites that the OAM scan found on the line,
-- each with its X, its line and its index in OAM; and, during mode 3, the
-- match of their X with the pixel counter, which starts their fetch.
--
-- @hw die: dmg-schematics, categories ppu-objreg, ppu-xcomp and ppu-xprio
-- (sheets "Sprite store", "Sprite X matchers" and "Sprite X priority"),
-- and the index register of ppu-oam; checked against dmg-sim
-- (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

package sprite_slots is
	type x_t is array (0 to 9) of std_ulogic_vector(7 downto 0);
	type line_t is array (0 to 9) of std_ulogic_vector(3 downto 0);
	type index_t is array (0 to 9) of std_ulogic_vector(5 downto 0);
end package;

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

use work.sprite_slots.all;

entity sprite_store is
	port (
		wuvu       : in std_ulogic;
		line_start : in std_ulogic;
		ppu_reset  : in std_ulogic;

		-- From the OAM scan
		oam_index   : in unsigned(5 downto 0);  -- OAM's address, bits 7 to 2
		save        : in std_ulogic_vector(9 downto 0);
		sprite_x    : in std_ulogic_vector(7 downto 0);
		sprite_line : in std_ulogic_vector(3 downto 0);

		-- From the rendering
		px      : in unsigned(7 downto 0);  -- the pixel counter
		enabled   : in std_ulogic;          -- sprites on (LCDC bit 1), in mode 3
		rendering : in std_ulogic;          -- mode 3, after the scan
		loaded  : in std_ulogic;            -- the fetched sprite's pixels are loaded

		x_match     : out std_ulogic;  -- a sprite starts at this pixel
		index       : out std_ulogic_vector(5 downto 0);  -- the matching sprite's
		line        : out std_ulogic_vector(3 downto 0)   -- index and line
	);
end entity;

architecture rtl of sprite_store is
	signal scanned   : std_ulogic_vector(5 downto 0) := (others => '0');
	signal xs        : x_t := (others => (others => '1'));
	signal lines     : line_t := (others => (others => '0'));
	signal indexes   : index_t := (others => (others => '0'));
	signal fetched   : std_ulogic_vector(9 downto 0) := (others => '0');
	signal matches   : std_ulogic_vector(9 downto 0);
	signal selected  : std_ulogic_vector(9 downto 0);
begin
	-- The index of the sprite being scanned, as OAM's address had it at the
	-- last rising edge of wuvu.
	-- @hw die: XADU XEDY ZUZE XOBE YDUF XECU (dffr_cc), clocked by WUDA
	scanned_ff : process (wuvu)
	begin
		if rising_edge(wuvu) then
			scanned <= std_ulogic_vector(oam_index) after tco;
		end if;
	end process;

	-- Each slot is a set of latches, open while the scan saves to it. A
	-- slot whose sprite has been fetched, and every slot at the start of a
	-- line, has its X cleared to FF, which matches no pixel.
	-- @hw die: the drlatch_ee of the X (ppu-xcomp) and the dlatch_ee of the
	-- line and the index (ppu-objreg); EBOJ CEDY EGAV GOTA XUDY WAFY WOMY
	-- WAPO EXUQ FONO (dffr), clocked by WUTY
	slot : for i in 0 to 9 generate
		x_latch : process (all)
		begin
			if line_start = '1' or fetched(i) = '1' then
				xs(i) <= (others => '1');
			elsif save(i) = '1' then
				xs(i) <= sprite_x;
			end if;
		end process;

		line_latch : process (all)
		begin
			if save(i) = '1' then
				lines(i)   <= sprite_line;
				indexes(i) <= scanned;
			end if;
		end process;

		fetched_ff : process (loaded, line_start)
		begin
			if line_start = '1' then
				fetched(i) <= '0';
			elsif rising_edge(loaded) then
				fetched(i) <= selected(i) after tco;
			end if;
		end process;

		-- @hw die: the nor4 of the low and high nibbles (ppu-xcomp), and the
		-- nand3 with AROR (ppu-xprio)
		matches(i) <= enabled when unsigned(xs(i)) = px else '0';
	end generate;

	-- Of the slots that match, the first one is fetched.
	-- @hw die: GUVA ENUT EMOL GYFY GONO GEGA XOJA GUTU FOXA GUZE (nor2), and
	-- the or2 chain GEZE FUMA GEDE WUTO XYLA WEJA WYLA FAVO GYGA
	priority : process (all)
		variable before : std_ulogic;
	begin
		before := '0';
		for i in 0 to 9 loop
			selected(i) <= matches(i) and not before;
			before      := before or matches(i);
		end loop;
	end process;

	-- @hw die: FEFY FOVE FEPO
	x_match <= '1' when matches /= "0000000000" else '0';

	-- The index lines carry the scanned index outside rendering, and the
	-- selected slot's index while rendering; when nothing drives them, they
	-- keep their value.
	-- @hw die: WUZY WYSE ZYSU WYDA WUCO WEZA, and the not_if0 of the slots
	-- onto oam_render_a2..a7 and sprite_y_store0..3
	index_lines : process (all)
	begin
		if rendering = '0' then
			index <= scanned;
		else
			for i in 0 to 9 loop
				if selected(i) = '1' then
					index <= indexes(i);
				end if;
			end loop;
		end if;
	end process;

	-- The line lines carry the Y comparator's line while no slot matches.
	line_lines : process (all)
	begin
		line <= sprite_line;
		for i in 0 to 9 loop
			if selected(i) = '1' then
				line <= lines(i);
			end if;
		end loop;
	end process;
end architecture;
