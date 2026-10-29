-- SPDX-License-Identifier: MIT
--
-- The VRAM: 16 KiB in two banks of 8 KiB (MA13 is the bank). It is written
-- for as long as /MCS and /MWR are low, which /MRD does not override, and
-- drives MD while /MCS and /MRD are low and /MWR high. A second data bus
-- shows bank 1 at the same address, for the PPU's fetcher, which reads a
-- tile's number and its attributes at once.
--
-- @hw inferred: the DMG's VRAM chip on its board (an LH5164, as dmg-sim has
-- it, ../board/gameboy.vhd on branch hdl) twice as large: the CGB keeps its
-- VRAM inside the chip, and nothing tells how that RAM differs. The second
-- bus is SameBoy's reading ("the CGB has a 16-bit data bus for the VRAM").
-- @rule cgb-vram-in-chip

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity video_ram is
	generic (
		-- Whether the VRAM starts with zeros (as the boot ROM leaves it)
		-- instead of arbitrary bytes
		ZERO : boolean := false
	);
	port (
		ma                  : in    std_ulogic_vector(13 downto 0);
		md                  : inout std_logic_vector(7 downto 0);
		mcs_n, mrd_n, mwr_n : in    std_logic;
		md_attr             : out   std_ulogic_vector(7 downto 0)  -- bank 1 at MA0 to MA12
	);
end entity;

architecture rtl of video_ram is
	type bytes_t is array (natural range <>) of std_ulogic_vector(7 downto 0);

	-- RAM holds arbitrary bytes at power-on: these come from a fixed
	-- generator (a 32-bit xorshift), so that runs repeat.
	-- @hw inferred: the contents are unknown
	function power_on(zero : boolean) return bytes_t is
		variable r : bytes_t(0 to 16383) := (others => (others => '0'));
		variable x : unsigned(31 downto 0) := to_unsigned(2, 32);
	begin
		if zero then
			return r;
		end if;
		for i in r'range loop
			x := x xor shift_left(x, 13);
			x := x xor shift_right(x, 17);
			x := x xor shift_left(x, 5);
			r(i) := std_ulogic_vector(x(7 downto 0));
		end loop;
		return r;
	end function;

	signal bytes : bytes_t(0 to 16383) := power_on(ZERO);
begin
	write : process (all)
	begin
		if to_x01(mcs_n) = '0' and to_x01(mwr_n) = '0' and not is_x(ma) then
			bytes(to_integer(unsigned(ma))) <= to_x01(md);
		end if;
	end process;

	md <= (others => 'Z') when not (to_x01(mcs_n) = '0' and to_x01(mrd_n) = '0' and to_x01(mwr_n) = '1') else
	      (others => 'X') when is_x(ma) else
	      std_logic_vector(bytes(to_integer(unsigned(ma))));

	md_attr <= (others => 'X') when is_x(ma(12 downto 0)) else
	           bytes(8192 + to_integer(unsigned(ma(12 downto 0))));
end architecture;
