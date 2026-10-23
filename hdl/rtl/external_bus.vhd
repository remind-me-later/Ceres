-- SPDX-License-Identifier: MIT
--
-- The cartridge bus: the pins A0 to A15, D0 to D7, /RD, /WR and /CS, the
-- CPU's read and write strobes for the chip, and the precharge of the
-- data bus inside the chip.
--
-- The internal data bus is precharged high while the CPU core is between
-- M-cycles (exec_phase low: phase A, or the CPU halted); afterwards only
-- the block that the CPU reads drives it, and in between it keeps its
-- value.
--
-- @hw die: dmg-schematics, categories bus-adr, bus-data and sys-decode
-- (sheets "External address buses" and "External data buses"); checked
-- against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;

entity external_bus is
	port (
		clk_t4        : in std_ulogic;
		exec_phase    : in std_ulogic;
		cpu_wr_sync   : in std_ulogic;
		cpu_ext_cs_en : in std_ulogic;

		-- From the CPU core
		read  : in std_ulogic;  -- the CPU reads
		write : in std_ulogic;  -- the CPU writes
		mreq  : in std_ulogic;  -- ... memory outside the chip, or VRAM

		boot_sel : in std_ulogic;  -- the boot ROM answers

		-- OAM DMA, when it reads the cartridge bus
		dma_addr_ext : in std_ulogic;
		dma_addr     : in std_ulogic_vector(15 downto 0);

		-- Test mode: T1 alone makes the pins drive the CPU's bus
		t1_only : in std_ulogic;
		t2_only : in std_ulogic;

		-- The CPU's bus
		addr   : inout std_logic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : out   std_ulogic;
		cpu_wr : out   std_ulogic;

		non_vram_mreq : out std_ulogic;

		-- Pins
		a_pins : inout std_logic_vector(15 downto 0);
		d_pins : inout std_logic_vector(7 downto 0);
		rd_n   : inout std_logic;
		wr_n   : inout std_logic;
		cs_n   : out   std_ulogic
	);
end entity;

architecture rtl of external_bus is
	signal non_vram_mreq_i : std_ulogic;
	signal vram_addr       : std_ulogic := '0';
	signal addr_latch      : std_ulogic;
	signal latched_addr    : std_ulogic_vector(14 downto 0) := (others => '0');
	signal pin_addr        : std_ulogic_vector(15 downto 0);
	signal a15_cpu         : std_ulogic;
	signal cs              : std_ulogic;
	signal outside         : std_ulogic;  -- the access leaves the chip
	signal pins_out        : std_ulogic;  -- the data pins drive the bus outside
	signal read_pins       : std_ulogic;  -- the data pins drive the bus inside
	signal latched_data    : std_ulogic_vector(7 downto 0) := (others => '0');
	signal precharge       : std_ulogic;
