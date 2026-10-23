-- SPDX-License-Identifier: MIT
--
-- LCDC (FF40), and the clock and the resets of the PPU.
--
-- @hw die: dmg-schematics, categories ppu-control and ppu-decode (sheet
-- "PPU decode & video control"); checked against dmg-sim
-- (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;

entity lcd_control is
	port (
		clk    : in std_ulogic;
		reset2 : in std_ulogic;

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		lcdc : out std_ulogic_vector(7 downto 0);

		ppu_4mhz   : out std_ulogic;  -- the PPU's clock: the chip's, inverted
		hard_reset : out std_ulogic;  -- reset2
		ppu_reset  : out std_ulogic   -- the LCD is off, or reset2
	);
end entity;

architecture rtl of lcd_control is
	signal write_lcdc : std_ulogic;
	signal lcdc_i     : std_ulogic_vector(7 downto 0) := (others => '0');
begin
	-- @hw die: AZOF ZAXY ZEME, ALET
	ppu_4mhz <= not clk;

	-- @hw die: DULA CUNU XORE
	hard_reset <= reset2;

	-- LCDC is a set of latches, open while the CPU writes it.
	-- @hw die: VYXE XYLO XYMO XAFO WEXU WYMO WOKY XONA (drlatch_ee), WARU
	-- XUBO XURE, XARE
	-- It opens 3 ns after the CPU's strobe and the address, as dmg-sim's gates
	-- do, and closes with it: opening at once showed a change of the sprites'
	-- size to the OAM scan that dmg-sim does not (Gambatte's late_sizechange2).
	write_lcdc <= '1' after 3 ns when cpu_wr = '1' and addr = x"FF40" else '0';

	lcdc_latch : process (all)
	begin
		if reset2 = '1' then
			lcdc_i <= (others => '0');
		elsif write_lcdc = '1' then
			lcdc_i <= to_x01(data);
		end if;
	end process;
	lcdc <= lcdc_i;

	-- Bit 7 turns the LCD on; while it is off, the PPU is held in reset.
	-- (A second latch of bit 7, XECY, and a flip-flop behind it, XUVA,
	-- drive nothing.)
	-- @hw die: XODO XAPO PYRY, LYHA LYFE
	ppu_reset <= not lcdc_i(7) or reset2;

	-- @hw die: VYRE WYCE, WYPO XERO WYJU WUKA VOKE VATO VAHA XEBU (not_if0)
	data_out <= lcdc_i when cpu_rd = '1' and addr = x"FF40" else (others => 'Z');
end architecture;
