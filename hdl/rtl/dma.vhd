-- SPDX-License-Identifier: MIT
--
-- OAM DMA: writing FF46 copies 160 bytes, from XX00 to XX9F, to OAM, one
-- per M-cycle. The source is VRAM (8000 to 9FFF) or the cartridge bus.
--
-- @hw die: dmg-schematics, category ppu-dma (sheet "FF46 DMA"); checked
-- against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity dma is
	port (
		dma_phi    : in std_ulogic;  -- high in phases A to D, and while the CPU is halted
		hard_reset : in std_ulogic;  -- reset2

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		dma_run      : out std_ulogic;  -- a byte is copied in this M-cycle
		dma_addr     : out std_ulogic_vector(15 downto 0);
		dma_addr_ext : out std_ulogic;  -- ... from the cartridge bus
		vram_to_oam  : out std_ulogic   -- ... from VRAM
	);
end entity;

architecture rtl of dma is
	signal write_ff46  : std_ulogic;
	signal source      : std_ulogic_vector(7 downto 0) := (others => '0');  -- FF46
	signal requested   : std_ulogic := '0';
	signal start_1     : std_ulogic := '0';
	signal start_2     : std_ulogic := '0';
	signal restart     : std_ulogic;
	signal enabled     : std_ulogic := '0';
	signal count       : unsigned(7 downto 0) := (others => '0');
	signal done        : std_ulogic := '0';
	signal run         : std_ulogic := '0';
	signal from_vram   : std_ulogic;
begin
	-- @hw die: LAVY LORU PYSU, MOLU NYGO PUSY
	write_ff46 <= '1' when cpu_wr = '1' and addr = x"FF46" else '0';

	-- FF46 is a latch, open while the CPU writes it.
	-- @hw die: NAFA PYNE PARA NYDO NYGY PULA POKU MARU (dlatch_ee)
	source_latch : process (all)
	begin
		if write_ff46 = '1' then
			source <= to_x01(data);
		end if;
	end process;

	-- A write requests a transfer; it starts when the write is over, after
	-- the next rise and fall of dma_phi, and then starts over even if one is
	-- already running.
	-- @hw die: LYXE (nor_latch), LUPA, LUVY LENE (dffr), LOKO LAPA
	request_latch : process (all)
	begin
		if write_ff46 = '1' then
			requested <= '1';
		elsif restart = '1' then
			requested <= '0';
		end if;
	end process;

	start_1_ff : process (dma_phi, hard_reset)
	begin
		if hard_reset = '1' then
			start_1 <= '0';
		elsif rising_edge(dma_phi) then
			start_1 <= requested and not write_ff46;
		end if;
	end process;

	start_2_ff : process (dma_phi, hard_reset)
	begin
		if hard_reset = '1' then
			start_2 <= '0';
		elsif falling_edge(dma_phi) then
			start_2 <= start_1;
		end if;
	end process;

	restart <= start_2 or hard_reset;

	-- The transfer is enabled from the start until the last byte.
	-- @hw die: LOKY LARA (nand latch)
	enable_latch : process (all)
	begin
		if start_2 = '1' then
			enabled <= '1';
		elsif done = '1' or hard_reset = '1' then
			enabled <= '0';
		end if;
	end process;

	-- The low byte of the address counts when dma_phi rises; the last one,
	-- 9F, shows when dma_phi falls.
	-- @hw die: NAKY PYRO NEFY MUTY NYKO PYLO NUTO MUGU (dffr), META; NAVO
	-- NOLO, MYTE (dffr)
	counter : process (dma_phi, restart)
	begin
		if restart = '1' then
			count <= (others => '0');
		elsif rising_edge(dma_phi) and enabled = '1' then
			count <= count + 1;
		end if;
	end process;

	done_ff : process (dma_phi, restart)
	begin
		if restart = '1' then
			done <= '0';
		elsif falling_edge(dma_phi) then
			done <= '1' when count(7) = '1' and count(4 downto 0) = "11111" else '0';
		end if;
	end process;

	-- @hw die: MATU (dffr)
	run_ff : process (dma_phi, hard_reset)
	begin
		if hard_reset = '1' then
			run <= '0';
		elsif rising_edge(dma_phi) then
			run <= enabled;
		end if;
	end process;
	dma_run <= run;

	dma_addr <= source & std_ulogic_vector(count);

	-- @hw die: MUDA LOGO MORY LUMA, MUHO LUFA
	from_vram    <= '1' when source(7 downto 5) = "100" else '0';
	dma_addr_ext <= run and not from_vram;
	vram_to_oam  <= run and from_vram;

	-- @hw die: POLY ROFO REMA PANE PARE RALY RESU NUVY (not_if1)
	data <= source when cpu_rd = '1' and addr = x"FF46" else (others => 'Z');
end architecture;
