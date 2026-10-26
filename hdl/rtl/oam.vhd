-- SPDX-License-Identifier: MIT
--
-- OAM: the 160 bytes of the sprites' attributes, at FE00 to FE9F, as two
-- RAMs of 80 bytes (the even bytes, Y and tile, and the odd ones, X and
-- attributes) that are read or written together, one 16-bit word at a
-- time. Four sources address it: the CPU, OAM DMA, the OAM scan and the
-- sprite fetch. What the PPU reads is held in two stages of latches.
--
-- @hw die: dmg-schematics, category ppu-oam (sheet "OAM"), the latches of
-- ppu-ycomp and ppu-xcomp, and the RAM cells oam_a and oam_b (dmg-sim's
-- simplified SRAM); checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity oam is
	port (
		clk        : in std_ulogic;
		hard_reset : in std_ulogic;
		clk_t4     : in std_ulogic;
		dma_phi    : in std_ulogic;
		wuvu, wosu : in std_ulogic;

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		-- OAM DMA
		dma_run      : in std_ulogic;
		dma_addr     : in std_ulogic_vector(7 downto 0);
		dma_addr_ext : in std_ulogic;
		vram_to_oam  : in std_ulogic;
		d_pins       : in std_logic_vector(7 downto 0);  -- from the cartridge
		md           : in std_logic_vector(7 downto 0);  -- from VRAM

		-- The OAM scan and the sprite fetch
		mode2        : in std_ulogic;
		mode3        : in std_ulogic;
		scan_index   : in unsigned(5 downto 0);
		render_index : in std_logic_vector(5 downto 0);
		fetch_read   : in std_ulogic;  -- the sprite fetch reads OAM
		fetch_hold   : in std_ulogic;  -- ... and holds the second latches

		oam_addr : out std_ulogic_vector(7 downto 0);
		even     : out std_ulogic_vector(7 downto 0);  -- Y, or the tile
		odd      : out std_ulogic_vector(7 downto 0)   -- X, or the attributes
	);
end entity;

