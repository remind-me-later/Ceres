-- SPDX-License-Identifier: MIT
--
-- The VRAM bus: MA0 to MA13, MD0 to MD7, /MCS, /MRD and /MWR to the VRAM,
-- and who addresses VRAM: the CPU (outside mode 3), OAM DMA, or the PPU's
-- fetches. And VBK (FF4F), the bank of VRAM that the CPU sees.
--
-- @hw die: dmg-schematics, category ppu-vram (sheet "VRAM interface");
-- checked against dmg-sim (../check/chip_tb.vhd).
-- @hw inferred: the CGB's VRAM has two banks of 8 KiB, and its chip keeps
-- them inside (the DMG's pins become wires to it). MA13 is the bank: VBK's
-- for the CPU and for OAM DMA, the fetcher's for the PPU. VBK takes writes
-- in the CGB's mode only, and reads back on any CGB (Pan Docs "FF4F — VBK";
-- SameBoy's GB_IO_VBK).

library ieee;
use ieee.std_logic_1164.all;

use work.clocking.all;

entity vram_bus is
	port (
		hard_reset    : in std_ulogic;
		reset2        : in std_ulogic;
		cgb_mode      : in std_ulogic;
		clk_t4        : in std_ulogic;
		cpu_wr_sync   : in std_ulogic;
		cpu_ext_cs_en : in std_ulogic;
		t1_only       : in std_ulogic;

		-- The CPU's bus
		addr          : in    std_ulogic_vector(15 downto 0);
		data          : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd        : in    std_ulogic;
		write         : in    std_ulogic;  -- the CPU core writes
		non_vram_mreq : in    std_ulogic;

		-- OAM DMA
		vram_to_oam : in std_ulogic;
		dma_addr    : in std_ulogic_vector(12 downto 0);

		-- The PPU
		clk4         : in std_ulogic;
		double_speed : in std_ulogic;
		start_line   : in std_ulogic;  -- the first line after the LCD is turned on
		mode3        : in std_ulogic;
		fetching    : in std_ulogic;
		sprite_read : in std_ulogic;
		sprite_mrd  : in std_ulogic;
		render_ma   : in std_logic_vector(12 downto 0);
		render_bank : in std_ulogic;  -- the bank the fetcher reads

		ma : inout std_logic_vector(13 downto 0);  -- the address inside the chip
		md : inout std_logic_vector(7 downto 0);   -- the data inside the chip

		-- To the VRAM (pins on the DMG)
		ma_pins : out   std_ulogic_vector(13 downto 0);
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
	signal ppu_owns  : std_ulogic;  -- mode 3 takes VRAM from the CPU
	signal mode3_edges : natural range 0 to 10 := 0;
	signal cpu_write : std_ulogic;  -- sazo: ... and writes it
	signal reading   : std_ulogic;  -- racu: nobody reads VRAM
	signal idle      : std_ulogic;  -- sutu: nobody uses VRAM
	signal writing   : std_ulogic;  -- sohy: the CPU does not write VRAM
	signal write_vbk : std_ulogic;
	signal vbk       : std_ulogic := '0';
	signal render_bank_settled : std_ulogic := '0';
begin
	-- VBK: the CGB's mode only. Latched as the write strobe falls, as the
	-- DMG's registers are.
	write_vbk <= '1' when cpu_wr_sync = '1' and addr = x"FF4F" else '0';

	vbk_reg : process (write_vbk, reset2)
	begin
		if reset2 = '1' then
			vbk <= '0';
		elsif falling_edge(write_vbk) and cgb_mode = '1' then
			vbk <= to_x01(data(0));
		end if;
	end process;

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

	-- Mode 3 takes VRAM from the CPU, but on the first line after the LCD
	-- is turned on, at normal speed, only 10 half dots (5 dots) after it
	-- begins.
	-- @hw inferred: ceres (its FirstLineMode3 to FirstLineVramLock, from
	-- SameBoy), Gambatte's enable_display/ly0_late_vramr and _vramw (CGB:
	-- the access lands, DMG: it does not)
	mode3_count : process (clk4, mode3)
	begin
		if mode3 = '0' then
			mode3_edges <= 0;
		elsif clk4'event and mode3_edges < 10 then
			mode3_edges <= mode3_edges + 1;
		end if;
	end process;
	ppu_owns <= '0' when mode3 = '1' and start_line = '1' and double_speed = '0' and mode3_edges < 10 else
	            mode3;

	-- The CPU addresses VRAM: 8000 to 9FFF, without leaving the chip, in
	-- phases C to H; outside mode 3.
	-- @hw die: TEFA SOSE TUCA TOLE ROPY SERE, TEGU SALE RUVY SAZO
	fexx_ffxx <= '1' when addr(15 downto 9) = "1111111" else '0';
	sose      <= addr(15) and not fexx_ffxx and not non_vram_mreq;
	cpu_select <= not to_x01(mcs_n) when tuto = '1' else sose and cpu_ext_cs_en;
	cpu_writes <= not to_x01(mrd_n) when tuto = '1' else sose and write;
	cpu_vram   <= cpu_select and not ppu_owns;
	cpu_write  <= cpu_vram and cpu_writes;

	-- The address: the CPU's, outside mode 3 and DMA; DMA's; or the PPU's.
	-- (Nobody drives it at times in mode 3: it keeps its value.)
	-- @hw die: XANE XEDU, AHOC, and the not_if0 onto ~ma0..~ma12
	address : process (all)
	begin
		if vram_to_oam = '0' and ppu_owns = '0' then
			ma <= vbk & addr(12 downto 0);
		elsif vram_to_oam = '1' then
			ma <= vbk & dma_addr;
		elsif render_settled(0) /= 'Z' then
			ma <= render_bank_settled & render_settled;
		end if;
	end process;

	-- The drivers of the PPU's addresses take a moment: a glitch shorter
	-- than that does not reach the lines, which keep their value.
	render_settled      <= render_ma after 2 ns;
	render_bank_settled <= render_bank after 2 ns;
	ma_pins <= to_x01(ma);

	-- The data: the CPU's while it writes VRAM, the pins' otherwise.
	-- @hw die: RAHU, TEME..SUZA (buf_if0); RELA RENA, RODY..RAKU (not_if1);
	-- MD0..MD7 (pad_bidir_pu), ROFA, REGE..RYZE RURA..RADY
	md      <= to_x01(data) when cpu_write = '1' else to_x01(md_pins);
	md_pins <= to_x01(md) when cpu_write = '1' else (others => 'H');

	-- The CPU reads VRAM in phases G and H.
	-- @hw die: TYVY SEBY, RUGA..TOKU (not_if1)
	data_out <= to_x01(md) when cpu_vram = '1' and cpu_rd = '1' and clk_t4 = '1' else
	            "1111111" & vbk when cpu_rd = '1' and addr = x"FF4F" else
	            (others => 'Z');

	-- /MCS is low while anyone uses VRAM; /MRD unless the CPU writes it, or
	-- in mode 3, unless the PPU reads it; /MWR while the CPU writes it, in
	-- phases E to G.
	-- @hw die: SUTU TODE SOKY ROSU; RYLU SOHO RAWA RACU SEMA REFO RUTE RONU;
	-- TUJA TYJY SOHY TAXY SYSY SOFY RAGU; ~MCS ~MRD ~MWR (pad_bidir)
	idle    <= not (fetching or vram_to_oam or sprite_read or cpu_vram);
	reading <= (not (sose and write) nand not ppu_owns) and not sprite_mrd and not fetching
	           and not vram_to_oam;
	writing <= not (sose and cpu_wr_sync and cpu_vram);

	mcs_n <= 'Z' when tuto = '1' else idle;
	mrd_n <= 'Z' when tuto = '1' else reading;
	mwr_n <= 'Z' when tuto = '1' else writing;
end architecture;
