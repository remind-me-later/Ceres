-- SPDX-License-Identifier: MIT
--
-- The VRAM bus: the pins MA0 to MA12, MD0 to MD7, /MCS, /MRD and /MWR to
-- the VRAM chip, and who addresses VRAM: the CPU (outside mode 3), OAM DMA,
-- or the PPU's fetches.
--
-- @hw die: dmg-schematics, category ppu-vram (sheet "VRAM interface");
-- checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;

use work.clocking.all;

entity vram_bus is
	port (
		hard_reset    : in std_ulogic;
		clk_t4        : in std_ulogic;
		cpu_wr_sync   : in std_ulogic;
		cpu_ext_cs_en : in std_ulogic;
		t1_only       : in std_ulogic;

		-- The CPU's bus
		addr          : in    std_ulogic_vector(15 downto 0);
		data          : inout std_logic_vector(7 downto 0);
		cpu_rd        : in    std_ulogic;
		write         : in    std_ulogic;  -- the CPU core writes
		non_vram_mreq : in    std_ulogic;

		-- OAM DMA
		vram_to_oam : in std_ulogic;
		dma_addr    : in std_ulogic_vector(12 downto 0);

		-- The PPU
		mode3       : in std_ulogic;
		fetching    : in std_ulogic;
		sprite_read : in std_ulogic;
		sprite_mrd  : in std_ulogic;
		render_ma   : in std_logic_vector(12 downto 0);

		ma : inout std_logic_vector(12 downto 0);  -- the address inside the chip
		md : inout std_logic_vector(7 downto 0);   -- the data inside the chip

		-- Pins
		ma_pins : out   std_ulogic_vector(12 downto 0);
		md_pins : inout std_logic_vector(7 downto 0);
		mcs_n   : inout std_logic;
		mrd_n   : inout std_logic;
		mwr_n   : inout std_logic
	);
end entity;

architecture rtl of vram_bus is
	signal soto      : std_ulogic := '0';
	signal render_settled : std_logic_vector(12 downto 0) := (others => 'Z');
	signal tuto      : std_ulogic;  -- test mode: the VRAM pins drive the bus
	signal fexx_ffxx : std_ulogic;
	signal sose      : std_ulogic;
	signal cpu_select : std_ulogic;  -- tole
	signal cpu_writes : std_ulogic;  -- ruvy
	signal cpu_vram  : std_ulogic;  -- sere: the CPU accesses VRAM
	signal cpu_write : std_ulogic;  -- sazo: ... and writes it
	signal reading   : std_ulogic;  -- racu: nobody reads VRAM
	signal idle      : std_ulogic;  -- sutu: nobody uses VRAM
	signal writing   : std_ulogic;  -- sohy: the CPU does not write VRAM
begin
	-- Test mode: T1 alone makes /MCS, /MRD and /MWR inputs, every other time.
	-- @hw die: SOTO (dffr), SYCY, TUTO
	soto_ff : process (t1_only, hard_reset)
	begin
		if hard_reset = '1' then
			soto <= '0';
		elsif falling_edge(t1_only) then
			soto <= not soto after tco;
		end if;
	end process;
	tuto <= t1_only and not soto;

	-- The CPU addresses VRAM: 8000 to 9FFF, without leaving the chip, in
	-- phases C to H; outside mode 3.
	-- @hw die: TEFA SOSE TUCA TOLE ROPY SERE, TEGU SALE RUVY SAZO
	fexx_ffxx <= '1' when addr(15 downto 9) = "1111111" else '0';
	sose      <= addr(15) and not fexx_ffxx and not non_vram_mreq;
	cpu_select <= not to_x01(mcs_n) when tuto = '1' else sose and cpu_ext_cs_en;
	cpu_writes <= not to_x01(mrd_n) when tuto = '1' else sose and write;
	cpu_vram   <= cpu_select and not mode3;
	cpu_write  <= cpu_vram and cpu_writes;

	-- The address: the CPU's, outside mode 3 and DMA; DMA's; or the PPU's.
	-- (Nobody drives it at times in mode 3: it keeps its value.)
	-- @hw die: XANE XEDU, AHOC, and the not_if0 onto ~ma0..~ma12
	address : process (all)
	begin
		if vram_to_oam = '0' and mode3 = '0' then
			ma <= addr(12 downto 0);
		elsif vram_to_oam = '1' then
			ma <= dma_addr;
		elsif render_settled(0) /= 'Z' then
			ma <= render_settled;
		end if;
	end process;

	-- The drivers of the PPU's addresses take a moment: a glitch shorter
	-- than that does not reach the lines, which keep their value.
	render_settled <= render_ma after 1 ns;
	ma_pins <= to_x01(ma);

	-- The data: the CPU's while it writes VRAM, the pins' otherwise.
	-- @hw die: RAHU, TEME..SUZA (buf_if0); RELA RENA, RODY..RAKU (not_if1);
	-- MD0..MD7 (pad_bidir_pu), ROFA, REGE..RYZE RURA..RADY
	md      <= to_x01(data) when cpu_write = '1' else to_x01(md_pins);
	md_pins <= to_x01(md) when cpu_write = '1' else (others => 'H');

	-- The CPU reads VRAM in phases G and H.
	-- @hw die: TYVY SEBY, RUGA..TOKU (not_if1)
	data <= to_x01(md) when cpu_vram = '1' and cpu_rd = '1' and clk_t4 = '1' else (others => 'Z');

	-- /MCS is low while anyone uses VRAM; /MRD unless the CPU writes it, or
	-- in mode 3, unless the PPU reads it; /MWR while the CPU writes it, in
	-- phases E to G.
	-- @hw die: SUTU TODE SOKY ROSU; RYLU SOHO RAWA RACU SEMA REFO RUTE RONU;
	-- TUJA TYJY SOHY TAXY SYSY SOFY RAGU; ~MCS ~MRD ~MWR (pad_bidir)
	idle    <= not (fetching or vram_to_oam or sprite_read or cpu_vram);
	reading <= (not (sose and write) nand not mode3) and not sprite_mrd and not fetching
	           and not vram_to_oam;
	writing <= not (sose and cpu_wr_sync and cpu_vram);

	mcs_n <= 'Z' when tuto = '1' else idle;
	mrd_n <= 'Z' when tuto = '1' else reading;
	mwr_n <= 'Z' when tuto = '1' else writing;
end architecture;