architecture rtl of oam is
	type ram_t is array (0 to 79) of std_ulogic_vector(7 downto 0);

	-- (dmg-sim starts with its cells at 0, which are the active-low data
	-- lines' levels: the bytes are FF.)
	signal ram_even, ram_odd : ram_t := (others => (others => '1'));

	signal fexx            : std_ulogic;
	signal cpu_access      : std_ulogic;  -- the CPU addresses OAM
	signal free            : std_ulogic;  -- nobody else uses OAM
	signal wuje            : std_ulogic := '0';
	signal maka            : std_ulogic := '0';
	signal dma_write       : std_ulogic;
	signal write           : std_ulogic;
	signal addr_i          : std_ulogic_vector(7 downto 0);
	signal word            : integer range 0 to 127;
	signal bus_even        : std_ulogic_vector(7 downto 0) := (others => '0');
	signal bus_odd         : std_ulogic_vector(7 downto 0) := (others => '0');
	signal cpu_read_oam    : std_ulogic;
	signal latch_open      : std_ulogic;
	signal precharge       : std_ulogic;
	signal word_line       : std_ulogic;
	signal read_ram        : std_ulogic;
	signal write_even      : std_ulogic;
	signal write_odd       : std_ulogic;
	signal first_even      : std_ulogic_vector(7 downto 0) := (others => '1');
	signal first_odd       : std_ulogic_vector(7 downto 0) := (others => '1');
	signal second_even     : std_ulogic_vector(7 downto 0) := (others => '1');
	signal second_odd      : std_ulogic_vector(7 downto 0) := (others => '1');
	signal bit_lines_even  : std_ulogic_vector(7 downto 0) := (others => '1');
	signal bit_lines_odd   : std_ulogic_vector(7 downto 0) := (others => '1');
begin
	-- @hw die: SARO (fexx), AJON AJUJ AMAB
	fexx       <= '1' when addr(15 downto 8) = x"FE" else '0';
	free       <= not dma_run and not mode2 and not (mode3 and not dma_run);
	cpu_access <= fexx and free;

	-- The address: the CPU's, DMA's, the scan's (word 0 of the sprite: Y
	-- and X) or the sprite fetch's (word 1: the tile and the attributes).
	-- @hw die: ASAM DUGA APAR BETE, and the not_if0 onto oam_~{a0..a7}_tri
	addr_i <= addr(7 downto 0) when mode2 = '0' and mode3 = '0' and dma_run = '0' else
	          dma_addr when dma_run = '1' else
	          std_ulogic_vector(scan_index) & "00" when mode2 = '1' else
	          to_x01(render_index) & "11";
	oam_addr <= addr_i;
	word     <= to_integer(to_01(unsigned(addr_i(7 downto 1))));

	-- DMA writes a byte while dma_phi and maka, clk_t4 half a period later,
	-- are both low.
	-- @hw die: CATY DECY, MAKA (dffr), NAXY POWU
	maka_ff : process (clk, hard_reset)
	begin
		if hard_reset = '1' then
			maka <= '0';
		elsif falling_edge(clk) then
			maka <= clk_t4 after tco;
		end if;
	end process;
	dma_write <= dma_run and not dma_phi and not maka;

	-- After the CPU writes FExx, until dma_phi rises.
	-- @hw die: WUJE (nor_latch), XUTO, XUPA
	wuje_latch : process (all)
	begin
		if fexx = '1' and cpu_wr = '1' then
			wuje <= '0';
		elsif dma_phi = '1' then
			wuje <= '1';
		end if;
	end process;

	-- @hw die: WYJA YLYC YNYC ZONE ZOFE
	write      <= (cpu_access and cpu_wr) or dma_write;
	write_odd  <= write and addr_i(0);
	write_even <= write and not addr_i(0);

	-- The data lines: DMA's source, the CPU's data, or the RAM while it is
	-- read; when nothing drives them, they keep their value. The drivers
	-- take a moment (1 ns): a shorter glitch does not reach the lines.
	-- @hw die: CEDE AZAR AZUL APAG, and their not_if0; WORY (the RAM's oe)
	data_lines : process (all)
	begin
		if dma_addr_ext = '1' then
			bus_even <= to_x01(d_pins) after 1 ns;
			bus_odd  <= to_x01(d_pins) after 1 ns;
		elsif vram_to_oam = '1' then
			bus_even <= to_x01(md) after 1 ns;
			bus_odd  <= to_x01(md) after 1 ns;
		elsif (not wuje and cpu_access) = '1' or (free and not fexx) = '1' then
			bus_even <= to_x01(data) after 1 ns;
			bus_odd  <= to_x01(data) after 1 ns;
		elsif read_ram = '1' then
			bus_even <= bit_lines_even after 1 ns;
			bus_odd  <= bit_lines_odd after 1 ns;
		else
			bus_even <= bus_even after 1 ns;
			bus_odd  <= bus_odd after 1 ns;
		end if;
	end process;

	-- The CPU reads OAM in phases E and F (before clk_t4); the first latches
	-- are open while the scan reads (wosu low), while the sprite fetch reads,
	-- and while the CPU reads.
	-- @hw die: BOTA, AJEP XUJA ASYT BODE, YVAL YRYV ZODO
	-- @fact oam-scan-during-dma
	cpu_read_oam <= fexx and cpu_rd and not clk_t4;
	latch_open   <= (mode2 and not wosu) or fetch_read or cpu_read_oam;

	-- The RAM: its bit lines are precharged while cota is high, and the
	-- second latches are open then; otherwise the word line reads the RAM
	-- onto the bit lines, or a write drives them.
	-- @hw die: CUFE XUJY AVER BYCU COTA WOVU, WAZO WUJY WAHE, WYCY WORY,
	-- WEGA WEJY WEMA WEXA; oam_a oam_b (simplified_sram)
	precharge <= (not (fexx or dma_run) or dma_phi) and not fetch_hold and not (mode2 and not (wosu and wuvu));
	word_line <= (latch_open or write) and not precharge;
	read_ram  <= latch_open and word_line;

	ram : process (all)
	begin
		if precharge = '1' then
			bit_lines_even <= (others => '1');
			bit_lines_odd  <= (others => '1');
		else
			if write_even = '1' then
				bit_lines_even <= bus_even;
				if word < 80 then
					ram_even(word) <= bus_even;
				end if;
			elsif word_line = '1' and word < 80 then
				bit_lines_even <= ram_even(word);
			end if;
			if write_odd = '1' then
				bit_lines_odd <= bus_odd;
				if word < 80 then
					ram_odd(word) <= bus_odd;
				end if;
			elsif word_line = '1' and word < 80 then
				bit_lines_odd <= ram_odd(word);
			end if;
		end if;
	end process;

	-- The first latches take the data lines, which the RAM drives while it
	-- is read; the second ones take the first while the bit lines are
	-- precharged, and hold during the next read.
	-- @hw die: YDYV YCEB ZUCA WONE ZAXE XAFU YSES ZECA, XYKY YRUM YSEX YVEL
	-- WYNO CYRA ZUVE ECED (dlatch); XUSO XEGU YJEX XYJU YBOG WYSO XOTE YZAB,
	-- YLOR ZYTY ZYVE ZEZY GOMO BAXO YZOS DEPO (dlatch_ee)
	first_latch : process (all)
	begin
		if latch_open = '1' then
			first_even <= bus_even;
			first_odd  <= bus_odd;
		end if;
	end process;

	second_latch : process (all)
	begin
		if precharge = '1' then
			second_even <= first_even;
			second_odd  <= first_odd;
		end if;
	end process;
	even <= second_even;
	odd  <= second_odd;

	-- The CPU reads OAM from the first latches, in phases G and H.
	-- @hw die: LEKO MYNU, WUKU WEWU GUKO WUME, and the buf_if0 onto d0..d7
	data_out <= first_odd when cpu_access = '1' and cpu_rd = '1' and clk_t4 = '1' and addr_i(0) = '1' else
	        first_even when cpu_access = '1' and cpu_rd = '1' and clk_t4 = '1' and addr_i(0) = '0' else
	        (others => 'Z');
end architecture;
