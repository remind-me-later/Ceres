-- SPDX-License-Identifier: MIT
--
-- The CGB's colour palettes: eight for the background and eight for the
-- sprites, four colours each, in two RAMs of 64 bytes (a colour is two
-- bytes, red in bits 0 to 4, green in 5 to 9, blue in 10 to 14), which the
-- CPU reaches through BCPS and BCPD (FF68, FF69) and OCPS and OCPD (FF6A,
-- FF6B); and the LCD's pixel pins, which take the colour of the pixel that
-- ../render.vhd sends.
--
-- The index registers (BCPS, OCPS) take the byte's address in bits 0 to 5
-- and, in bit 7, whether a write to the data register moves it on; bit 6
-- reads 1. While the PPU draws the RAMs are the PPU's: the CPU reads FF and
-- its writes are lost, though they still move the index on. That lock
-- begins 3 dots after mode 3 (2 on the first line after the LCD is turned
-- on) and ends 4 dots after it (2 in double speed). In the
-- DMG's mode, once the boot ROM is unmapped, the data registers answer no
-- more (the boot ROM writes the DMG mode's palettes before).
--
-- @hw inferred: no netlist of the CGB's chip is known. The registers are Pan
-- Docs' ("LCD Color Palettes"); the lock is Pan Docs' (mode 3), and its
-- edges ceres's (from SameBoy: Mode3PalettesLock, FirstLinePalettesLock,
-- HBlankPalettesUnlock);
-- the RAMs power up as zeros, as ceres has them (unknown on the chip); the
-- writes latch as the write strobe falls, as the DMG's registers do.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity colour_palettes is
	port (
		reset2   : in std_ulogic;
		cgb_mode : in std_ulogic;
		boot_on  : in std_ulogic;  -- the boot ROM is mapped
		clk4         : in std_ulogic;
		double_speed : in std_ulogic;
		start_line   : in std_ulogic;  -- the first line after the LCD is turned on
		mode3        : in std_ulogic;  -- the PPU draws

		-- The CPU's bus
		addr     : in  std_ulogic_vector(15 downto 0);
		data     : in  std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd   : in  std_ulogic;
		cpu_wr   : in  std_ulogic;

		-- The pixel, from the PPU
		pixel_obj     : in std_ulogic;
		pixel_palette : in std_ulogic_vector(2 downto 0);
		pixel_colour  : in std_ulogic_vector(1 downto 0);

		-- The LCD's pixel pins: red, green and blue, 5 bits each (red low)
		rgb : out std_ulogic_vector(14 downto 0)
	);
end entity;

architecture rtl of colour_palettes is
	type ram_t is array (0 to 63) of std_ulogic_vector(7 downto 0);

	signal bg_ram, obj_ram       : ram_t := (others => (others => '0'));
	signal bcps, ocps            : std_ulogic_vector(7 downto 0) := (others => '0');
	signal write_bcps, write_ocps : std_ulogic;
	signal write_bcpd, write_ocpd : std_ulogic;
	signal data_open             : std_ulogic;  -- the data registers answer
	signal bg_byte, obj_byte     : std_ulogic_vector(7 downto 0);
	signal colour                : natural range 0 to 31;
	signal since_start           : natural range 0 to 6 := 0;
	signal since_end             : natural range 0 to 8 := 8;
	signal locked                : std_ulogic;  -- the RAMs are the PPU's
begin
	-- The lock follows mode 3, a few half dots (the 4 MiHz clock's edges)
	-- late at each end.
	lock_edges : process (clk4)
	begin
		if clk4'event then
			if mode3 = '1' then
				since_end <= 0;
				if since_start < 6 then
					since_start <= since_start + 1;
				end if;
			else
				since_start <= 0;
				if since_end < 8 then
					since_end <= since_end + 1;
				end if;
			end if;
		end if;
	end process;
	locked <= '1' when mode3 = '1' and since_start >= 4 and start_line = '1' else
	          '1' when mode3 = '1' and since_start >= 6 else
	          '1' when mode3 = '0' and since_end < 4 else
	          '1' when mode3 = '0' and since_end < 8 and double_speed = '0' else
	          '0';

	write_bcps <= '1' when cpu_wr = '1' and addr = x"FF68" else '0';
	write_bcpd <= '1' when cpu_wr = '1' and addr = x"FF69" else '0';
	write_ocps <= '1' when cpu_wr = '1' and addr = x"FF6A" else '0';
	write_ocpd <= '1' when cpu_wr = '1' and addr = x"FF6B" else '0';

	data_open <= cgb_mode or boot_on;

	-- The index registers, which a write to their data register moves on
	-- when bit 7 is set (bits 0 to 5 count, and wrap); and the RAMs, which
	-- outside mode 3 take the write.
	background : process (write_bcps, write_bcpd, reset2)
	begin
		if reset2 = '1' then
			bcps <= (others => '0');
		elsif falling_edge(write_bcps) then
			bcps <= to_x01(data);
		elsif falling_edge(write_bcpd) and data_open = '1' then
			if locked = '0' then
				bg_ram(to_integer(unsigned(bcps(5 downto 0)))) <= to_x01(data);
			end if;
			if bcps(7) = '1' then
				bcps(5 downto 0) <= std_ulogic_vector(unsigned(bcps(5 downto 0)) + 1);
			end if;
		end if;
	end process;

	objects : process (write_ocps, write_ocpd, reset2)
	begin
		if reset2 = '1' then
			ocps <= (others => '0');
		elsif falling_edge(write_ocps) then
			ocps <= to_x01(data);
		elsif falling_edge(write_ocpd) and data_open = '1' then
			if locked = '0' then
				obj_ram(to_integer(unsigned(ocps(5 downto 0)))) <= to_x01(data);
			end if;
			if ocps(7) = '1' then
				ocps(5 downto 0) <= std_ulogic_vector(unsigned(ocps(5 downto 0)) + 1);
			end if;
		end if;
	end process;

	bg_byte  <= bg_ram(to_integer(unsigned(bcps(5 downto 0)))) when locked = '0' else x"FF";
	obj_byte <= obj_ram(to_integer(unsigned(ocps(5 downto 0)))) when locked = '0' else x"FF";

	data_out <= bcps or x"40" when cpu_rd = '1' and addr = x"FF68" else
	            bg_byte when cpu_rd = '1' and addr = x"FF69" and data_open = '1' else
	            ocps or x"40" when cpu_rd = '1' and addr = x"FF6A" else
	            obj_byte when cpu_rd = '1' and addr = x"FF6B" and data_open = '1' else
	            (others => 'Z');

	-- The pixel's colour: its two bytes, without bit 15.
	colour <= to_integer(unsigned(to_01(pixel_palette & pixel_colour)));
	rgb    <= obj_ram(2 * colour + 1)(6 downto 0) & obj_ram(2 * colour) when pixel_obj = '1' else
	          bg_ram(2 * colour + 1)(6 downto 0) & bg_ram(2 * colour);
end architecture;