begin
	-- The chip's own registers see the CPU's strobes; test mode takes them
	-- from the pins.
	-- @hw die: UJYV (muxi) TEDO, UBAL (muxi) TAPU
	cpu_rd <= not to_x01(rd_n) when t1_only = '1' else read;
	cpu_wr <= not to_x01(wr_n) when t1_only = '1' else cpu_wr_sync;

	-- An access that leaves the chip: mreq, outside VRAM (8000 to 9FFF).
	-- The decoding of VRAM's addresses takes 0.3 ns; TEXO rises 2.4 ns
	-- after mreq and falls 1.35 ns after the decoding, and passes no
	-- shorter pulse. An address from the register file reaches the
	-- decoding 3 or 4 ns after mreq rises (../rtl/sm83.vhd), and a VRAM
	-- access from there leaves the chip for a moment when it starts; one
	-- from W (LD (nn),A, LD A,(nn)) arrives with mreq and does not.
	-- @hw die: SORE TEVY TEXO (the delays are dmg-sim's)
	vram_addr       <= '1' after 300 ps when addr(15 downto 13) = "100" else '0' after 300 ps;
	non_vram_mreq_i <= '1' after 2400 ps when mreq = '1' and vram_addr = '0' else '0' after 1350 ps;
	non_vram_mreq   <= non_vram_mreq_i;

	-- The address pins hold the address of the last access that left the
	-- chip, from latches that are open during such accesses. (The CPU core
	-- changes the high byte of its address before its request, and the low
	-- byte after it: when an access ends, in phase A, the pins keep the low
	-- byte, and their high byte drops to 00 with the internal bus.)
	-- @hw die: ALOR APUR ALYR ARET AVYS ATEV AROS ARYM LUNO LYSA PATE LUMY
	-- LOBU LONU NYRE (dlatch), LOXO LASY MATE
	--
	-- The latches take a new address only if they stay open for about
	-- 2.7 ns: the moment a VRAM access leaves the chip passes the address
	-- when it comes from HL or SP, and not when it comes from BC or DE,
	-- which arrive sooner (as in dmg-sim).
	addr_latch <= non_vram_mreq_i or t2_only after 2700 ps;

	address_latch : process (all)
	begin
		if addr_latch = '1' then
			latched_addr <= to_x01(addr(14 downto 0));
		end if;
	end process;

	-- A15 is low only while the CPU reads or writes the cartridge's ROM
	-- (0000 to 7FFF, without the boot ROM), in phases C to H.
	-- @hw die: SOBY SEPY
	a15_cpu <= not (cpu_ext_cs_en and not addr(15) and not boot_sel);

	-- @hw die: AMET ATOL APOK AMER ATEM ATOV ATYR ASUR MANO MASU PAMY MALE
	-- MOJY MUCE PEGE TAZY (mux)
	pin_addr <= dma_addr when dma_addr_ext = '1' else a15_cpu & latched_addr;

	-- @hw die: A0..A15 (pad_bidir), KUPO KOTY .. SUZE RULO; KEJO .. SYZU
	-- (buf_if0), TOVA RYCA
	a_pins <= pin_addr when t1_only = '0' else (others => 'Z');
	addr   <= to_x01(a_pins) when t1_only = '1' else (others => 'Z');

	-- /CS selects the cartridge's RAM and the work RAM: A000 to FDFF, in
	-- phases C to H. DMA selects them by its address's bit 15.
	-- @hw die: TYNU TUMA TOZA TYHO, ~CS (pad_out)
	cs   <= dma_addr(15) when dma_addr_ext = '1' else
	        cpu_ext_cs_en when (addr(15 downto 13) = "101" or addr(15 downto 14) = "11")
	                       and addr(15 downto 9) /= "1111111" else
	        '0';
	cs_n <= not cs;

	-- The data pins drive the CPU's data while it writes outside VRAM (or
	-- reads, in test mode T2). Otherwise they have pull-ups.
	-- @hw die: MOCA LAGU LYWE MOTY, RORU (mux), LULA; RUXA RUNE .. (nand2,
	-- nor2), D0..D7 (pad_bidir_pu)
	outside  <= non_vram_mreq_i or t2_only;
	pins_out <= outside and ((read and not non_vram_mreq_i) or write) when t1_only = '0' else
	            not to_x01(rd_n);
	d_pins   <= to_x01(data) when pins_out = '1' else (others => 'H');

	-- /RD is low unless the CPU writes outside VRAM; /WR is low while it
	-- does, in phases E to G. DMA keeps /RD low and /WR high.
	-- @hw die: TYMU UGAC URUN, PUVA NEVY MEXO UVER USUF, ~RD ~WR (pad_bidir)
	rd_n <= 'Z' when t1_only = '1' else
	        '1' when dma_addr_ext = '0' and pins_out = '1' else
	        '0';
	wr_n <= 'Z' when t1_only = '1' else
	        '0' when dma_addr_ext = '0' and cpu_wr_sync = '1' and outside = '1' else
	        '1';

	-- A read outside the chip takes the data pins as they are at the start
	-- of phase G, from latches that are open until then.
	-- @hw die: SOMA RONY RAXY SELO SODY SAGO RUPA SAZY (dlatch), LAVO
	read_pins <= read and non_vram_mreq_i and clk_t4;

	data_latch : process (all)
	begin
		if read_pins = '0' then
			latched_data <= to_x01(d_pins);
		end if;
	end process;

	-- @hw die: TOVU..TEWA (buf_if0), ROMY..RARU (not_if1), ANOC..BEDA
	-- (not_if0), SOLE LECO AFOP; RYMA..TAJU (not_if0); TUTY..TEMY (buf_if0)
	precharge <= not exec_phase and not t1_only;

	data_out <= (others => '1') when precharge = '1' else
	        latched_data when read_pins = '1' else
	        to_x01(d_pins) when t1_only = '1' and to_x01(rd_n) = '1' else
	        (others => 'Z');
end architecture;
